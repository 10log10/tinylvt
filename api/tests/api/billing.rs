//! Tests for storage-based billing enforcement and webhook
//! state transitions.

use api::store;
use payloads::{SubscriptionStatus, SubscriptionTier, TierLimits, requests};
use reqwest::StatusCode;
use test_helpers::{TestApp, assert_status_code, spawn_app};

/// Helper to get current storage usage directly from the database.
async fn get_cached_storage_total(
    app: &TestApp,
    community_id: payloads::CommunityId,
) -> i64 {
    let usage =
        store::billing::get_cached_storage_usage(&app.db_pool, community_id)
            .await
            .expect("Failed to get cached storage usage");
    usage.map(|u| u.total_bytes()).unwrap_or(0)
}

/// Helper to insert a fake large image with specified size.
async fn insert_fake_large_image(
    app: &TestApp,
    community_id: payloads::CommunityId,
    file_size: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO site_images
            (community_id, name, image_data, mime_type, file_size,
             created_at, updated_at)
        VALUES ($1, 'fake_large_image', $2, 'image/png', $3, $4, $4)
        "#,
    )
    .bind(community_id)
    .bind(vec![0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
    .bind(file_size)
    .bind(jiff_sqlx::Timestamp::from(app.time_source.now()))
    .execute(&app.db_pool)
    .await?;
    Ok(())
}

/// Helper to set the cached image_bytes to a specific value.
async fn set_cached_image_bytes(
    app: &TestApp,
    community_id: payloads::CommunityId,
    image_bytes: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO community_storage_usage (
            community_id, image_bytes, member_bytes, space_bytes,
            auction_bytes, transaction_bytes, calculated_at
        )
        VALUES ($1, $2, 0, 0, 0, 0, $3)
        ON CONFLICT (community_id) DO UPDATE SET
            image_bytes = EXCLUDED.image_bytes
        "#,
    )
    .bind(community_id)
    .bind(image_bytes)
    .bind(jiff_sqlx::Timestamp::from(app.time_source.now()))
    .execute(&app.db_pool)
    .await?;
    Ok(())
}

/// Helper to retrieve current cached byte values from the database.
async fn get_cached_bytes(
    app: &TestApp,
    community_id: payloads::CommunityId,
) -> anyhow::Result<(i64, i64, i64, i64, i64)> {
    let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
        "SELECT image_bytes, member_bytes, space_bytes, auction_bytes, \
         transaction_bytes FROM community_storage_usage WHERE community_id = $1",
    )
    .bind(community_id)
    .fetch_optional(&app.db_pool)
    .await?;

    Ok(row.unwrap_or((0, 0, 0, 0, 0)))
}

#[tokio::test]
async fn test_storage_enforcement_blocks_image_upload_over_limit()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    let free_limit = TierLimits::for_tier(SubscriptionTier::Free).storage_bytes;

    // Insert a fake large image to fill storage
    insert_fake_large_image(&app, community_id, free_limit).await?;

    // Trigger cache recalculation by refreshing
    store::billing::refresh_all_community_storage(
        &app.db_pool,
        &app.time_source,
    )
    .await?;

    // Now try to upload another image - should be blocked
    let body = test_helpers::site_image_details_a(community_id);
    let result = app.client.create_site_image(&body).await;

    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn test_storage_enforcement_allows_image_upload_under_limit()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    // Upload an image - should succeed since we're well under the limit
    let site_image = app.create_test_site_image(&community_id).await?;
    assert!(!site_image.name.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_immediate_cache_update_on_image_create() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    // Get initial cache state (may not exist)
    let initial = get_cached_storage_total(&app, community_id).await;

    // Upload an image
    let site_image = app.create_test_site_image(&community_id).await?;
    let image_size = site_image.file_size;

    // Check that cache was updated with the new image size
    let after_upload = get_cached_storage_total(&app, community_id).await;

    // Cache should have increased by at least the image size
    // (it may have done a full recalculation which includes other data)
    assert!(
        after_upload >= initial + image_size,
        "Cache should increase after image upload: initial={}, after={}, \
         image_size={}",
        initial,
        after_upload,
        image_size
    );

    Ok(())
}

#[tokio::test]
async fn test_immediate_cache_update_on_image_delete() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    // Upload an image first
    let site_image = app.create_test_site_image(&community_id).await?;
    let image_size = site_image.file_size;

    // Get cache state after upload
    let after_upload = get_cached_storage_total(&app, community_id).await;

    // Delete the image
    app.client.delete_site_image(&site_image.id).await?;

    // Check that cache was decremented
    let after_delete = get_cached_storage_total(&app, community_id).await;

    assert_eq!(
        after_delete,
        after_upload - image_size,
        "Cache should decrease by image size after deletion"
    );

    Ok(())
}

