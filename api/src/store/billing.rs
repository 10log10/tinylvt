//! Billing and storage usage tracking for communities.

use anyhow::Context;
use jiff_sqlx::ToSqlx;
use sqlx::PgPool;

use payloads::{
    BillingInterval, CommunityId, StorageUsage, SubscriptionInfo,
    SubscriptionStatus, SubscriptionTier, TierLimits,
};

use crate::AppConfig;
use crate::stripe_service::StripeService;
use crate::time::TimeSource;

use super::{StoreError, ValidatedMember};

/// Subscription tier as stored in the database (only paid communities
/// have rows; missing row = free tier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "subscription_tier", rename_all = "snake_case")]
enum DbSubscriptionTier {
    Paid,
}

/// Row size estimates in bytes for storage calculation.
/// Based on PostgreSQL storage: column sizes + 23 byte tuple header + indexes.
/// Exposed publicly so callers can estimate operation size.
pub mod row_estimates {
    // Members category
    pub const MEMBER: i64 = 120; // 2 UUIDs, role enum, bool, 2 timestamps
    pub const ACCOUNT: i64 = 160; // 2 UUIDs, enum, numeric, timestamps

    // Spaces category (descriptions add ~200 bytes avg if present)
    pub const SITE: i64 = 400; // many columns + description text
    pub const SPACE: i64 = 350; // UUID refs, name, description, floats

    // Auctions category
    pub const AUCTION: i64 = 180; // UUID refs, timestamps, integers
    pub const AUCTION_ROUND: i64 = 150; // UUID, int, timestamps, floats
    pub const BID: i64 = 150; // 3 UUIDs, timestamps + indexes
    pub const ROUND_SPACE_RESULT: i64 = 160; // 3 UUIDs, numeric + indexes
    pub const USER_ELIGIBILITY: i64 = 80; // 2 UUIDs, float

    // Transactions category
    pub const JOURNAL_ENTRY: i64 = 240; // UUIDs, enum, note text, timestamp
    pub const JOURNAL_LINE: i64 = 110; // 3 UUIDs, numeric + index
}

/// Raw row counts from the database for storage calculation.
#[derive(Debug, sqlx::FromRow)]
struct RowCounts {
    image_bytes: i64,
    members: i64,
    accounts: i64,
    sites: i64,
    spaces: i64,
    auctions: i64,
    auction_rounds: i64,
    bids: i64,
    round_space_results: i64,
    user_eligibilities: i64,
    journal_entries: i64,
    journal_lines: i64,
}

/// Calculate current storage usage for a community.
fn calculate_storage_usage(
    counts: &RowCounts,
    calculated_at: jiff::Timestamp,
) -> StorageUsage {
    StorageUsage {
        image_bytes: counts.image_bytes,
        member_bytes: counts.members * row_estimates::MEMBER
            + counts.accounts * row_estimates::ACCOUNT,
        space_bytes: counts.sites * row_estimates::SITE
            + counts.spaces * row_estimates::SPACE,
        auction_bytes: counts.auctions * row_estimates::AUCTION
            + counts.auction_rounds * row_estimates::AUCTION_ROUND
            + counts.bids * row_estimates::BID
            + counts.round_space_results * row_estimates::ROUND_SPACE_RESULT
            + counts.user_eligibilities * row_estimates::USER_ELIGIBILITY,
        transaction_bytes: counts.journal_entries
            * row_estimates::JOURNAL_ENTRY
            + counts.journal_lines * row_estimates::JOURNAL_LINE,
        calculated_at,
    }
}

