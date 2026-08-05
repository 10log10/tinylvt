//! Member card setup tests (phase 4): platform customer creation and
//! reuse, card adoption via both the webhook and the return-redirect
//! poll path, replace-detaches-old, removal gating, and the hold
//! strategy setting.

use api::store;
use api::stripe_service::CardPaymentMethod;
use payloads::{ApiError, UserId, requests};
use serde_json::json;
use test_helpers::{TestApp, assert_api_error, spawn_app};

fn visa(id: &str) -> CardPaymentMethod {
    CardPaymentMethod {
        id: id.to_string(),
        brand: "visa".to_string(),
        last4: "4242".to_string(),
        exp_month: 12,
        exp_year: 2030,
    }
}

async fn alice_user_id(
    app: &TestApp,
    community_id: payloads::CommunityId,
) -> anyhow::Result<UserId> {
    let members = app.client.get_members(&community_id).await?;
    Ok(members
        .iter()
        .find(|m| m.user.username == "alice")
        .unwrap()
        .user
        .user_id)
}

/// First setup creates and persists the platform customer; later
/// sessions reuse it. The return-redirect poll (get_payment_profile)
/// adopts the card without any webhook, and a replace adopts the newer
/// card and detaches the old one.
#[tokio::test]
async fn card_setup_and_replace_lifecycle() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;

    let response = app.client.create_card_setup_session().await?;
    assert!(response.checkout_url.contains("cus_user_mock_1"));
    app.client.create_card_setup_session().await?;
    assert_eq!(
        app.stripe_service.mock_user_customers.lock().unwrap().len(),
        1
    );

    // No card saved yet.
    let profile = app.client.get_payment_profile().await?;
    assert_eq!(profile.card, None);

    // Checkout completes: Stripe now lists a card; the poll path
    // adopts it without a webhook.
    app.stripe_service
        .mock_customer_payment_methods
        .lock()
        .unwrap()
        .insert("cus_user_mock_1".to_string(), vec![visa("pm_first")]);
    let profile = app.client.get_payment_profile().await?;
    let card = profile.card.expect("card adopted via poll path");
    assert_eq!(card.brand, "visa");
    assert_eq!(card.last4, "4242");

    // Replace: a newer card appears first in the list; adopting it
    // detaches the old one.
    app.stripe_service
        .mock_customer_payment_methods
        .lock()
        .unwrap()
        .insert(
            "cus_user_mock_1".to_string(),
            vec![
                CardPaymentMethod {
                    id: "pm_second".to_string(),
                    brand: "mastercard".to_string(),
                    last4: "4444".to_string(),
                    exp_month: 1,
                    exp_year: 2031,
                },
                visa("pm_first"),
            ],
        );
    let profile = app.client.get_payment_profile().await?;
    assert_eq!(profile.card.unwrap().brand, "mastercard");
    assert!(
        app.stripe_service
            .mock_detached_payment_methods
            .lock()
            .unwrap()
            .contains(&"pm_first".to_string())
    );

    Ok(())
}

/// The platform webhook's checkout.session.completed (mode=setup)
/// adopts the card through the shared sync routine.
#[tokio::test]
async fn setup_webhook_adopts_card() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    let user_id = alice_user_id(&app, community_id).await?;

    app.client.create_card_setup_session().await?;
    app.stripe_service
        .mock_customer_payment_methods
        .lock()
        .unwrap()
        .insert("cus_user_mock_1".to_string(), vec![visa("pm_hook")]);

    let event = json!({
        "type": "checkout.session.completed",
        "data": { "object": {
            "id": "cs_test_setup",
            "object": "checkout.session",
            "mode": "setup",
            "customer": "cus_user_mock_1",
            "metadata": { "user_id": user_id.to_string() },
        }},
    });
    store::billing::handle_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &event,
    )
    .await?;

    let (pm, consented): (Option<String>, bool) = sqlx::query_as(
        "SELECT payment_method_id, consented_at IS NOT NULL \
         FROM user_payment_profiles WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(pm.as_deref(), Some("pm_hook"));
    assert!(consented);

    Ok(())
}