#[tokio::test]
async fn test_background_refresh_updates_all_communities() -> anyhow::Result<()>
{
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    // Upload some images
    let _img1 = app.create_test_site_image(&community_id).await?;
    let body2 = test_helpers::site_image_details_b(community_id);
    let _img2 = app.client.create_site_image(&body2).await?;

    // Clear the cache to simulate stale data
    sqlx::query("DELETE FROM community_storage_usage WHERE community_id = $1")
        .bind(community_id)
        .execute(&app.db_pool)
        .await?;

    // Manually trigger the background refresh
    let stats = store::billing::refresh_all_community_storage(
        &app.db_pool,
        &app.time_source,
    )
    .await?;

    assert!(stats.total > 0, "Should have at least one community");
    assert_eq!(stats.error_count, 0, "Should have no errors");

    // Verify cache was populated
    let usage = get_cached_storage_total(&app, community_id).await;
    assert!(usage > 0, "Cache should be populated after refresh");

    Ok(())
}

#[tokio::test]
async fn test_storage_enforcement_blocks_site_creation() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    let free_limit = TierLimits::for_tier(SubscriptionTier::Free).storage_bytes;

    // Insert a fake large image to fill storage
    insert_fake_large_image(&app, community_id, free_limit).await?;

    // Trigger cache recalculation
    store::billing::refresh_all_community_storage(
        &app.db_pool,
        &app.time_source,
    )
    .await?;

    // Try to create a site - should be blocked by storage limit
    let site_details = test_helpers::site_details_b(community_id);
    let result = app.client.create_site(&site_details).await;

    // Verify it's blocked specifically for storage, not some other error
    let err =
        result.expect_err("Site creation should fail due to storage limit");
    assert!(
        err.to_string().to_lowercase().contains("storage"),
        "Error should mention storage limit, got: {}",
        err
    );

    Ok(())
}

#[tokio::test]
async fn test_storage_enforcement_allows_operations_under_limit()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    // With a fresh community, we should be well under the limit
    // All these operations should succeed

    // Create a site
    let site = app.create_test_site(&community_id).await?;
    assert!(!site.site_details.name.is_empty());

    // Create a space
    let space_details = test_helpers::space_details_a(site.site_id);
    let space_id = app.client.create_space(&space_details).await?;
    let space = app.client.get_space(&space_id).await?;
    assert!(!space.space_details.name.is_empty());

    // Create an auction (requires site)
    let auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    let auction_id = app.client.create_auction(&auction_details).await?;
    let auction = app.client.get_auction(&auction_id).await?;
    assert!(auction.end_at.is_none()); // Not yet concluded

    Ok(())
}

#[tokio::test]
async fn test_cache_bypassed_when_near_limit() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    let free_limit = TierLimits::for_tier(SubscriptionTier::Free).storage_bytes;

    // Set up cache showing 91% usage (above 90% threshold)
    let near_limit_bytes = (free_limit as f64 * 0.91) as i64;
    set_cached_image_bytes(&app, community_id, near_limit_bytes).await?;

    // Verify the fake cache was inserted
    let (cached_before, _, _, _, _) =
        get_cached_bytes(&app, community_id).await?;
    assert_eq!(cached_before, near_limit_bytes);

    // When we try to upload an image, the system should:
    // 1. Check cache, see we're at 91% (above 90% threshold)
    // 2. Bypass cache and recalculate from database
    // 3. Find that actual usage is much lower (just the member row)
    // 4. Allow the upload
    // 5. Update cache to reflect actual usage

    let site_image = app.create_test_site_image(&community_id).await?;
    assert!(!site_image.name.is_empty());

    // Verify that the cache was recalculated and dropped significantly
    let (image, member, space, auction, transaction) =
        get_cached_bytes(&app, community_id).await?;
    let cached_after = image + member + space + auction + transaction;

    // The actual usage should be well below the fake 91% we set
    let usage_percent = (cached_after as f64 / free_limit as f64) * 100.0;
    assert!(
        usage_percent < 1.0,
        "Cache should have been recalculated to much lower value, \
         but got {}%",
        usage_percent
    );

    Ok(())
}