/// Fetch row counts from the database.
async fn fetch_row_counts(
    pool: &PgPool,
    community_id: CommunityId,
) -> Result<RowCounts, StoreError> {
    sqlx::query_as::<_, RowCounts>(
        r#"
        SELECT
            -- Images (actual bytes)
            COALESCE(
                (SELECT SUM(file_size)::BIGINT FROM site_images WHERE community_id = $1),
                0
            ) as image_bytes,

            -- Members
            (SELECT COUNT(*) FROM community_members
                WHERE community_id = $1) as members,
            (SELECT COUNT(*) FROM accounts
                WHERE community_id = $1) as accounts,

            -- Spaces
            (SELECT COUNT(*) FROM sites
                WHERE community_id = $1) as sites,
            (SELECT COUNT(*) FROM spaces s
                JOIN sites st ON s.site_id = st.id
                WHERE st.community_id = $1) as spaces,

            -- Auctions
            (SELECT COUNT(*) FROM auctions a
                JOIN sites st ON a.site_id = st.id
                WHERE st.community_id = $1) as auctions,
            (SELECT COUNT(*) FROM auction_rounds ar
                JOIN auctions a ON ar.auction_id = a.id
                JOIN sites st ON a.site_id = st.id
                WHERE st.community_id = $1) as auction_rounds,
            (SELECT COUNT(*) FROM bids b
                JOIN auction_rounds ar ON b.round_id = ar.id
                JOIN auctions a ON ar.auction_id = a.id
                JOIN sites st ON a.site_id = st.id
                WHERE st.community_id = $1) as bids,
            (SELECT COUNT(*) FROM round_space_results rsr
                JOIN auction_rounds ar ON rsr.round_id = ar.id
                JOIN auctions a ON ar.auction_id = a.id
                JOIN sites st ON a.site_id = st.id
                WHERE st.community_id = $1) as round_space_results,
            (SELECT COUNT(*) FROM user_eligibilities ue
                JOIN auction_rounds ar ON ue.round_id = ar.id
                JOIN auctions a ON ar.auction_id = a.id
                JOIN sites st ON a.site_id = st.id
                WHERE st.community_id = $1) as user_eligibilities,

            -- Transactions
            (SELECT COUNT(*) FROM journal_entries
                WHERE community_id = $1) as journal_entries,
            (SELECT COUNT(*) FROM journal_lines jl
                JOIN journal_entries je ON jl.entry_id = je.id
                WHERE je.community_id = $1) as journal_lines
        "#,
    )
    .bind(community_id)
    .fetch_one(pool)
    .await
    .context("Failed to calculate storage usage")
    .map_err(StoreError::from)
}

/// Get storage usage for a community.
/// Requires coleader+ permissions.
pub async fn get_storage_usage(
    pool: &PgPool,
    time_source: &TimeSource,
    actor: &ValidatedMember,
) -> Result<StorageUsage, StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    let community_id = actor.0.community_id;
    let now = time_source.now();

    let counts = fetch_row_counts(pool, community_id).await?;
    let usage = calculate_storage_usage(&counts, now);

    // Refresh the cache with the newly calculated value
    update_storage_cache(pool, community_id, &usage, now).await?;

    Ok(usage)
}

/// Get cached storage usage for a community, if available.
pub async fn get_cached_storage_usage(
    pool: &PgPool,
    community_id: CommunityId,
) -> Result<Option<StorageUsage>, StoreError> {
    sqlx::query_as::<_, StorageUsage>(
        r#"
        SELECT image_bytes, member_bytes, space_bytes,
               auction_bytes, transaction_bytes, calculated_at
        FROM community_storage_usage
        WHERE community_id = $1
        "#,
    )
    .bind(community_id)
    .fetch_optional(pool)
    .await
    .context("Failed to get cached storage usage")
    .map_err(StoreError::from)
}

/// Get subscription tier for a community, defaulting to Free.
/// Active and past_due subscriptions get paid access; canceled
/// subscriptions and missing rows are free.
pub async fn get_subscription_tier(
    pool: &PgPool,
    community_id: CommunityId,
) -> Result<SubscriptionTier, StoreError> {
    let row = sqlx::query_as::<_, (DbSubscriptionTier, SubscriptionStatus)>(
        "SELECT tier, status FROM community_subscriptions \
         WHERE community_id = $1",
    )
    .bind(community_id)
    .fetch_optional(pool)
    .await
    .context("Failed to get subscription tier")?;

    Ok(match row {
        Some((DbSubscriptionTier::Paid, status)) => match status {
            SubscriptionStatus::Active | SubscriptionStatus::PastDue => {
                SubscriptionTier::Paid
            }
            SubscriptionStatus::Canceled | SubscriptionStatus::Unpaid => {
                SubscriptionTier::Free
            }
        },
        None => SubscriptionTier::Free,
    })
}