/// Removal detaches the card in Stripe and clears the row — even while
/// the card backs live authorizations. It's the member's global escape
/// hatch from further card activity; existing holds are unaffected
/// (capture rides the authorization on the connected account's cloned
/// method, not the saved card).
#[tokio::test]
async fn remove_payment_method_with_active_intents() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    let user_id = alice_user_id(&app, community_id).await?;

    // Nothing saved yet.
    let result = app.client.remove_payment_method().await;
    assert_api_error(result, ApiError::NoSavedPaymentMethod);

    app.client.create_card_setup_session().await?;
    app.stripe_service
        .mock_customer_payment_methods
        .lock()
        .unwrap()
        .insert("cus_user_mock_1".to_string(), vec![visa("pm_gone")]);
    app.client.get_payment_profile().await?;

    // A live funding intent does not block removal.
    let site = app.create_test_site(&community_id).await?;
    let auction = app.create_test_auction(&site.site_id).await?;
    sqlx::query(
        "INSERT INTO funding_intents \
         (auction_id, user_id, payment_intent_id, is_active, status, \
          origin, requested_amount, authorized_amount, capture_before, \
          created_at, updated_at) \
         VALUES ($1, $2, 'pi_live', TRUE, 'authorized', 'bid_flow', 5, 5, \
          NOW() + INTERVAL '6 days', NOW(), NOW())",
    )
    .bind(auction.auction_id)
    .bind(user_id)
    .execute(&app.db_pool)
    .await?;
    app.client.remove_payment_method().await?;
    assert!(
        app.stripe_service
            .mock_detached_payment_methods
            .lock()
            .unwrap()
            .contains(&"pm_gone".to_string())
    );
    let profile = app.client.get_payment_profile().await?;
    assert_eq!(profile.card, None);

    // The intent row is untouched — the hold it mirrors remains live
    // and capturable.
    let status: String =
        sqlx::query_scalar("SELECT status::TEXT FROM funding_intents")
            .fetch_one(&app.db_pool)
            .await?;
    assert_eq!(status, "authorized");

    Ok(())
}