#[tokio::test]
async fn test_cache_used_when_far_from_limit() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    let free_limit = TierLimits::for_tier(SubscriptionTier::Free).storage_bytes;

    // Set up cache showing 50% usage (well below 90% threshold)
    // This is an intentionally incorrect high value to verify cache is used
    let half_limit = free_limit / 2;
    set_cached_image_bytes(&app, community_id, half_limit).await?;

    // Verify the fake cache was inserted
    let (cached_before, _, _, _, _) =
        get_cached_bytes(&app, community_id).await?;
    assert_eq!(cached_before, half_limit);

    // Since we're at 50% (below 90% threshold), cache should be used
    // The upload should succeed because half_limit + image_size < limit
    let site_image = app.create_test_site_image(&community_id).await?;
    assert!(!site_image.name.is_empty());

    // Verify that the cache was NOT recalculated from the database.
    // The storage check should have used the cached 50% value, so only the
    // image_bytes delta was applied. If cache was recalculated, the other
    // fields would be populated with actual values, not 0.
    let (
        image_bytes,
        member_bytes,
        space_bytes,
        auction_bytes,
        transaction_bytes,
    ) = get_cached_bytes(&app, community_id).await?;

    // Cache was used (not recalculated), so member_bytes etc should still be 0
    // Only image_bytes was updated with the delta
    assert_eq!(
        member_bytes, 0,
        "Cache should not have been recalculated - member_bytes should be 0"
    );
    assert_eq!(
        space_bytes, 0,
        "Cache should not have been recalculated - space_bytes should be 0"
    );
    assert_eq!(
        auction_bytes, 0,
        "Cache should not have been recalculated - auction_bytes should be 0"
    );
    assert_eq!(
        transaction_bytes, 0,
        "Cache should not have been recalculated - transaction_bytes should be 0"
    );
    // image_bytes should be slightly more than half_limit due to the image we
    // added
    assert!(
        image_bytes > half_limit,
        "image_bytes should have been incremented by image delta"
    );

    Ok(())
}

#[tokio::test]
async fn test_member_addition_allowed_even_at_storage_limit()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    let free_limit = TierLimits::for_tier(SubscriptionTier::Free).storage_bytes;

    // Insert a fake large image to fill storage
    insert_fake_large_image(&app, community_id, free_limit).await?;

    // Trigger cache recalculation so it reflects the large image
    store::billing::refresh_all_community_storage(
        &app.db_pool,
        &app.time_source,
    )
    .await?;

    // Verify cache now shows we're at the limit
    let cached = get_cached_storage_total(&app, community_id).await;
    assert!(
        cached >= free_limit,
        "Cache should reflect storage at limit: {}",
        cached
    );

    // Create Bob and have them join the community
    // This should succeed even though storage is over the limit
    // (member addition is not enforced to avoid confusing non-coleaders)
    app.create_bob_user().await?;
    let invite_id = app.invite_bob().await?;
    app.login_bob().await?;

    // Accept the invite - should succeed despite storage being at limit
    let result = app.client.accept_invite(&invite_id).await;

    assert!(
        result.is_ok(),
        "Member addition should succeed at storage limit"
    );

    Ok(())
}