/// Get subscription info for a community, if any subscription
/// exists. Returns None for communities that have never subscribed.
/// Returns Some even for canceled subscriptions so the UI can
/// distinguish "never subscribed" from "was subscribed".
pub async fn get_subscription_info(
    pool: &PgPool,
    actor: &ValidatedMember,
) -> Result<Option<SubscriptionInfo>, StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }
    sqlx::query_as(
        "SELECT status, billing_interval, current_period_end, \
                cancel_at_period_end \
         FROM community_subscriptions WHERE community_id = $1",
    )
    .bind(actor.0.community_id)
    .fetch_optional(pool)
    .await
    .context("Failed to get subscription info")
    .map_err(StoreError::from)
}

/// Percentage of storage limit at or above which we bypass cache and
/// recalculate from the database.
const NEAR_LIMIT_THRESHOLD_PERCENT: f64 = 90.0;

/// Check if a community has enough storage for an operation.
/// Returns Ok if the operation can proceed, or StorageLimitExceeded if not.
///
/// Uses smart caching: recalculates from database if usage is ≥90% of limit,
/// otherwise uses cached value for performance.
pub async fn check_storage_limit(
    pool: &PgPool,
    time_source: &TimeSource,
    community_id: CommunityId,
    estimated_additional_bytes: i64,
) -> Result<(), StoreError> {
    let (usage, limits) =
        get_storage_usage_for_enforcement(pool, time_source, community_id)
            .await?;

    let current_total = usage.total_bytes();
    let estimated_total = current_total + estimated_additional_bytes;

    if estimated_total > limits.storage_bytes {
        return Err(StoreError::StorageLimitExceeded {
            current: current_total,
            limit: limits.storage_bytes,
            estimated_size_after_operation: estimated_total,
        });
    }

    Ok(())
}

/// Get storage usage and tier limits for enforcement.
/// Uses smart caching: bypasses cache if usage is ≥90% of
/// limit. Returns both usage and limits to avoid redundant
/// tier lookups.
async fn get_storage_usage_for_enforcement(
    pool: &PgPool,
    time_source: &TimeSource,
    community_id: CommunityId,
) -> Result<(StorageUsage, TierLimits), StoreError> {
    let tier = get_subscription_tier(pool, community_id).await?;
    let limits = TierLimits::for_tier(tier);

    // First, try to get cached usage
    let cached = get_cached_storage_usage(pool, community_id).await?;

    // Check if we should bypass the cache
    let should_recalculate = match &cached {
        None => true,
        Some(usage) => {
            let usage_percent = (usage.total_bytes() as f64
                / limits.storage_bytes as f64)
                * 100.0;
            usage_percent >= NEAR_LIMIT_THRESHOLD_PERCENT
        }
    };

    if should_recalculate {
        // Recalculate from database and update cache
        let now = time_source.now();
        let counts = fetch_row_counts(pool, community_id).await?;
        let usage = calculate_storage_usage(&counts, now);
        update_storage_cache(pool, community_id, &usage, now).await?;
        Ok((usage, limits))
    } else {
        // Use cached value
        Ok((cached.unwrap(), limits))
    }
}

