//! Stripe Connect onboarding tests (phase 3): account creation and
//! reuse, mode/permission gating, the status endpoint's live-state
//! reconciliation and currency-mismatch surfacing, and the Connect
//! webhook handler's charges-enabled/deauthorization convergence
//! (applying live account state, since retried payloads can be stale).

use api::store;
use api::stripe_service::AccountStatus;
use payloads::{
    ApiError, CommunityId, PurchaseKind, StripeConnectStatus, requests,
};
use rust_decimal::dec;
use serde_json::json;
use test_helpers::{TestApp, assert_api_error, spawn_app};

async fn backed_community(app: &TestApp) -> anyhow::Result<CommunityId> {
    let community_id = app.create_two_person_community().await?;
    app.set_backed_credits_mode(&community_id).await?;
    Ok(community_id)
}

/// Set the mock's live status for the first created connected account.
fn set_mock_account_status(
    app: &TestApp,
    charges_enabled: bool,
    currency: &str,
) {
    app.stripe_service
        .mock_account_status
        .lock()
        .unwrap()
        .insert(
            "acct_mock_1".to_string(),
            AccountStatus {
                charges_enabled,
                details_submitted: true,
                default_currency: Some(currency.to_string()),
            },
        );
}

async fn stripe_account_row(
    app: &TestApp,
    community_id: CommunityId,
) -> anyhow::Result<(Option<String>, bool, bool)> {
    let row: (Option<String>, bool, bool) = sqlx::query_as(
        "SELECT stripe_account_id, stripe_charges_enabled, \
         stripe_deauthorized_at IS NOT NULL \
         FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_one(&app.db_pool)
    .await?;
    Ok(row)
}

#[tokio::test]
async fn connect_requires_backed_mode() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    // Default test community mode is distributed_clearing.
    let community_id = app.create_test_community().await?;

    let result = app.client.connect_community_stripe(&community_id).await;
    assert_api_error(result, ApiError::StripeConnectRequiresBackedCredits);

    Ok(())
}

#[tokio::test]
async fn connect_requires_coleader() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = backed_community(&app).await?;

    app.login_bob().await?;
    let result = app.client.connect_community_stripe(&community_id).await;
    assert_api_error(result, ApiError::RequiresColeaderPermissions);

    Ok(())
}

/// The first connect creates and persists the account; later connects
/// reuse it (each minting a fresh onboarding link).
#[tokio::test]
async fn connect_creates_and_reuses_account() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = backed_community(&app).await?;

    let response = app.client.connect_community_stripe(&community_id).await?;
    assert!(response.checkout_url.contains("acct_mock_1"));

    let (account_id, charges_enabled, deauthorized) =
        stripe_account_row(&app, community_id).await?;
    assert_eq!(account_id.as_deref(), Some("acct_mock_1"));
    assert!(!charges_enabled);
    assert!(!deauthorized);

    app.client.connect_community_stripe(&community_id).await?;
    let created = app
        .stripe_service
        .mock_connected_accounts
        .lock()
        .unwrap()
        .len();
    assert_eq!(created, 1);

    Ok(())
}

/// Status walks the lifecycle: not connected → onboarding incomplete →
/// charges enabled (with the DB flag reconciled from live state), and
/// surfaces a settlement-currency mismatch.
#[tokio::test]
async fn status_reflects_live_account_state() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = backed_community(&app).await?;
    let status = app
        .client
        .get_community_stripe_status(&community_id)
        .await?;
    assert_eq!(status.status, StripeConnectStatus::NotConnected);

    app.client.connect_community_stripe(&community_id).await?;
    let status = app
        .client
        .get_community_stripe_status(&community_id)
        .await?;
    assert_eq!(status.status, StripeConnectStatus::OnboardingIncomplete);
    assert_eq!(status.settlement_currency_mismatch, None);

    // Onboarding finishes: charges enabled, USD settlement.
    set_mock_account_status(&app, true, "usd");
    let status = app
        .client
        .get_community_stripe_status(&community_id)
        .await?;
    assert_eq!(status.status, StripeConnectStatus::ChargesEnabled);
    assert_eq!(status.settlement_currency_mismatch, None);
    // The status fetch reconciled the stored flag.
    let (_, charges_enabled, _) =
        stripe_account_row(&app, community_id).await?;
    assert!(charges_enabled);

    // A settlement currency that doesn't match the community's
    // denomination is surfaced.
    set_mock_account_status(&app, true, "eur");
    let status = app
        .client
        .get_community_stripe_status(&community_id)
        .await?;
    assert_eq!(status.settlement_currency_mismatch, Some("eur".to_string()));

    Ok(())
}