#[tokio::test]
async fn test_storage_enforcement_blocks_auction_creation() -> anyhow::Result<()>
{
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    let free_limit = TierLimits::for_tier(SubscriptionTier::Free).storage_bytes;

    // Insert a fake large image to fill storage
    insert_fake_large_image(&app, community_id, free_limit).await?;

    // Trigger cache recalculation so enforcement sees the storage limit
    store::billing::refresh_all_community_storage(
        &app.db_pool,
        &app.time_source,
    )
    .await?;

    // Insert a site directly to avoid the storage check (we need a site for
    // auctions). We're testing auction enforcement, not site enforcement.
    let now = jiff_sqlx::Timestamp::from(app.time_source.now());

    let auction_params_id: api::store::AuctionParamsId = sqlx::query_scalar(
        r#"
        INSERT INTO auction_params (
            round_duration, bid_increment, activity_rule_params,
            created_at, updated_at
        )
        VALUES ('1 minute'::interval, 1.0, '[]'::jsonb, $1, $1)
        RETURNING id
        "#,
    )
    .bind(now)
    .fetch_one(&app.db_pool)
    .await?;

    let site_id: payloads::SiteId = sqlx::query_scalar(
        r#"
        INSERT INTO sites (
            community_id, name, description, default_auction_params_id,
            possession_period, auction_lead_time, proxy_bidding_lead_time,
            auto_schedule, timezone, created_at, updated_at
        )
        VALUES ($1, 'test site', 'test', $2, '1 hour'::interval,
                '1 hour'::interval, '1 hour'::interval, false, 'UTC', $3, $3)
        RETURNING id
        "#,
    )
    .bind(community_id)
    .bind(auction_params_id)
    .bind(now)
    .fetch_one(&app.db_pool)
    .await?;

    // Try to create an auction - should be blocked
    let auction_details =
        test_helpers::auction_details_a(site_id, &app.time_source);
    let result = app.client.create_auction(&auction_details).await;

    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

// --- Webhook state transition tests ---
//
// These test the webhook handler logic by calling
// handle_webhook_event directly with constructed stripe types.
// This avoids fighting with async-stripe's strict JSON
// deserialization which requires many fields we don't use.

/// Fetch subscription info via the API endpoint.
async fn get_subscription(
    app: &TestApp,
    community_id: payloads::CommunityId,
) -> Option<payloads::SubscriptionInfo> {
    let request = requests::GetSubscriptionInfo { community_id };
    app.client
        .get_subscription_info(&request)
        .await
        .expect("get_subscription_info request failed")
}

/// Insert a subscription row directly for tests that need
/// an existing subscription to act on.
async fn insert_test_subscription(
    app: &TestApp,
    community_id: payloads::CommunityId,
    subscription_id: &str,
) {
    let now = jiff_sqlx::Timestamp::from(app.time_source.now());
    sqlx::query(
        "UPDATE communities SET stripe_customer_id = 'cus_test' \
         WHERE id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await
    .expect("Failed to set stripe customer ID on community");

    sqlx::query(
        "INSERT INTO community_subscriptions (
            community_id, tier, status, billing_interval,
            stripe_subscription_id,
            current_period_start, current_period_end,
            cancel_at_period_end,
            created_at, updated_at
        ) VALUES (
            $1, 'paid', 'active', 'month',
            $2, $3, $3, false, $3, $3
        )",
    )
    .bind(community_id)
    .bind(subscription_id)
    .bind(now)
    .execute(&app.db_pool)
    .await
    .expect("Failed to insert test subscription");
}

/// Convert a stripe::Event to serde_json::Value for the
/// webhook handler (which parses raw JSON).
fn to_value(event: stripe::Event) -> serde_json::Value {
    serde_json::to_value(event).unwrap()
}

/// Build a subscription event (created or updated).
#[allow(clippy::too_many_arguments)]
fn make_subscription_event(
    event_type: stripe::EventType,
    subscription_id: &str,
    customer_id: &str,
    status: stripe::SubscriptionStatus,
    interval: stripe::PlanInterval,
    cancel_at_period_end: bool,
    period_start: i64,
    period_end: i64,
) -> stripe::Event {
    use stripe::{EventObject, Expandable, NotificationEventData};

    let sub = stripe::Subscription {
        id: subscription_id.parse().unwrap(),
        customer: Expandable::Id(customer_id.parse().unwrap()),
        status,
        cancel_at_period_end,
        current_period_start: period_start,
        current_period_end: period_end,
        items: stripe::List {
            data: vec![stripe::SubscriptionItem {
                id: "si_test".parse().unwrap(),
                plan: Some(stripe::Plan {
                    id: "plan_test".parse().unwrap(),
                    interval: Some(interval),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            has_more: false,
            total_count: Some(1),
            url: "/v1/subscription_items".to_string(),
        },
        ..Default::default()
    };

    stripe::Event {
        id: "evt_test".parse().unwrap(),
        type_: event_type,
        data: NotificationEventData {
            object: EventObject::Subscription(sub),
            previous_attributes: None,
        },
        ..Default::default()
    }
}

#[tokio::test]
async fn test_webhook_subscription_created_inserts_row() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    assert!(get_subscription(&app, community_id).await.is_none());

    // Set up mock customer → community mapping
    app.stripe_service
        .mock_customer_communities
        .lock()
        .unwrap()
        .insert("cus_test".to_string(), community_id);

    let event = make_subscription_event(
        stripe::EventType::CustomerSubscriptionCreated,
        "sub_test",
        "cus_test",
        stripe::SubscriptionStatus::Active,
        stripe::PlanInterval::Month,
        false,
        1700000000,
        1702592000,
    );
    store::billing::handle_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &to_value(event),
    )
    .await?;

    let info = get_subscription(&app, community_id)
        .await
        .expect("Subscription should exist");
    assert_eq!(info.status, SubscriptionStatus::Active);
    assert_eq!(info.billing_interval, payloads::BillingInterval::Month);
    assert!(!info.cancel_at_period_end);

    Ok(())
}

#[tokio::test]
async fn test_webhook_subscription_updated_sets_cancel() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    insert_test_subscription(&app, community_id, "sub_test").await;

    let event = make_subscription_event(
        stripe::EventType::CustomerSubscriptionUpdated,
        "sub_test",
        "cus_test",
        stripe::SubscriptionStatus::Active,
        stripe::PlanInterval::Month,
        true,
        1700000000,
        1702592000,
    );
    store::billing::handle_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &to_value(event),
    )
    .await?;

    let info = get_subscription(&app, community_id).await.unwrap();
    assert_eq!(info.status, SubscriptionStatus::Active);
    assert!(
        info.cancel_at_period_end,
        "cancel_at_period_end should be true"
    );

    Ok(())
}

#[tokio::test]
async fn test_webhook_subscription_canceled() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    insert_test_subscription(&app, community_id, "sub_test").await;

    let event = make_subscription_event(
        stripe::EventType::CustomerSubscriptionUpdated,
        "sub_test",
        "cus_test",
        stripe::SubscriptionStatus::Canceled,
        stripe::PlanInterval::Month,
        false,
        1700000000,
        1702592000,
    );
    store::billing::handle_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &to_value(event),
    )
    .await?;

    let info = get_subscription(&app, community_id).await.unwrap();
    assert_eq!(info.status, SubscriptionStatus::Canceled);

    Ok(())
}