/// Update the cached storage usage in the database.
async fn update_storage_cache(
    pool: &PgPool,
    community_id: CommunityId,
    usage: &StorageUsage,
    now: jiff::Timestamp,
) -> Result<(), StoreError> {
    sqlx::query(
        r#"
        INSERT INTO community_storage_usage (
            community_id, image_bytes, member_bytes, space_bytes,
            auction_bytes, transaction_bytes, calculated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (community_id) DO UPDATE SET
            image_bytes = EXCLUDED.image_bytes,
            member_bytes = EXCLUDED.member_bytes,
            space_bytes = EXCLUDED.space_bytes,
            auction_bytes = EXCLUDED.auction_bytes,
            transaction_bytes = EXCLUDED.transaction_bytes,
            calculated_at = EXCLUDED.calculated_at
        "#,
    )
    .bind(community_id)
    .bind(usage.image_bytes)
    .bind(usage.member_bytes)
    .bind(usage.space_bytes)
    .bind(usage.auction_bytes)
    .bind(usage.transaction_bytes)
    .bind(now.to_sqlx())
    .execute(pool)
    .await
    .context("Failed to update cached storage usage")?;

    Ok(())
}

/// Update cached storage after an image operation (create or delete).
/// If no cache exists, calculates full storage. Otherwise, just updates
/// image_bytes to keep estimates accurate until the next scheduled refresh.
///
/// - `size_delta`: positive for image creation, negative for deletion
pub async fn update_cached_storage_after_image_op(
    pool: &PgPool,
    time_source: &TimeSource,
    community_id: CommunityId,
    size_delta: i64,
) -> Result<(), StoreError> {
    // Check if cache already exists
    let cache_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM community_storage_usage WHERE \
         community_id = $1)",
    )
    .bind(community_id)
    .fetch_one(pool)
    .await
    .context("Failed to check if cache exists")?;

    if !cache_exists {
        // No cache yet - calculate full storage on first operation
        let now = time_source.now();
        let counts = fetch_row_counts(pool, community_id).await?;
        let usage = calculate_storage_usage(&counts, now);
        update_storage_cache(pool, community_id, &usage, now).await?;
    } else {
        // Cache exists - just update image_bytes with delta
        sqlx::query(
            "UPDATE community_storage_usage \
             SET image_bytes = image_bytes + $2 \
             WHERE community_id = $1",
        )
        .bind(community_id)
        .bind(size_delta)
        .execute(pool)
        .await
        .context("Failed to update cached storage after image operation")?;
    }

    Ok(())
}

/// Refresh storage usage for communities that need it (background job).
/// Refreshes communities that either:
/// - Don't have a cached value yet
/// - Have a cache older than 1 hour
///
/// Uses an advisory lock to prevent concurrent execution.
pub async fn refresh_all_community_storage(
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<RefreshStats, StoreError> {
    // Hold a transaction for the duration of the function so
    // that the advisory lock is automatically released when it
    // ends (on commit, rollback, or drop).
    let mut tx = pool.begin().await.context("Failed to begin transaction")?;

    let lock_acquired: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_xact_lock(
            hashtext('storage_refresh_all_communities')
        )",
    )
    .fetch_one(&mut *tx)
    .await
    .context("Failed to acquire advisory lock")?;

    if !lock_acquired {
        tracing::trace!(
            "Storage refresh already in progress, skipping this tick"
        );
        return Ok(RefreshStats {
            total: 0,
            success_count: 0,
            error_count: 0,
        });
    }

    let now = time_source.now();
    let one_hour_ago = now
        .checked_sub(jiff::SignedDuration::from_secs(3600))
        .context("Failed to calculate one hour ago")?;

    // Get community IDs that need refresh:
    // - No cached value yet, OR
    // - Cached value is older than 1 hour
    let community_ids = sqlx::query_scalar::<_, CommunityId>(
        "SELECT c.id FROM communities c
             WHERE NOT EXISTS(
                SELECT 1 FROM community_storage_usage cu
                WHERE cu.community_id = c.id
                  AND cu.calculated_at > $1
             )",
    )
    .bind(one_hour_ago.to_sqlx())
    .fetch_all(pool)
    .await
    .context("Failed to fetch communities needing storage refresh")?;

    let total = community_ids.len();
    let mut success_count = 0;
    let mut error_count = 0;

    for community_id in community_ids {
        match fetch_row_counts(pool, community_id).await {
            Ok(counts) => {
                let usage = calculate_storage_usage(&counts, now);
                if let Err(e) =
                    update_storage_cache(pool, community_id, &usage, now).await
                {
                    tracing::warn!(
                        community_id = %community_id,
                        error = %e,
                        "Failed to update storage cache"
                    );
                    error_count += 1;
                } else {
                    success_count += 1;
                }
            }
            Err(e) => {
                tracing::warn!(
                    community_id = %community_id,
                    error = %e,
                    "Failed to fetch row counts for storage refresh"
                );
                error_count += 1;
            }
        }
    }

    // Commit the transaction, releasing the advisory lock.
    tx.commit()
        .await
        .context("Failed to commit refresh transaction")?;

    if total > 0 {
        tracing::info!(
            total = total,
            success = success_count,
            errors = error_count,
            "Completed storage usage refresh for all communities"
        );
    }

    Ok(RefreshStats {
        total,
        success_count,
        error_count,
    })
}

