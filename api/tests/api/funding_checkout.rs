//! Unsaved-card checkout authorization tests: the mint's validation and
//! session parameters, completion-by-webhook activating the hold (and
//! backing bids through settlement capture), raise-by-fresh-checkout
//! superseding the prior hold, stale-session retirement before a new
//! mint, abandoned-session expiry, in-session declines leaving the row
//! retryable, and late completions the auction can no longer use being
//! released with notice.

use api::store;
use payloads::{ApiError, AuctionId, CommunityId, UserId, requests};
use rust_decimal::{Decimal, dec};
use serde_json::{Value, json};
use test_helpers::{TestApp, assert_api_error, spawn_app};

use crate::funding::{create_open_auction, credit_member, run_until_ended};
use crate::funding_auth::{bob_user_id, intent_rows};
use crate::funding_capture::member_balance;

/// Backed community with a charges-enabled mock connected account and
/// no saved card for bob — the unsaved-card flow's audience. Leaves bob
/// logged in.
async fn checkout_setup(app: &TestApp) -> anyhow::Result<CommunityId> {
    let community_id = app.create_two_person_community().await?;
    app.set_backed_credits_mode(&community_id).await?;
    sqlx::query(
        "UPDATE communities SET stripe_account_id = 'acct_mock_1', \
         stripe_charges_enabled = TRUE WHERE id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;
    app.login_bob().await?;
    Ok(community_id)
}

#[derive(Debug, sqlx::FromRow)]
struct CheckoutRow {
    id: uuid::Uuid,
    checkout_session_id: Option<String>,
    status: String,
    origin: String,
    requested_amount: Decimal,
    reconciled_at_set: bool,
}

async fn checkout_rows(
    app: &TestApp,
    auction_id: &AuctionId,
) -> anyhow::Result<Vec<CheckoutRow>> {
    Ok(sqlx::query_as(
        "SELECT id, checkout_session_id, status::TEXT, origin::TEXT, \
                requested_amount, reconciled_at IS NOT NULL \
                    AS reconciled_at_set \
         FROM funding_intents \
         WHERE auction_id = $1 AND origin = 'member_checkout' \
         ORDER BY created_at, id",
    )
    .bind(auction_id)
    .fetch_all(&app.db_pool)
    .await?)
}

/// Mint a checkout for the logged-in member and return its intent row.
async fn mint_checkout(
    app: &TestApp,
    auction_id: &AuctionId,
    amount: Decimal,
) -> anyhow::Result<CheckoutRow> {
    let response = app
        .client
        .checkout_funding(&requests::CheckoutFunding {
            auction_id: *auction_id,
            amount,
        })
        .await?;
    let rows = checkout_rows(app, auction_id).await?;
    let row = rows
        .into_iter()
        .find(|r| r.status == "checkout_created")
        .expect("mint leaves a checkout_created row");
    assert_eq!(
        response.checkout_url,
        format!("https://mock-stripe.invalid/funding/{}", row.id)
    );
    Ok(row)
}

/// Stage the member's payment of a mock session: mark it complete with
/// its PaymentIntent, and create that intent as a live manual-capture
/// hold carrying the row's metadata.
fn pay_mock_session(
    app: &TestApp,
    session_id: &str,
    pi_id: &str,
    intent_id: uuid::Uuid,
    amount_minor: i64,
) {
    {
        let mut sessions = app.stripe_service.mock_sessions.lock().unwrap();
        let session = sessions.get_mut(session_id).unwrap();
        session.status = "complete".to_string();
        session.payment_intent_id = Some(pi_id.to_string());
    }
    app.stripe_service
        .mock_payment_intents
        .lock()
        .unwrap()
        .insert(
            pi_id.to_string(),
            api::stripe_service::MockPaymentIntent {
                account_id: "acct_mock_1".to_string(),
                amount_minor,
                status: "requires_capture".to_string(),
                metadata: std::collections::HashMap::from([(
                    "funding_intent_id".to_string(),
                    intent_id.to_string(),
                )]),
                amount_received: 0,
                application_fee_minor: None,
                capture_before_epoch: Some(
                    app.time_source.now().as_second() + 7 * 24 * 3600,
                ),
                cancellation_reason: None,
                capture_idempotency_key: None,
            },
        );
}

/// The confirm webhook a paid session's PaymentIntent fires.
fn capturable_event(pi_id: &str, intent_id: uuid::Uuid, minor: i64) -> Value {
    json!({
        "type": "payment_intent.amount_capturable_updated",
        "account": "acct_mock_1",
        "data": {"object": {
            "id": pi_id,
            "amount_capturable": minor,
            "metadata": {"funding_intent_id": intent_id.to_string()},
        }},
    })
}

async fn deliver(app: &TestApp, event: &Value) -> anyhow::Result<()> {
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        event,
    )
    .await?;
    Ok(())
}