#[tokio::test]
async fn test_webhook_subscription_past_due() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    insert_test_subscription(&app, community_id, "sub_test").await;

    let event = make_subscription_event(
        stripe::EventType::CustomerSubscriptionUpdated,
        "sub_test",
        "cus_test",
        stripe::SubscriptionStatus::PastDue,
        stripe::PlanInterval::Month,
        false,
        1700000000,
        1702592000,
    );
    store::billing::handle_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &to_value(event),
    )
    .await?;

    let info = get_subscription(&app, community_id).await.unwrap();
    assert_eq!(info.status, SubscriptionStatus::PastDue);

    Ok(())
}

#[tokio::test]
async fn test_webhook_past_due_reactivates() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    insert_test_subscription(&app, community_id, "sub_test").await;

    // Set to past_due
    let event = make_subscription_event(
        stripe::EventType::CustomerSubscriptionUpdated,
        "sub_test",
        "cus_test",
        stripe::SubscriptionStatus::PastDue,
        stripe::PlanInterval::Month,
        false,
        1700000000,
        1702592000,
    );
    store::billing::handle_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &to_value(event),
    )
    .await?;

    let info = get_subscription(&app, community_id).await.unwrap();
    assert_eq!(info.status, SubscriptionStatus::PastDue);

    // Payment retry succeeds
    let event = make_subscription_event(
        stripe::EventType::CustomerSubscriptionUpdated,
        "sub_test",
        "cus_test",
        stripe::SubscriptionStatus::Active,
        stripe::PlanInterval::Month,
        false,
        1700000000,
        1702592000,
    );
    store::billing::handle_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &to_value(event),
    )
    .await?;

    let info = get_subscription(&app, community_id).await.unwrap();
    assert_eq!(info.status, SubscriptionStatus::Active);

    Ok(())
}

#[tokio::test]
async fn test_webhook_resubscribe_after_cancel() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    insert_test_subscription(&app, community_id, "sub_test").await;

    // Cancel
    let event = make_subscription_event(
        stripe::EventType::CustomerSubscriptionUpdated,
        "sub_test",
        "cus_test",
        stripe::SubscriptionStatus::Canceled,
        stripe::PlanInterval::Month,
        false,
        1700000000,
        1702592000,
    );
    store::billing::handle_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &to_value(event),
    )
    .await?;

    let info = get_subscription(&app, community_id).await.unwrap();
    assert_eq!(info.status, SubscriptionStatus::Canceled);

    // Resubscribe via new subscription.created
    app.stripe_service
        .mock_customer_communities
        .lock()
        .unwrap()
        .insert("cus_test_new".to_string(), community_id);
    let event = make_subscription_event(
        stripe::EventType::CustomerSubscriptionCreated,
        "sub_test_new",
        "cus_test_new",
        stripe::SubscriptionStatus::Active,
        stripe::PlanInterval::Month,
        false,
        1700000000,
        1702592000,
    );
    store::billing::handle_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &to_value(event),
    )
    .await?;

    let info = get_subscription(&app, community_id).await.unwrap();
    assert_eq!(info.status, SubscriptionStatus::Active);
    assert!(!info.cancel_at_period_end);

    Ok(())
}

#[tokio::test]
async fn test_delete_community_cancels_stripe_subscription()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    insert_test_subscription(&app, community_id, "sub_test").await;

    app.client.delete_community(&community_id).await?;

    // Verify the Stripe subscription was canceled
    let canceled = app
        .stripe_service
        .mock_canceled_subscriptions
        .lock()
        .unwrap();
    assert_eq!(
        canceled.as_slice(),
        &["sub_test"],
        "Stripe subscription should have been canceled"
    );

    Ok(())
}