/// Removal's row clear is a compare-and-clear on the method it
/// detached. A card adoption landing between remove's read and its
/// write (the profile sync runs on every view, so this interleaving is
/// ordinary) must not have its row wiped: a blind clear keyed on
/// user_id alone would drop the new card locally while it stayed
/// attached at Stripe, and the next sync would silently re-adopt it —
/// Remove appearing to succeed, then undoing itself.
#[tokio::test]
async fn remove_does_not_clear_a_concurrently_adopted_card()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;

    app.client.create_card_setup_session().await?;
    app.stripe_service
        .mock_customer_payment_methods
        .lock()
        .unwrap()
        .insert("cus_user_mock_1".to_string(), vec![visa("pm_old")]);
    app.client.get_payment_profile().await?;

    // Hold the profile row so remove's clear parks between its read
    // (pm_old) and its write — the real race window.
    let mut tx = app.db_pool.begin().await?;
    sqlx::query("SELECT user_id FROM user_payment_profiles FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;

    let remove_fut = app.client.remove_payment_method();
    let adopt_fut = async {
        // Commit the racing adoption only once remove is observably
        // parked at its clear — after it read pm_old and detached it.
        app.wait_for_lock_waiter("%SET payment_method_id = NULL%")
            .await?;
        sqlx::query(
            "UPDATE user_payment_profiles \
             SET payment_method_id = 'pm_new', card_brand = 'mastercard', \
                 card_last4 = '4444', card_exp_month = 1, \
                 card_exp_year = 2031",
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        anyhow::Ok(())
    };

    // Remove's clear targets pm_old (what it read); the CAS misses.
    let (remove_res, adopt_res) = tokio::join!(remove_fut, adopt_fut);
    adopt_res?;
    remove_res?;

    // pm_old is detached at Stripe — that is what the member asked to
    // remove — but the newly adopted card survives locally.
    assert!(
        app.stripe_service
            .mock_detached_payment_methods
            .lock()
            .unwrap()
            .contains(&"pm_old".to_string())
    );
    let pm: Option<String> = sqlx::query_scalar(
        "SELECT payment_method_id FROM user_payment_profiles",
    )
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(pm.as_deref(), Some("pm_new"));

    Ok(())
}

/// Concurrent syncs (webhook and profile view, or two views) can list
/// different "newest" cards mid-replacement. The adoption is a
/// compare-and-swap on the previously stored method, so the loser
/// writes nothing rather than overwriting the winner — and skips the
/// detach, leaving the winner to retire the shared predecessor.
#[tokio::test]
async fn stale_card_sync_does_not_overwrite_the_winner() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;

    app.client.create_card_setup_session().await?;
    app.stripe_service
        .mock_customer_payment_methods
        .lock()
        .unwrap()
        .insert("cus_user_mock_1".to_string(), vec![visa("pm_old")]);
    app.client.get_payment_profile().await?;

    // The loser's list call reports pm_lose as newest; its adoption
    // decision will be made against stored = pm_old.
    app.stripe_service
        .mock_customer_payment_methods
        .lock()
        .unwrap()
        .insert(
            "cus_user_mock_1".to_string(),
            vec![
                CardPaymentMethod {
                    id: "pm_lose".to_string(),
                    brand: "amex".to_string(),
                    last4: "0005".to_string(),
                    exp_month: 6,
                    exp_year: 2032,
                },
                visa("pm_old"),
            ],
        );

    // Hold the profile row so the loser's persist parks between its
    // read of stored (pm_old) and its write — the real race window.
    let mut tx = app.db_pool.begin().await?;
    sqlx::query("SELECT user_id FROM user_payment_profiles FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;

    let loser_fut = app.client.get_payment_profile();
    let winner_fut = async {
        // The winner's adoption of pm_win commits only once the loser
        // is observably parked at its persist, having decided against
        // the about-to-be-replaced pm_old.
        app.wait_for_lock_waiter("%SET payment_method_id = $1%")
            .await?;
        sqlx::query(
            "UPDATE user_payment_profiles \
             SET payment_method_id = 'pm_win', card_brand = 'mastercard', \
                 card_last4 = '4444', card_exp_month = 1, \
                 card_exp_year = 2031",
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        anyhow::Ok(())
    };
    let (profile_res, winner_res) = tokio::join!(loser_fut, winner_fut);
    winner_res?;
    let profile = profile_res?;

    // The winner's card stands, and the loser detached nothing — a
    // detach of pm_old here would race the winner's own adoption.
    assert_eq!(profile.card.clone().unwrap().brand, "mastercard");
    assert_eq!(profile.card.unwrap().last4, "4444");
    assert!(
        app.stripe_service
            .mock_detached_payment_methods
            .lock()
            .unwrap()
            .is_empty()
    );

    Ok(())
}

/// The hold strategy defaults to budget holds and round-trips through
/// the update endpoint.
#[tokio::test]
async fn hold_strategy_round_trip() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;

    let profile = app.client.get_payment_profile().await?;
    assert!(profile.budget_holds);

    app.client
        .update_hold_strategy(&requests::UpdateHoldStrategy {
            budget_holds: false,
        })
        .await?;
    let profile = app.client.get_payment_profile().await?;
    assert!(!profile.budget_holds);

    Ok(())
}

/// The per-community card charge grant reads back through the getter,
/// round-tripping grant and revoke.
#[tokio::test]
async fn card_charge_grant_round_trip() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    app.login_bob().await?;
    assert!(!app.client.get_card_charge_grant(&community_id).await?);

    app.client
        .update_card_charge_grant(&requests::UpdateCardChargeGrant {
            community_id,
            grant: requests::ChargeGrant::Granted,
        })
        .await?;
    assert!(app.client.get_card_charge_grant(&community_id).await?);

    app.client
        .update_card_charge_grant(&requests::UpdateCardChargeGrant {
            community_id,
            grant: requests::ChargeGrant::Revoked,
        })
        .await?;
    assert!(!app.client.get_card_charge_grant(&community_id).await?);
    Ok(())
}