/// Statistics from a storage refresh operation.
#[derive(Debug)]
pub struct RefreshStats {
    pub total: usize,
    pub success_count: usize,
    pub error_count: usize,
}

/// Cancel any active or past-due Stripe subscription for a
/// community. No-op if the community has no subscription or
/// the subscription is already canceled.
pub async fn cancel_subscription_if_active(
    pool: &PgPool,
    stripe_service: &StripeService,
    community_id: &CommunityId,
) -> Result<(), super::StoreError> {
    let sub: Option<(String, SubscriptionStatus)> = sqlx::query_as(
        "SELECT stripe_subscription_id, status \
         FROM community_subscriptions \
         WHERE community_id = $1",
    )
    .bind(community_id)
    .fetch_optional(pool)
    .await
    .context("Failed to look up subscription for cancellation")?;

    if let Some((sub_id, status)) = sub
        && status != SubscriptionStatus::Canceled
    {
        stripe_service
            .cancel_subscription(&sub_id)
            .await
            .map_err(|e| super::StoreError::StripeError(e.to_string()))?;
        tracing::info!(
            %community_id,
            stripe_subscription_id = %sub_id,
            "Canceled Stripe subscription"
        );
    }

    Ok(())
}

/// Create a Stripe Checkout session for upgrading a community.
/// Requires coleader+ permissions.
pub async fn create_checkout_session(
    pool: &PgPool,
    stripe_service: &StripeService,
    app_config: &AppConfig,
    actor: &ValidatedMember,
    billing_interval: BillingInterval,
) -> Result<String, StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    let community_id = actor.0.community_id;

    // Block checkout if there's an active or past_due subscription.
    // Past_due means Stripe is retrying payment — the user should
    // update their payment method via the portal, not start a new
    // subscription.
    let status: Option<SubscriptionStatus> = sqlx::query_scalar(
        "SELECT status FROM community_subscriptions \
         WHERE community_id = $1",
    )
    .bind(community_id)
    .fetch_optional(pool)
    .await
    .context("Failed to check subscription status")?;

    match status {
        Some(SubscriptionStatus::Active) => {
            return Err(StoreError::AlreadySubscribed);
        }
        Some(SubscriptionStatus::PastDue) => {
            return Err(StoreError::SubscriptionPastDue);
        }
        Some(SubscriptionStatus::Unpaid) => {
            return Err(StoreError::SubscriptionPastDue);
        }
        Some(SubscriptionStatus::Canceled) | None => {
            // OK to proceed
        }
    }

    // Reuse existing Stripe customer or create a new one
    let (community_name, existing_customer_id): (String, Option<String>) =
        sqlx::query_as(
            "SELECT name, stripe_customer_id \
             FROM communities WHERE id = $1",
        )
        .bind(community_id)
        .fetch_one(pool)
        .await
        .context("Failed to get community")
        .map_err(StoreError::from)?;

    let customer_id: stripe::CustomerId = match existing_customer_id {
        Some(id) => id
            .parse()
            .context("Invalid stored stripe_customer_id")
            .map_err(StoreError::from)?,
        None => {
            let id = stripe_service
                .create_customer(&community_name, &community_id)
                .await
                .map_err(|e| StoreError::StripeError(format!("{e:#}")))?;

            // Persist immediately. WHERE stripe_customer_id IS
            // NULL guards against a concurrent checkout race.
            let rows = sqlx::query(
                "UPDATE communities \
                 SET stripe_customer_id = $1 \
                 WHERE id = $2 AND stripe_customer_id IS NULL",
            )
            .bind(id.as_str())
            .bind(community_id)
            .execute(pool)
            .await
            .context("Failed to persist stripe customer ID")?;

            if rows.rows_affected() == 0 {
                // Another request won the race — use their ID
                let winner: Option<String> = sqlx::query_scalar(
                    "SELECT stripe_customer_id \
                     FROM communities WHERE id = $1",
                )
                .bind(community_id)
                .fetch_one(pool)
                .await
                .context("Failed to read winning customer ID")?;
                winner
                    .ok_or_else(|| {
                        StoreError::StripeError(
                            "Race: stripe_customer_id still NULL".into(),
                        )
                    })?
                    .parse()
                    .context("Invalid stored stripe_customer_id")
                    .map_err(StoreError::from)?
            } else {
                tracing::info!(
                    %community_id,
                    customer_id = id.as_str(),
                    "Stripe customer ID persisted on community"
                );
                id
            }
        }
    };

    // Determine price ID
    let price_id = match billing_interval {
        BillingInterval::Month => &app_config.stripe_monthly_price_id,
        BillingInterval::Year => &app_config.stripe_annual_price_id,
    };

    // Build redirect URLs
    let success_url = format!(
        "{}/communities/{}/billing?checkout=success",
        app_config.base_url, community_id,
    );
    let cancel_url = format!(
        "{}/communities/{}/billing?checkout=canceled",
        app_config.base_url, community_id,
    );

    // Create Checkout session
    let checkout_url = stripe_service
        .create_checkout_session(
            &customer_id,
            price_id,
            &community_id,
            &success_url,
            &cancel_url,
        )
        .await
        .map_err(|e| StoreError::StripeError(format!("{e:#}")))?;

    Ok(checkout_url)
}