/// The full journey: a member with no saved card mints a checkout (the
/// session authorizes rather than charges, with no platform fee at
/// mint), the completion webhook activates the hold, the hold backs a
/// bid, and settlement captures from it.
#[tokio::test]
async fn checkout_full_flow() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = checkout_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(
        funding.card_available,
        payloads::responses::CardAvailability::NoSavedCard
    );
    let row = mint_checkout(&app, &auction_id, dec!(10)).await?;
    assert_eq!(row.origin, "member_checkout");
    assert_eq!(row.requested_amount, dec!(10));
    let session_id = row.checkout_session_id.clone().unwrap();
    {
        let sessions = app.stripe_service.mock_sessions.lock().unwrap();
        let session = sessions.get(&session_id).unwrap();
        assert_eq!(session.account_id, "acct_mock_1");
        assert_eq!(session.amount_minor, 1000);
        assert_eq!(session.currency, "usd");
        assert!(session.manual_capture, "the session must hold, not charge");
        assert_eq!(session.application_fee_minor, None);
    }
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.checkout_pending, Some(dec!(10)));
    assert_eq!(funding.authorized, dec!(0));

    pay_mock_session(&app, &session_id, "pi_checkout_1", row.id, 1000);
    deliver(&app, &capturable_event("pi_checkout_1", row.id, 1000)).await?;

    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.checkout_pending, None);
    assert_eq!(funding.authorized, dec!(10));
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "authorized");
    assert!(rows[0].is_active);

    // The hold backs a bid like any other authorization, and settlement
    // captures the win from it. Any platform fee is applied at capture
    // time (the session set none); with the fee suspended, none is set.
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    run_until_ended(&app, &[auction_id]).await?;
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "captured");
    {
        let intents = app.stripe_service.mock_payment_intents.lock().unwrap();
        let pi = intents.get("pi_checkout_1").unwrap();
        assert_eq!(pi.status, "succeeded");
        assert_eq!(pi.amount_received, 1000);
        assert_eq!(pi.application_fee_minor, None);
    }
    Ok(())
}

/// The `checkout.session.completed` webhook alone converges a paid
/// session (a lost confirm event still activates): the handler links
/// the session's PaymentIntent and routes the live hold to adoption.
#[tokio::test]
async fn session_completed_event_activates() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = checkout_setup(&app).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    let row = mint_checkout(&app, &auction_id, dec!(8)).await?;
    let session_id = row.checkout_session_id.clone().unwrap();
    pay_mock_session(&app, &session_id, "pi_checkout_2", row.id, 800);

    deliver(
        &app,
        &json!({
            "type": "checkout.session.completed",
            "account": "acct_mock_1",
            "data": {"object": {
                "id": session_id,
                "metadata": {"funding_intent_id": row.id.to_string()},
            }},
        }),
    )
    .await?;

    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "authorized");
    assert!(rows[0].is_active);
    assert_eq!(rows[0].authorized_amount, Some(dec!(8)));
    assert_eq!(rows[0].payment_intent_id.as_deref(), Some("pi_checkout_2"));
    Ok(())
}

/// A fresh checkout completion supersedes the prior hold — the raise
/// path: the new authorization activates at its full amount and the
/// worker cancels the replaced hold at Stripe.
#[tokio::test]
async fn fresh_checkout_supersedes_prior_hold() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = checkout_setup(&app).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;

    let first = mint_checkout(&app, &auction_id, dec!(5)).await?;
    let first_session = first.checkout_session_id.clone().unwrap();
    pay_mock_session(&app, &first_session, "pi_first", first.id, 500);
    deliver(&app, &capturable_event("pi_first", first.id, 500)).await?;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(5));

    // The first row is authorized, not checkout_created, so the second
    // mint has nothing to retire.
    let second = mint_checkout(&app, &auction_id, dec!(9)).await?;
    let second_session = second.checkout_session_id.clone().unwrap();
    pay_mock_session(&app, &second_session, "pi_second", second.id, 900);
    deliver(&app, &capturable_event("pi_second", second.id, 900)).await?;

    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(9));
    let rows = intent_rows(&app, &auction_id).await?;
    let old = rows.iter().find(|r| r.status == "superseded").unwrap();
    assert_eq!(old.authorized_amount, Some(dec!(5)));
    let new = rows.iter().find(|r| r.status == "authorized").unwrap();
    assert!(new.is_active);
    assert_eq!(new.authorized_amount, Some(dec!(9)));

    app.tick().await;
    let rows = intent_rows(&app, &auction_id).await?;
    assert!(rows.iter().any(|r| r.status == "canceled"));
    {
        let intents = app.stripe_service.mock_payment_intents.lock().unwrap();
        assert_eq!(intents.get("pi_first").unwrap().status, "canceled");
        assert_eq!(
            intents.get("pi_second").unwrap().status,
            "requires_capture"
        );
    }
    Ok(())
}