/// An account deleted or disconnected in Stripe is unrecoverable: the
/// status probe detects it and marks the community disconnected, and
/// the next connect request swaps in a freshly created account (with
/// onboarding state reset), retires in-flight purchases whose Checkout
/// sessions died with the account, and mints a link for the
/// replacement.
#[tokio::test]
async fn gone_account_is_replaced_on_reconnect() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = backed_community(&app).await?;
    app.client.connect_community_stripe(&community_id).await?;

    // Onboarding completes before the account disappears; the webhook
    // handler applies the live state it re-fetches.
    set_mock_account_status(&app, true, "usd");
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "account.updated",
            "account": "acct_mock_1",
            "data": { "object": {
                "id": "acct_mock_1",
                "object": "account",
                "charges_enabled": true,
                "details_submitted": true,
            }},
        }),
    )
    .await?;

    // A purchase is opened but never completed — its Checkout session
    // lives on the doomed account.
    app.client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id,
            kind: PurchaseKind::TopUp,
            amount: dec!(25),
        })
        .await?;

    // The community deletes the account in their Stripe dashboard.
    app.stripe_service
        .mock_gone_accounts
        .lock()
        .unwrap()
        .insert("acct_mock_1".to_string());

    // The status probe's failing API call is the detection signal.
    let status = app
        .client
        .get_community_stripe_status(&community_id)
        .await?;
    assert_eq!(status.status, StripeConnectStatus::Disconnected);
    let (_, _, deauthorized) = stripe_account_row(&app, community_id).await?;
    assert!(deauthorized);

    // Reconnecting replaces the gone account with a fresh one and
    // returns an onboarding link for it.
    let response = app.client.connect_community_stripe(&community_id).await?;
    assert!(response.checkout_url.contains("acct_mock_2"));
    let (account_id, charges_enabled, deauthorized) =
        stripe_account_row(&app, community_id).await?;
    assert_eq!(account_id.as_deref(), Some("acct_mock_2"));
    assert!(!charges_enabled);
    assert!(!deauthorized);

    // The stranded purchase was retired in the swap; left 'created' it
    // would block the member's future debt settlements.
    let purchase_status: String = sqlx::query_scalar(
        "SELECT status::TEXT FROM credit_purchases WHERE community_id = $1",
    )
    .bind(community_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(purchase_status, "expired");

    let status = app
        .client
        .get_community_stripe_status(&community_id)
        .await?;
    assert_eq!(status.status, StripeConnectStatus::OnboardingIncomplete);

    Ok(())
}

/// A stale status probe that read the old account before a concurrent
/// gone-account replacement must not mark the fresh account
/// deauthorized: its write is conditioned on the probed id still being
/// stored, so it no-ops after the swap.
#[tokio::test]
async fn stale_probe_does_not_deauthorize_replacement() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = backed_community(&app).await?;
    app.client.connect_community_stripe(&community_id).await?;

    // The account dies and a connect request replaces it directly (the
    // replacement path that never sets the deauthorization marker).
    app.stripe_service
        .mock_gone_accounts
        .lock()
        .unwrap()
        .insert("acct_mock_1".to_string());
    app.client.connect_community_stripe(&community_id).await?;
    let (account_id, _, deauthorized) =
        stripe_account_row(&app, community_id).await?;
    assert_eq!(account_id.as_deref(), Some("acct_mock_2"));
    assert!(!deauthorized);

    // A status probe that read acct_mock_1 before the swap now finds it
    // gone and reaches its deauthorization write.
    store::connect::set_deauthorized(
        &app.db_pool,
        &community_id,
        "acct_mock_1",
        &app.time_source,
    )
    .await?;

    // The fresh account is untouched and the status endpoint still
    // probes it rather than short-circuiting on the marker.
    let (account_id, _, deauthorized) =
        stripe_account_row(&app, community_id).await?;
    assert_eq!(account_id.as_deref(), Some("acct_mock_2"));
    assert!(!deauthorized);
    let status = app
        .client
        .get_community_stripe_status(&community_id)
        .await?;
    assert_eq!(status.status, StripeConnectStatus::OnboardingIncomplete);

    Ok(())
}

/// The Connect webhook applies live account state on account.updated
/// (a stale retried payload can't regress charges_enabled), marks
/// deauthorization, refuses to clear the marker while the account is
/// inaccessible, and clears it once a retrieve succeeds; reconnecting
/// via the connect endpoint also clears it.
#[tokio::test]
async fn connect_webhook_lifecycle() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = backed_community(&app).await?;
    app.client.connect_community_stripe(&community_id).await?;

    // The payload says charges are disabled — a stale redelivery — but
    // the live account has them enabled; the handler applies the live
    // state it re-fetches, not the payload.
    let updated_event = json!({
        "type": "account.updated",
        "account": "acct_mock_1",
        "data": { "object": {
            "id": "acct_mock_1",
            "object": "account",
            "charges_enabled": false,
            "details_submitted": true,
        }},
    });
    set_mock_account_status(&app, true, "usd");
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &updated_event,
    )
    .await?;
    let (_, charges_enabled, _) =
        stripe_account_row(&app, community_id).await?;
    assert!(charges_enabled);

    // The community disconnects the platform in their dashboard: the
    // deauthorized event sets the marker, and the account stops being
    // retrievable.
    let deauthorized_event = json!({
        "type": "account.application.deauthorized",
        "account": "acct_mock_1",
        "data": { "object": {
            "id": "ca_mock_application",
            "object": "application",
        }},
    });
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &deauthorized_event,
    )
    .await?;
    app.stripe_service
        .mock_gone_accounts
        .lock()
        .unwrap()
        .insert("acct_mock_1".to_string());
    let (_, _, deauthorized) = stripe_account_row(&app, community_id).await?;
    assert!(deauthorized);
    let status = app
        .client
        .get_community_stripe_status(&community_id)
        .await?;
    assert_eq!(status.status, StripeConnectStatus::Disconnected);

    // A stale account.updated redelivery can't clear the marker: the
    // retrieve fails for the gone account, so stored state stands.
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &updated_event,
    )
    .await?;
    let (_, _, deauthorized) = stripe_account_row(&app, community_id).await?;
    assert!(deauthorized);

    // API access restored: the successful retrieve is the proof of
    // reconnection, clearing the marker.
    app.stripe_service
        .mock_gone_accounts
        .lock()
        .unwrap()
        .remove("acct_mock_1");
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &updated_event,
    )
    .await?;
    let (_, _, deauthorized) = stripe_account_row(&app, community_id).await?;
    assert!(!deauthorized);

    // Marked disconnected again; the connect endpoint's successful
    // account-link creation also clears the marker.
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &deauthorized_event,
    )
    .await?;
    app.client.connect_community_stripe(&community_id).await?;
    let (_, _, deauthorized) = stripe_account_row(&app, community_id).await?;
    assert!(!deauthorized);

    Ok(())
}