/// Create a Stripe Billing Portal session for managing a
/// subscription. Requires coleader+ permissions.
pub async fn create_portal_session(
    pool: &PgPool,
    stripe_service: &StripeService,
    app_config: &AppConfig,
    actor: &ValidatedMember,
) -> Result<String, StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    let community_id = actor.0.community_id;

    let stripe_customer_id: Option<String> = sqlx::query_scalar(
        "SELECT stripe_customer_id \
         FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_one(pool)
    .await
    .context("Failed to get stripe customer ID")?;

    let stripe_customer_id =
        stripe_customer_id.ok_or(StoreError::NoSubscriptionFound)?;

    let return_url = format!(
        "{}/communities/{}/billing",
        app_config.base_url, community_id,
    );

    let portal_url = stripe_service
        .create_portal_session(&stripe_customer_id, &return_url)
        .await
        .map_err(|e| StoreError::StripeError(format!("{e:#}")))?;

    Ok(portal_url)
}

/// Handle a Stripe webhook event by updating subscription
/// state. Parses from raw JSON to avoid coupling to
/// async-stripe's API-version-specific type definitions.
pub async fn handle_webhook_event(
    pool: &PgPool,
    time_source: &TimeSource,
    stripe_service: &StripeService,
    event: &serde_json::Value,
) -> Result<(), StoreError> {
    let now = time_source.now();

    let event_type = event["type"].as_str().unwrap_or("unknown");
    let obj = &event["data"]["object"];

    match event_type {
        "customer.subscription.created"
        | "customer.subscription.updated"
        | "customer.subscription.deleted" => {
            handle_subscription_upsert(pool, stripe_service, obj, now).await?;
        }
        _ => {
            tracing::trace!(
                event_type,
                "Ignoring unhandled webhook event type"
            );
        }
    }

    Ok(())
}

/// Helper to extract a string field from a JSON value.
fn json_str<'a>(
    v: &'a serde_json::Value,
    field: &str,
) -> Result<&'a str, StoreError> {
    v[field].as_str().ok_or_else(|| {
        StoreError::StripeError(format!("Missing or non-string field: {field}"))
    })
}