/// The replacement floor for a reducing mint is the commitment less
/// balance backing. Below it committed bids would lose their backing,
/// so the mint is refused up front — the member learns before walking
/// the Checkout flow, not after; at the floor the reduction leans on
/// balance and completes, superseding the larger hold.
#[tokio::test]
async fn checkout_reduces_into_balance_backing() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = checkout_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(5)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;

    let first = mint_checkout(&app, &auction_id, dec!(8)).await?;
    let first_session = first.checkout_session_id.clone().unwrap();
    pay_mock_session(&app, &first_session, "pi_first", first.id, 800);
    deliver(&app, &capturable_event("pi_first", first.id, 800)).await?;

    // The bid commits 10 against the 8 hold + 5 balance; the floor for
    // a replacement is the 5 the balance can't cover.
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    let result = app
        .client
        .checkout_funding(&requests::CheckoutFunding {
            auction_id,
            amount: dec!(4),
        })
        .await;
    assert_api_error(
        result,
        ApiError::HoldReplacementTooSmall {
            current: dec!(8),
            floor: dec!(5),
        },
    );

    // At the floor: the completion supersedes the 8 hold and the worker
    // releases it, with the balance now backing the other 5 committed.
    let second = mint_checkout(&app, &auction_id, dec!(5)).await?;
    let second_session = second.checkout_session_id.clone().unwrap();
    pay_mock_session(&app, &second_session, "pi_second", second.id, 500);
    deliver(&app, &capturable_event("pi_second", second.id, 500)).await?;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(5));
    app.tick().await;
    {
        let intents = app.stripe_service.mock_payment_intents.lock().unwrap();
        assert_eq!(intents.get("pi_first").unwrap().status, "canceled");
        assert_eq!(
            intents.get("pi_second").unwrap().status,
            "requires_capture"
        );
    }

    Ok(())
}

/// An over-held member reduces their hold through a fresh smaller
/// checkout: with nothing committed by bids the replacement floor is
/// zero, so the smaller completion supersedes the larger hold and the
/// worker releases it.
#[tokio::test]
async fn checkout_reduces_overheld_amount() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = checkout_setup(&app).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;

    let first = mint_checkout(&app, &auction_id, dec!(9)).await?;
    let first_session = first.checkout_session_id.clone().unwrap();
    pay_mock_session(&app, &first_session, "pi_first", first.id, 900);
    deliver(&app, &capturable_event("pi_first", first.id, 900)).await?;

    let second = mint_checkout(&app, &auction_id, dec!(4)).await?;
    let second_session = second.checkout_session_id.clone().unwrap();
    pay_mock_session(&app, &second_session, "pi_second", second.id, 400);
    deliver(&app, &capturable_event("pi_second", second.id, 400)).await?;

    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(4));

    app.tick().await;
    {
        let intents = app.stripe_service.mock_payment_intents.lock().unwrap();
        assert_eq!(intents.get("pi_first").unwrap().status, "canceled");
        assert_eq!(
            intents.get("pi_second").unwrap().status,
            "requires_capture"
        );
    }
    Ok(())
}

/// The adoption-time shrink guard: bids commit after a reducing session
/// was minted, so by the time the member pays, the smaller hold would
/// under-back them — the completion is released with notice instead of
/// superseding the larger hold.
#[tokio::test]
async fn shrunk_completion_released_when_bids_commit() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = checkout_setup(&app).await?;
    app.login_alice().await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;

    let first = mint_checkout(&app, &auction_id, dec!(10)).await?;
    let first_session = first.checkout_session_id.clone().unwrap();
    pay_mock_session(&app, &first_session, "pi_first", first.id, 1000);
    deliver(&app, &capturable_event("pi_first", first.id, 1000)).await?;

    // Nothing committed yet, so the reducing mint passes its floor...
    let second = mint_checkout(&app, &auction_id, dec!(5)).await?;
    // ...then a bid commits the full 10 against the live hold.
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;

    let second_session = second.checkout_session_id.clone().unwrap();
    pay_mock_session(&app, &second_session, "pi_second", second.id, 500);
    deliver(&app, &capturable_event("pi_second", second.id, 500)).await?;

    // The 10 hold stands; the 5 completion was canceled with notice.
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(10));
    let rows = checkout_rows(&app, &auction_id).await?;
    let released = rows.iter().find(|r| r.id == second.id).unwrap();
    assert_eq!(released.status, "canceled");
    assert!(released.reconciled_at_set);
    {
        let intents = app.stripe_service.mock_payment_intents.lock().unwrap();
        assert_eq!(intents.get("pi_second").unwrap().status, "canceled");
        assert_eq!(intents.get("pi_first").unwrap().status, "requires_capture");
    }
    Ok(())
}

/// A new mint expires the member's previous open session at Stripe and
/// retires its row, so at most one completable session exists; the dead
/// row is stamped reconciled (no hold can exist behind an expired
/// session).
#[tokio::test]
async fn new_mint_retires_stale_open_session() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = checkout_setup(&app).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;

    let first = mint_checkout(&app, &auction_id, dec!(5)).await?;
    let second = mint_checkout(&app, &auction_id, dec!(9)).await?;
    assert_ne!(first.id, second.id);

    let rows = checkout_rows(&app, &auction_id).await?;
    let old = rows.iter().find(|r| r.id == first.id).unwrap();
    assert_eq!(old.status, "canceled");
    assert!(old.reconciled_at_set);
    let new = rows.iter().find(|r| r.id == second.id).unwrap();
    assert_eq!(new.status, "checkout_created");
    {
        let sessions = app.stripe_service.mock_sessions.lock().unwrap();
        let old_session = sessions
            .get(first.checkout_session_id.as_deref().unwrap())
            .unwrap();
        assert_eq!(old_session.status, "expired");
    }
    Ok(())
}

/// A previous session the member already paid refuses to be superseded:
/// the expire is atomic against completion, and the mint bails while
/// that payment's webhook activates the hold.
#[tokio::test]
async fn paid_stale_session_blocks_new_mint() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = checkout_setup(&app).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;

    let first = mint_checkout(&app, &auction_id, dec!(5)).await?;
    let session_id = first.checkout_session_id.clone().unwrap();
    pay_mock_session(&app, &session_id, "pi_paid", first.id, 500);

    assert_api_error(
        app.client
            .checkout_funding(&requests::CheckoutFunding {
                auction_id,
                amount: dec!(9),
            })
            .await,
        ApiError::CheckoutAlreadyProcessing,
    );
    let rows = checkout_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "checkout_created");
    Ok(())
}

/// An abandoned session's expiry webhook retires the row.
#[tokio::test]
async fn abandoned_session_expires_row() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = checkout_setup(&app).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    let row = mint_checkout(&app, &auction_id, dec!(5)).await?;
    let session_id = row.checkout_session_id.clone().unwrap();
    {
        let mut sessions = app.stripe_service.mock_sessions.lock().unwrap();
        sessions.get_mut(&session_id).unwrap().status = "expired".to_string();
    }

    deliver(
        &app,
        &json!({
            "type": "checkout.session.expired",
            "account": "acct_mock_1",
            "data": {"object": {
                "id": session_id,
                "metadata": {"funding_intent_id": row.id.to_string()},
            }},
        }),
    )
    .await?;

    let rows = checkout_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "canceled");
    assert!(rows[0].reconciled_at_set);
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.checkout_pending, None);
    Ok(())
}

/// An in-session decline leaves the row untouched: the session stays
/// open for retry, and no decline metadata lands (it would pause a
/// saved-card member's automatic raises).
#[tokio::test]
async fn in_session_decline_leaves_row_open() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = checkout_setup(&app).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    let row = mint_checkout(&app, &auction_id, dec!(5)).await?;

    deliver(
        &app,
        &json!({
            "type": "payment_intent.payment_failed",
            "account": "acct_mock_1",
            "data": {"object": {
                "id": "pi_declined",
                "metadata": {"funding_intent_id": row.id.to_string()},
                "last_payment_error": {"decline_code": "card_declined"},
            }},
        }),
    )
    .await?;

    let rows = checkout_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "checkout_created");
    let (pi, decline): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT payment_intent_id, last_decline_code \
         FROM funding_intents WHERE id = $1",
    )
    .bind(row.id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(pi, None);
    assert_eq!(decline, None);
    Ok(())
}