/// Extract billing interval from a subscription object.
/// Tries price.recurring.interval (current), then
/// plan.interval (legacy).
fn subscription_interval(sub: &serde_json::Value) -> Option<&str> {
    let item = &sub["items"]["data"][0];
    item["price"]["recurring"]["interval"]
        .as_str()
        .or_else(|| item["plan"]["interval"].as_str())
}

/// Extract current_period_start from a subscription.
/// Pre-Basil: top-level. Basil+: on items.data[0].
fn subscription_period_start(sub: &serde_json::Value) -> Option<i64> {
    sub["current_period_start"]
        .as_i64()
        .or_else(|| sub["items"]["data"][0]["current_period_start"].as_i64())
}

/// Extract current_period_end from a subscription.
/// Pre-Basil: top-level. Basil+: on items.data[0].
fn subscription_period_end(sub: &serde_json::Value) -> Option<i64> {
    sub["current_period_end"]
        .as_i64()
        .or_else(|| sub["items"]["data"][0]["current_period_end"].as_i64())
}

/// Handle customer.subscription.created or .updated.
/// Upserts the subscription row. If no row exists yet
/// (first event for this subscription), fetches the
/// community_id from the Stripe customer's metadata.
async fn handle_subscription_upsert(
    pool: &PgPool,
    stripe_service: &StripeService,
    sub: &serde_json::Value,
    now: jiff::Timestamp,
) -> Result<(), StoreError> {
    let sub_id = json_str(sub, "id")?;
    let status_str = json_str(sub, "status")?;

    let status = match map_stripe_status(status_str)? {
        Some(s) => s,
        None => {
            tracing::trace!(
                stripe_subscription_id = sub_id,
                status = status_str,
                "Skipping transient subscription status"
            );
            return Ok(());
        }
    };

    let now_sqlx = now.to_sqlx();

    let billing_interval = match subscription_interval(sub) {
        Some("month") => BillingInterval::Month,
        Some("year") => BillingInterval::Year,
        other => {
            return Err(StoreError::StripeError(format!(
                "Unexpected billing interval: {other:?}"
            )));
        }
    };

    let cancel_at_period_end =
        sub["cancel_at_period_end"].as_bool().unwrap_or(false);

    let canceled_at = sub["canceled_at"]
        .as_i64()
        .map(|ts| {
            jiff::Timestamp::from_second(ts).map_err(|e| {
                StoreError::StripeError(format!(
                    "Invalid canceled_at {ts}: {e}"
                ))
            })
        })
        .transpose()?;
    let canceled_at_sqlx = canceled_at.map(|t| t.to_sqlx());

    let period_start_ts = subscription_period_start(sub).ok_or_else(|| {
        StoreError::StripeError("Missing current_period_start".into())
    })?;
    let period_start =
        jiff::Timestamp::from_second(period_start_ts).map_err(|e| {
            StoreError::StripeError(format!(
                "Invalid current_period_start: {e}"
            ))
        })?;

    let period_end_ts = subscription_period_end(sub).ok_or_else(|| {
        StoreError::StripeError("Missing current_period_end".into())
    })?;
    let period_end =
        jiff::Timestamp::from_second(period_end_ts).map_err(|e| {
            StoreError::StripeError(format!("Invalid current_period_end: {e}"))
        })?;

    // Try to update an existing row first
    let rows = sqlx::query(
        "UPDATE community_subscriptions SET
            status = $2,
            billing_interval = $3,
            current_period_start = $4,
            current_period_end = $5,
            cancel_at_period_end = $6,
            canceled_at = $7,
            updated_at = $8
        WHERE stripe_subscription_id = $1",
    )
    .bind(sub_id)
    .bind(status)
    .bind(billing_interval)
    .bind(period_start.to_sqlx())
    .bind(period_end.to_sqlx())
    .bind(cancel_at_period_end)
    .bind(canceled_at_sqlx)
    .bind(now_sqlx)
    .execute(pool)
    .await
    .context("Failed to update subscription")?;

    if rows.rows_affected() > 0 {
        tracing::info!(
            stripe_subscription_id = sub_id,
            ?status,
            "Subscription updated"
        );
        return Ok(());
    }

    // No existing row — this is a new subscription.
    // Look up community_id from the Stripe customer.
    let customer_id = sub["customer"]
        .as_str()
        .or_else(|| sub["customer"]["id"].as_str())
        .ok_or_else(|| {
            StoreError::StripeError("Subscription missing customer".into())
        })?;

    let community_id = stripe_service
        .get_customer_community_id(customer_id)
        .await
        .map_err(|e| StoreError::StripeError(format!("{e:#}")))?;

    // If the community has been deleted, this webhook is for
    // a subscription we already canceled during deletion.
    let community_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM communities WHERE id = $1)",
    )
    .bind(community_id)
    .fetch_one(pool)
    .await
    .context("Failed to check community existence")?;

    if !community_exists {
        tracing::info!(
            stripe_subscription_id = sub_id,
            %community_id,
            "Ignoring subscription webhook for deleted community"
        );
        return Ok(());
    }

    // Only upsert if the existing row is canceled/unpaid or
    // has the same subscription ID. Prevents a stale event
    // from an old subscription overwriting a newer one during
    // resubscription (Stripe doesn't guarantee ordering).
    let rows = sqlx::query(
        "INSERT INTO community_subscriptions (
            community_id, tier, status, billing_interval,
            stripe_subscription_id,
            current_period_start, current_period_end,
            cancel_at_period_end, canceled_at,
            created_at, updated_at
        ) VALUES (
            $1, 'paid', $2, $3, $4,
            $5, $6, $7, $8, $9, $9
        )
        ON CONFLICT (community_id) DO UPDATE SET
            tier = 'paid',
            status = EXCLUDED.status,
            billing_interval = EXCLUDED.billing_interval,
            stripe_subscription_id = EXCLUDED.stripe_subscription_id,
            current_period_start = EXCLUDED.current_period_start,
            current_period_end = EXCLUDED.current_period_end,
            cancel_at_period_end = EXCLUDED.cancel_at_period_end,
            canceled_at = EXCLUDED.canceled_at,
            updated_at = EXCLUDED.updated_at
        WHERE community_subscriptions.status IN ('canceled', 'unpaid')
           OR community_subscriptions.stripe_subscription_id = $4",
    )
    .bind(community_id)
    .bind(status)
    .bind(billing_interval)
    .bind(sub_id)
    .bind(period_start.to_sqlx())
    .bind(period_end.to_sqlx())
    .bind(cancel_at_period_end)
    .bind(canceled_at_sqlx)
    .bind(now_sqlx)
    .execute(pool)
    .await
    .context("Failed to insert subscription")?;

    if rows.rows_affected() == 0 {
        tracing::warn!(
            stripe_subscription_id = sub_id,
            %community_id,
            "Skipped stale subscription event \
             (community has a newer subscription)"
        );
        return Ok(());
    }

    tracing::info!(
        %community_id,
        stripe_subscription_id = sub_id,
        "Subscription created"
    );
    Ok(())
}

/// Map Stripe's subscription status string to our DB status.
/// Returns None for transient statuses (incomplete,
/// incomplete_expired, paused) that we don't persist.
fn map_stripe_status(
    status: &str,
) -> Result<Option<SubscriptionStatus>, StoreError> {
    match status {
        "active" | "trialing" => Ok(Some(SubscriptionStatus::Active)),
        "past_due" => Ok(Some(SubscriptionStatus::PastDue)),
        "canceled" => Ok(Some(SubscriptionStatus::Canceled)),
        "unpaid" => Ok(Some(SubscriptionStatus::Unpaid)),
        // Transient states — don't persist
        "incomplete" | "incomplete_expired" | "paused" => Ok(None),
        other => Err(StoreError::StripeError(format!(
            "Unexpected subscription status: {other}"
        ))),
    }
}