/// A completion landing after the auction ended is released: the hold
/// is canceled at Stripe, the row terminalized, and the member notified
/// (they may have closed the tab believing they were backed).
#[tokio::test]
async fn late_completion_after_end_released() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = checkout_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    let row = mint_checkout(&app, &auction_id, dec!(5)).await?;
    let session_id = row.checkout_session_id.clone().unwrap();

    // Nobody bids; the auction concludes while the session is open.
    run_until_ended(&app, &[auction_id]).await?;
    pay_mock_session(&app, &session_id, "pi_late", row.id, 500);
    deliver(&app, &capturable_event("pi_late", row.id, 500)).await?;

    let rows = checkout_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "canceled");
    assert!(rows[0].reconciled_at_set);
    {
        let intents = app.stripe_service.mock_payment_intents.lock().unwrap();
        assert_eq!(intents.get("pi_late").unwrap().status, "canceled");
    }
    let (kind, notified): (String, UserId) = sqlx::query_as(
        "SELECT kind::TEXT, user_id FROM notification_outbox \
         WHERE dedup_key = $1",
    )
    .bind(format!("checkout_released:{}", row.id))
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(kind, "checkout_released");
    assert_eq!(notified, bob);
    Ok(())
}

/// The reconciliation converge retires a checkout whose expiry webhook
/// was permanently missed, from live session state.
#[tokio::test]
async fn converge_retires_dead_session_without_webhook() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = checkout_setup(&app).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    let row = mint_checkout(&app, &auction_id, dec!(5)).await?;
    let session_id = row.checkout_session_id.clone().unwrap();
    {
        let mut sessions = app.stripe_service.mock_sessions.lock().unwrap();
        sessions.get_mut(&session_id).unwrap().status = "expired".to_string();
    }

    store::funding_checkout::converge_checkout(
        &payloads::FundingIntentId(row.id),
        "acct_mock_1",
        &app.stripe_service,
        &app.time_source,
        &app.db_pool,
    )
    .await?;

    let rows = checkout_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "canceled");
    assert!(rows[0].reconciled_at_set);
    Ok(())
}

/// Mint validation: amount quantization and bounds, the advance gate,
/// and the saved-card/grant prerequisites deliberately not applying.
#[tokio::test]
async fn mint_validation() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = checkout_setup(&app).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    assert_api_error(
        app.client
            .checkout_funding(&requests::CheckoutFunding {
                auction_id,
                amount: dec!(0),
            })
            .await,
        ApiError::InvalidPurchaseAmount,
    );
    assert_api_error(
        app.client
            .checkout_funding(&requests::CheckoutFunding {
                auction_id,
                amount: dec!(0.25),
            })
            .await,
        ApiError::InvalidPurchaseAmount,
    );
    assert_api_error(
        app.client
            .checkout_funding(&requests::CheckoutFunding {
                auction_id,
                amount: dec!(1.001),
            })
            .await,
        ApiError::InvalidPurchaseAmount,
    );
    assert_api_error(
        app.client
            .checkout_funding(&requests::CheckoutFunding {
                auction_id,
                amount: dec!(1_000_000),
            })
            .await,
        ApiError::AmountTooLarge {
            max: dec!(999999.99),
        },
    );
    assert!(checkout_rows(&app, &auction_id).await?.is_empty());
    Ok(())
}

/// The advance gate mirrors the saved-card pre-authorize: a known start
/// more than 24h out rejects with the opening time.
#[tokio::test]
async fn mint_advance_gate() -> anyhow::Result<()> {
    use jiff::Span;

    let app = spawn_app().await;
    let community_id = checkout_setup(&app).await?;
    app.login_alice().await?;
    let start_at = app.time_source.now() + Span::new().hours(48);
    let (_, auction_id) = crate::funding_schedule::create_scheduled_auction(
        &app,
        community_id,
        "site a",
        dec!(10),
        Some(Span::new().hours(48)),
    )
    .await?;

    app.login_bob().await?;
    assert_api_error(
        app.client
            .checkout_funding(&requests::CheckoutFunding {
                auction_id,
                amount: dec!(5),
            })
            .await,
        ApiError::PreauthNotYetOpen {
            authorize_from: start_at - Span::new().hours(24),
        },
    );
    assert!(checkout_rows(&app, &auction_id).await?.is_empty());
    Ok(())
}

/// A community without card payments enabled refuses the mint, and a
/// balance-only member's bids stay unaffected by the checkout machinery.
#[tokio::test]
async fn mint_requires_operational_account() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_backed_credits_mode(&community_id).await?;
    app.login_alice().await?;
    let bob = bob_user_id(&app, &community_id).await?;
    credit_member(&app, community_id, bob, dec!(10)).await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    assert_api_error(
        app.client
            .checkout_funding(&requests::CheckoutFunding {
                auction_id,
                amount: dec!(5),
            })
            .await,
        ApiError::CardPaymentsNotEnabled,
    );
    Ok(())
}
