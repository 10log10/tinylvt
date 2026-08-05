//! Phase 9 reconciliation tests: the invariant suite as an oracle
//! (clean states pass, seeded drift of every class is detected, legal
//! shortfall states are tolerated), the orphaned-hold sweep's
//! metadata probe, the live-PI cross-check, the hourly run-all cadence
//! gate, mid-auction account degradation, and the community wind-down
//! guard.

use api::store;
use api::store::reconciliation::InvariantViolation;
use jiff::Span;
use jiff_sqlx::ToSqlx;
use payloads::{ApiError, AuctionId, requests};
use rust_decimal::dec;
use serde_json::json;
use test_helpers::{TestApp, assert_api_error, spawn_app};

use crate::funding::{create_open_auction, credit_member, run_until_ended};
use crate::funding_auth::{bob_user_id, card_enabled_setup, intent_rows};
use crate::funding_schedule::emails_matching;

/// Force a mock PaymentIntent into an out-of-band state.
fn set_mock_intent(
    app: &TestApp,
    pi_id: &str,
    status: &str,
    cancellation_reason: Option<&str>,
) {
    let mut intents = app.stripe_service.mock_payment_intents.lock().unwrap();
    let intent = intents.get_mut(pi_id).unwrap();
    intent.status = status.to_string();
    intent.cancellation_reason = cancellation_reason.map(str::to_string);
}

/// The single funding-intent row id for an auction.
async fn sole_intent_id(
    app: &TestApp,
    auction_id: &AuctionId,
) -> anyhow::Result<uuid::Uuid> {
    Ok(sqlx::query_scalar(
        "SELECT id FROM funding_intents WHERE auction_id = $1",
    )
    .bind(auction_id)
    .fetch_one(&app.db_pool)
    .await?)
}

/// Insert a canceled no-PI intent row (an orphan-sweep candidate) for
/// (auction, member), returning its id.
async fn insert_canceled_orphan(
    app: &TestApp,
    auction_id: &AuctionId,
    user_id: &payloads::UserId,
) -> anyhow::Result<uuid::Uuid> {
    Ok(sqlx::query_scalar(
        "INSERT INTO funding_intents \
         (auction_id, user_id, status, origin, requested_amount, \
          created_at, updated_at) \
         VALUES ($1, $2, 'canceled', 'bid_flow', 2, $3, $3) RETURNING id",
    )
    .bind(auction_id)
    .bind(user_id)
    .bind(app.time_source.now().to_sqlx())
    .fetch_one(&app.db_pool)
    .await?)
}

/// A full card-backed auction (bid 10 against balance 4, capture 6)
/// leaves every invariant clean, and the cross-check exercises the
/// mock retrieve.
#[tokio::test]
async fn clean_state_suite_passes() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(4)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    run_until_ended(&app, &[auction_id]).await?;

    let report = app.reconcile().await;
    assert!(report.violations.is_empty(), "{:?}", report.violations);
    assert!(report.intents_cross_checked >= 1);
    assert_eq!(report.orphan_candidates, 0);
    Ok(())
}

/// Seeded ledger drift is detected: a bumped balance_cached, and a
/// one-sided journal line (which also drifts its account's balance).
#[tokio::test]
async fn ledger_drift_detected() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_backed_credits_mode(&community_id).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(10)).await?;

    sqlx::query(
        "UPDATE accounts SET balance_cached = balance_cached + 1 \
         WHERE community_id = $1 AND owner_type = 'member_main' \
           AND owner_id = $2",
    )
    .bind(community_id)
    .bind(bob)
    .execute(&app.db_pool)
    .await?;
    let entry_id: uuid::Uuid = sqlx::query_scalar(
        "SELECT id FROM journal_entries WHERE community_id = $1",
    )
    .bind(community_id)
    .fetch_one(&app.db_pool)
    .await?;
    sqlx::query(
        "INSERT INTO journal_lines (id, entry_id, account_id, amount) \
         SELECT gen_random_uuid(), $1, id, 5 FROM accounts \
         WHERE community_id = $2 AND owner_type = 'community_treasury'",
    )
    .bind(entry_id)
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;

    let report = app.reconcile().await;
    assert!(report.violations.iter().any(|(_, v)| matches!(
        v,
        InvariantViolation::EntryNotBalanced { entry_id: e, .. }
            if e.0 == entry_id
    )));
    // Both the bumped member account and the treasury (whose line was
    // inserted without a balance update) re-derive differently.
    let drifts = report
        .violations
        .iter()
        .filter(|(_, v)| matches!(v, InvariantViolation::BalanceDrift { .. }))
        .count();
    assert_eq!(drifts, 2, "{:?}", report.violations);
    Ok(())
}

/// Backing lost to an out-of-band cancel leaves the member's balance
/// commitments exceeding their balance — the shortfall state: reported,
/// tolerated, not an error.
#[tokio::test]
async fn backing_shortfall_tolerated() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(4)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;

    let rows = intent_rows(&app, &auction_id).await?;
    let pi_id = rows[0].payment_intent_id.clone().unwrap();
    set_mock_intent(&app, &pi_id, "canceled", Some("requested_by_customer"));
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "payment_intent.canceled",
            "account": "acct_mock_1",
            "data": {"object": {
                "id": pi_id,
                "cancellation_reason": "requested_by_customer",
            }},
        }),
    )
    .await?;

    let report = app.reconcile().await;
    let (_, shortfall) = report
        .violations
        .iter()
        .find(|(_, v)| {
            matches!(v, InvariantViolation::MemberUnderBacked { .. })
        })
        .expect("shortfall reported");
    assert!(shortfall.is_tolerated());
    assert_eq!(report.errors().count(), 0, "{:?}", report.violations);
    Ok(())
}

/// An active authorized hold on an ended auction is flagged only past
/// the worker-lag grace.
#[tokio::test]
async fn stale_authorized_intent_flagged() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(4)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    run_until_ended(&app, &[auction_id]).await?;

    // Simulate the settlement marking never having run. The mock intent
    // is reset to a live hold to match: with the pair in sync
    // (authorized × requires_capture) the cross-check's repair leaves
    // the row alone and the pure-DB invariant is what must flag it.
    let intent_id = sole_intent_id(&app, &auction_id).await?;
    sqlx::query(
        "UPDATE funding_intents SET status = 'authorized', is_active = TRUE \
         WHERE id = $1",
    )
    .bind(intent_id)
    .execute(&app.db_pool)
    .await?;
    let pi_id: String = sqlx::query_scalar(
        "SELECT payment_intent_id FROM funding_intents WHERE id = $1",
    )
    .bind(intent_id)
    .fetch_one(&app.db_pool)
    .await?;
    set_mock_intent(&app, &pi_id, "requires_capture", None);

    app.time_source.advance(Span::new().hours(2));
    let report = app.reconcile().await;
    assert!(!report.violations.iter().any(|(_, v)| matches!(
        v,
        InvariantViolation::StaleAuthorizedIntent { .. }
    )));

    app.time_source.advance(Span::new().hours(2));
    let report = app.reconcile().await;
    assert!(report.violations.iter().any(|(_, v)| matches!(
        v,
        InvariantViolation::StaleAuthorizedIntent { .. }
    )));
    Ok(())
}

/// A capture_pending row is flagged only once its capture window is
/// past the grace.
#[tokio::test]
async fn overdue_capture_flagged() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(4)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    run_until_ended(&app, &[auction_id]).await?;

    let intent_id = sole_intent_id(&app, &auction_id).await?;
    let seed_window = |hours_ago: i64| {
        let capture_before = app
            .time_source
            .now()
            .checked_sub(Span::new().hours(hours_ago))
            .unwrap();
        sqlx::query(
            "UPDATE funding_intents \
             SET status = 'capture_pending', capture_before = $1 \
             WHERE id = $2",
        )
        .bind(capture_before.to_sqlx())
        .bind(intent_id)
    };

    seed_window(1).execute(&app.db_pool).await?;
    let report = app.reconcile().await;
    assert!(
        !report.violations.iter().any(|(_, v)| matches!(
            v,
            InvariantViolation::OverdueCapture { .. }
        ))
    );

    seed_window(4).execute(&app.db_pool).await?;
    let report = app.reconcile().await;
    assert!(
        report.violations.iter().any(|(_, v)| matches!(
            v,
            InvariantViolation::OverdueCapture { .. }
        ))
    );
    Ok(())
}

/// A corrupted entry↔intent linkage is flagged from both directions:
/// the entry matches no captured intent, and the captured intent has no
/// entry.
#[tokio::test]
async fn payment_entry_mismatch_flagged() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(4)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    run_until_ended(&app, &[auction_id]).await?;

    sqlx::query(
        "UPDATE journal_entries SET payment_intent_id = 'pi_bogus' \
         WHERE community_id = $1 AND entry_type = 'stripe_payment'",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;

    let report = app.reconcile().await;
    assert!(report.violations.iter().any(|(_, v)| matches!(
        v,
        InvariantViolation::PaymentEntryMismatch { payment_intent_id, .. }
            if payment_intent_id == "pi_bogus"
    )));
    assert!(report.violations.iter().any(|(_, v)| matches!(
        v,
        InvariantViolation::CapturedIntentMissingEntry { .. }
    )));
    Ok(())
}

/// A `stripe_payment` entry whose lines hit the wrong accounts is still
/// flagged: rerouting the member line to the treasury keeps the entry
/// balanced (EntryNotBalanced stays quiet), so the mismatch check's
/// member-line lookup must not silently drop the entry.
#[tokio::test]
async fn payment_entry_without_member_line_flagged() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(4)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    run_until_ended(&app, &[auction_id]).await?;

    sqlx::query(
        "UPDATE journal_lines jl SET account_id = tr.id \
         FROM journal_entries je, accounts tr \
         WHERE jl.entry_id = je.id \
           AND je.community_id = $1 AND je.entry_type = 'stripe_payment' \
           AND tr.community_id = $1 \
           AND tr.owner_type = 'community_treasury' \
           AND jl.account_id <> tr.id",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;

    let report = app.reconcile().await;
    assert!(report.violations.iter().any(|(_, v)| matches!(
        v,
        InvariantViolation::PaymentEntryMismatch { detail, .. }
            if detail.contains("no member_main line")
    )));
    Ok(())
}

/// The sweep recovers a hold whose execute crashed after Stripe created
/// it (canceled local row, no recorded PI): the metadata probe finds
/// the PaymentIntent, cancels the hold, records the linkage, and
/// retires the row from the candidate set.
#[tokio::test]
async fn orphan_sweep_recovers_and_cancels() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (_space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(2)),
        })
        .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    let pi_id = rows[0].payment_intent_id.clone().unwrap();

    // The crash aftermath: the claim's finalize write never landed and
    // the adoption webhook was lost.
    sqlx::query(
        "UPDATE funding_intents \
         SET status = 'canceled', is_active = FALSE, \
             payment_intent_id = NULL \
         WHERE id = $1",
    )
    .bind(sole_intent_id(&app, &auction_id).await?)
    .execute(&app.db_pool)
    .await?;
    app.time_source.advance(Span::new().hours(4));

    let report = app.reconcile().await;
    assert_eq!(report.orphan_candidates, 1);
    assert_eq!(report.orphans_resolved, 1);
    assert_eq!(report.holds_canceled, 1);
    {
        let intents = app.stripe_service.mock_payment_intents.lock().unwrap();
        assert_eq!(intents.get(&pi_id).unwrap().status, "canceled");
    }
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].payment_intent_id.as_deref(), Some(pi_id.as_str()));
    let reconciled: bool = sqlx::query_scalar(
        "SELECT reconciled_at IS NOT NULL FROM funding_intents \
         WHERE id = $1",
    )
    .bind(sole_intent_id(&app, &auction_id).await?)
    .fetch_one(&app.db_pool)
    .await?;
    assert!(reconciled);

    // Resolved rows leave the candidate set.
    let report = app.reconcile().await;
    assert_eq!(report.orphan_candidates, 0);
    Ok(())
}

/// A canceled row whose execute never reached Stripe resolves without
/// creating anything: the probe confirms absence and the row is marked
/// reconciled — never the fresh-hold-then-cancel the replay-by-key
/// design would have minted.
#[tokio::test]
async fn orphan_sweep_skips_never_attempted() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(5)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(1)).await?;
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;

    let intent_id = insert_canceled_orphan(&app, &auction_id, &bob).await?;
    app.time_source.advance(Span::new().hours(4));

    let intents_before = app
        .stripe_service
        .mock_payment_intents
        .lock()
        .unwrap()
        .len();
    let report = app.reconcile().await;
    assert_eq!(report.orphan_candidates, 1);
    assert_eq!(report.orphans_resolved, 1);
    assert_eq!(report.holds_canceled, 0);
    let (pi, reconciled): (Option<String>, bool) = sqlx::query_as(
        "SELECT payment_intent_id, reconciled_at IS NOT NULL \
         FROM funding_intents WHERE id = $1",
    )
    .bind(intent_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(pi, None);
    assert!(reconciled);
    assert_eq!(
        app.stripe_service
            .mock_payment_intents
            .lock()
            .unwrap()
            .len(),
        intents_before
    );
    Ok(())
}

/// A merely charges-disabled account (Stripe pausing charges over
/// requirements) is still API-reachable, so the orphan sweep and the
/// cross-check keep covering the community instead of going blind
/// during account distress.
#[tokio::test]
async fn charges_disabled_community_still_reconciled() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (_space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(2)),
        })
        .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    let pi_id = rows[0].payment_intent_id.clone().unwrap();
    // An out-of-band cancel for the cross-check to repair, and a
    // hand-made orphan candidate for the sweep to resolve.
    set_mock_intent(&app, &pi_id, "canceled", Some("requested_by_customer"));
    let bob = bob_user_id(&app, &community_id).await?;
    let orphan_id = insert_canceled_orphan(&app, &auction_id, &bob).await?;
    sqlx::query(
        "UPDATE communities SET stripe_charges_enabled = FALSE \
         WHERE id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;
    app.time_source.advance(Span::new().hours(4));

    let report = app.reconcile().await;
    assert_eq!(report.orphan_candidates, 1);
    assert_eq!(report.orphans_resolved, 1);
    report
        .violations
        .iter()
        .find(|(_, v)| matches!(v, InvariantViolation::IntentRepaired { .. }))
        .expect("cross-check repair reported");
    let reconciled: bool = sqlx::query_scalar(
        "SELECT reconciled_at IS NOT NULL FROM funding_intents \
         WHERE id = $1",
    )
    .bind(orphan_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert!(reconciled);
    let repaired: String = sqlx::query_scalar(
        "SELECT status::TEXT FROM funding_intents \
         WHERE payment_intent_id = $1",
    )
    .bind(&pi_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(repaired, "canceled");
    Ok(())
}

/// Deauthorization retires the community's orphan-sweep candidates:
/// any PaymentIntent their creates minted lives on an account the
/// platform can no longer probe, and unretired they'd sit in the
/// sweep's partial index forever.
#[tokio::test]
async fn deauthorization_retires_orphan_candidates() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (_space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(2)),
        })
        .await?;
    let bob = bob_user_id(&app, &community_id).await?;
    let orphan_id = insert_canceled_orphan(&app, &auction_id, &bob).await?;

    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "account.application.deauthorized",
            "account": "acct_mock_1",
            "data": { "object": {
                "id": "ca_mock_application",
                "object": "application",
            }},
        }),
    )
    .await?;

    let (pi, reconciled): (Option<String>, bool) = sqlx::query_as(
        "SELECT payment_intent_id, reconciled_at IS NOT NULL \
         FROM funding_intents WHERE id = $1",
    )
    .bind(orphan_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(pi, None, "retired without probing");
    assert!(reconciled);
    app.time_source.advance(Span::new().hours(4));
    let report = app.reconcile().await;
    assert_eq!(report.orphan_candidates, 0);
    Ok(())
}

/// A gone account replaced directly in the connect flow — the path
/// that never sets the deauthorization marker — also retires orphan
/// candidates in the swap: the sweep must not probe the replacement
/// account for old-account intents and wrongly confirm absence.
#[tokio::test]
async fn gone_account_replacement_retires_orphan_candidates()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (_space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(2)),
        })
        .await?;
    let bob = bob_user_id(&app, &community_id).await?;
    let orphan_id = insert_canceled_orphan(&app, &auction_id, &bob).await?;

    // Register the stored account in the mock so the replacement
    // mints a distinct id (card_enabled_setup stamps acct_mock_1 via
    // SQL without going through the mock).
    app.stripe_service
        .create_connected_account("seed", &community_id)
        .await?;
    app.stripe_service
        .mock_gone_accounts
        .lock()
        .unwrap()
        .insert("acct_mock_1".to_string());
    app.login_alice().await?;
    app.client.connect_community_stripe(&community_id).await?;

    let reconciled: bool = sqlx::query_scalar(
        "SELECT reconciled_at IS NOT NULL FROM funding_intents \
         WHERE id = $1",
    )
    .bind(orphan_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert!(reconciled);
    Ok(())
}

/// A hold canceled at Stripe without its webhook arriving is repaired
/// by the cross-check: the row converges to canceled (with the
/// backing-lost notice), reported as a tolerated `IntentRepaired`
/// finding rather than error-level drift.
#[tokio::test]
async fn out_of_band_cancel_repaired() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (_space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(2)),
        })
        .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    let pi_id = rows[0].payment_intent_id.clone().unwrap();
    set_mock_intent(&app, &pi_id, "canceled", Some("requested_by_customer"));

    let report = app.reconcile().await;
    let (_, repaired) = report
        .violations
        .iter()
        .find(|(_, v)| matches!(v, InvariantViolation::IntentRepaired { .. }))
        .expect("repair reported");
    assert!(repaired.is_tolerated());
    assert_eq!(report.errors().count(), 0, "{:?}", report.violations);

    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "canceled");
    assert!(!rows[0].is_active);

    // The repair carries the same backing-lost notice the webhook
    // would have sent.
    app.tick().await;
    let member = emails_matching(&app, "Your card hold in");
    assert_eq!(member.len(), 1);
    assert_eq!(member[0].0, "bob@example.com");

    // The next pass finds the pair in sync (canceled × canceled).
    let report = app.reconcile().await;
    assert!(report.violations.is_empty(), "{:?}", report.violations);
    Ok(())
}

/// A hold that reached Stripe's own expiry while locally authorized is
/// repaired to `expired`, and the notice says expiry, not a dashboard
/// cancel.
#[tokio::test]
async fn natural_expiry_repaired_with_expiry_wording() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (_space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(2)),
        })
        .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    let pi_id = rows[0].payment_intent_id.clone().unwrap();
    set_mock_intent(&app, &pi_id, "canceled", Some("expired"));

    let report = app.reconcile().await;
    assert_eq!(report.errors().count(), 0, "{:?}", report.violations);
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "expired");

    app.tick().await;
    let member = emails_matching(&app, "Your card hold in");
    assert_eq!(member.len(), 1);
    assert!(member[0].1.contains("expired"), "{}", member[0].1);
    assert!(
        !member[0].1.contains("canceled outside TinyLVT"),
        "{}",
        member[0].1
    );
    Ok(())
}

/// A routine card decline is not drift: the canceled row's intent
/// parks at Stripe's `requires_payment_method` (no hold was ever
/// placed), which the cross-check tolerates for declined rows.
#[tokio::test]
async fn declined_intent_not_drift() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.stripe_service
        .mock_declining_payment_methods
        .lock()
        .unwrap()
        .insert("pm_bob".to_string(), "insufficient_funds".to_string());
    app.login_alice().await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let result = app.client.create_bid(&space_id, &rounds[0].round_id).await;
    assert_api_error(
        result,
        ApiError::CardDeclined {
            code: Some("insufficient_funds".to_string()),
        },
    );

    let report = app.reconcile().await;
    assert!(report.violations.is_empty(), "{:?}", report.violations);
    assert!(report.intents_cross_checked >= 1);
    Ok(())
}

/// Force a mock PaymentIntent into the out-of-band-captured state (the
/// dashboard's Capture button, or a worker capture whose commit was
/// lost).
fn set_mock_captured(app: &TestApp, pi_id: &str, received_minor: i64) {
    let mut intents = app.stripe_service.mock_payment_intents.lock().unwrap();
    let intent = intents.get_mut(pi_id).unwrap();
    intent.status = "succeeded".to_string();
    intent.amount_received = received_minor;
}

/// The count of `stripe_payment` journal entries in a community.
async fn stripe_payment_entries(
    app: &TestApp,
    community_id: &payloads::CommunityId,
) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM journal_entries \
         WHERE community_id = $1 AND entry_type = 'stripe_payment'",
    )
    .bind(community_id)
    .fetch_one(&app.db_pool)
    .await?)
}

/// A dashboard capture of a live authorization — money collected
/// outside the app's flow — previously wedged the row `authorized`
/// forever with nothing booked. The cross-check repairs it: the row
/// converges to `captured`, the collected amount is booked as the
/// member's credit (no settlement debit exists, so it stays as
/// balance), and the repair is an error-level finding.
#[tokio::test]
async fn dashboard_capture_of_live_hold_booked() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    let (_space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(6)),
        })
        .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    let pi_id = rows[0].payment_intent_id.clone().unwrap();
    set_mock_captured(&app, &pi_id, 600);

    let report = app.reconcile().await;
    let (_, repaired) = report
        .violations
        .iter()
        .find(|(_, v)| matches!(v, InvariantViolation::IntentRepaired { .. }))
        .expect("repair reported");
    assert!(
        matches!(
            repaired,
            InvariantViolation::IntentRepaired {
                out_of_band_money: true,
                ..
            }
        ),
        "{repaired:?}"
    );
    assert_eq!(report.errors().count(), 1, "{:?}", report.violations);

    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "captured");
    assert!(!rows[0].is_active);
    assert_eq!(
        crate::funding_capture::member_balance(&app, &community_id, &bob)
            .await?,
        dec!(6)
    );
    assert_eq!(stripe_payment_entries(&app, &community_id).await?, 1);

    app.tick().await;
    assert_eq!(emails_matching(&app, "Receipt:").len(), 1);

    // The next pass finds the pair in sync (captured × succeeded).
    let report = app.reconcile().await;
    assert!(report.violations.is_empty(), "{:?}", report.violations);
    Ok(())
}

/// A worker capture that succeeded at Stripe but whose commit was lost
/// (crash before the DB write) is re-booked by the cross-check: row
/// `captured`, issuance entry created once, debt cleared — a routine
/// (info-level) repair since the collected amount matches the sized
/// capture.
#[tokio::test]
async fn lost_capture_commit_rebooked() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(4)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    let pi_id = intent_rows(&app, &auction_id).await?[0]
        .payment_intent_id
        .clone()
        .unwrap();
    // Keep the worker's capture failing (a status the mock's capture
    // rejects) so the row stays capture_pending through conclusion.
    set_mock_intent(&app, &pi_id, "processing", None);
    run_until_ended(&app, &[auction_id]).await?;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "capture_pending");

    // The capture "landed" at Stripe for exactly the sized amount.
    set_mock_captured(&app, &pi_id, 600);
    let report = app.reconcile().await;
    assert_eq!(report.errors().count(), 0, "{:?}", report.violations);
    assert!(report.violations.iter().any(|(_, v)| matches!(
        v,
        InvariantViolation::IntentRepaired {
            out_of_band_money: false,
            ..
        }
    )));

    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "captured");
    // Settlement debited 10 against 4 balance; the booked capture of 6
    // returns the balance to zero.
    assert_eq!(
        crate::funding_capture::member_balance(&app, &community_id, &bob)
            .await?,
        dec!(0)
    );
    assert_eq!(stripe_payment_entries(&app, &community_id).await?, 1);
    Ok(())
}

/// Money collected on a hold we owed a release (a dashboard capture of
/// a release_pending intent) is booked so the ledger matches reality,
/// and reported at error level — the refund, if owed, is the
/// community's dashboard decision.
#[tokio::test]
async fn collected_release_booked_loudly() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    let (_space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(6)),
        })
        .await?;
    let pi_id = intent_rows(&app, &auction_id).await?[0]
        .payment_intent_id
        .clone()
        .unwrap();
    // Keep the worker's release cancel failing so the row stays
    // release_pending after conclusion (no win — nothing owed).
    set_mock_intent(&app, &pi_id, "processing", None);
    run_until_ended(&app, &[auction_id]).await?;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "release_pending");

    set_mock_captured(&app, &pi_id, 600);
    let report = app.reconcile().await;
    assert!(
        report.errors().any(|(_, v)| matches!(
            v,
            InvariantViolation::IntentRepaired {
                out_of_band_money: true,
                ..
            }
        )),
        "{:?}",
        report.violations
    );
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "captured");
    assert_eq!(
        crate::funding_capture::member_balance(&app, &community_id, &bob)
            .await?,
        dec!(6)
    );
    Ok(())
}

/// A locally-expired row whose hold is still live at Stripe (the
/// fallback-window underestimate, or Stripe's lazy auto-cancel) gets
/// its hold canceled by the cross-check instead of logging drift until
/// the hold drains on its own.
#[tokio::test]
async fn expired_rows_live_hold_canceled() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (_space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(6)),
        })
        .await?;
    let intent_id = sole_intent_id(&app, &auction_id).await?;
    let pi_id: String = sqlx::query_scalar(
        "SELECT payment_intent_id FROM funding_intents WHERE id = $1",
    )
    .bind(intent_id)
    .fetch_one(&app.db_pool)
    .await?;
    // The local-expiry shortcut's end state: the row converged with no
    // Stripe call while the real hold outlives the assumed window.
    sqlx::query(
        "UPDATE funding_intents \
         SET status = 'expired', is_active = FALSE WHERE id = $1",
    )
    .bind(intent_id)
    .execute(&app.db_pool)
    .await?;

    let report = app.reconcile().await;
    assert_eq!(report.errors().count(), 0, "{:?}", report.violations);
    assert!(report.violations.iter().any(|(_, v)| matches!(
        v,
        InvariantViolation::IntentRepaired {
            out_of_band_money: false,
            ..
        }
    )));
    assert_eq!(
        app.stripe_service
            .mock_payment_intents
            .lock()
            .unwrap()
            .get(&pi_id)
            .unwrap()
            .status,
        "canceled"
    );

    // The next pass finds the pair in sync (expired × canceled).
    let report = app.reconcile().await;
    assert!(report.violations.is_empty(), "{:?}", report.violations);
    Ok(())
}

/// The tick-driven gate: creation stamps a community current, nothing
/// reruns within the interval, and a stale watermark triggers a rerun
/// for ALL communities together.
#[tokio::test]
async fn reconciliation_cadence() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let t0 = app.time_source.now();
    app.create_two_person_community().await?;

    // Sorted, since community ids are random UUIDs.
    async fn stamps(app: &TestApp) -> Vec<jiff::Timestamp> {
        let rows: Vec<jiff_sqlx::Timestamp> = sqlx::query_scalar(
            "SELECT last_reconciliation_at FROM communities",
        )
        .fetch_all(&app.db_pool)
        .await
        .unwrap();
        let mut stamps: Vec<jiff::Timestamp> =
            rows.into_iter().map(|t| t.to_jiff()).collect();
        stamps.sort();
        stamps
    }

    // Creation stamps the watermark, so nothing is due yet: a new
    // community has nothing to reconcile, and the gate is global.
    assert_eq!(stamps(&app).await, vec![t0]);
    app.time_source.advance(Span::new().minutes(30));
    app.tick().await;
    assert_eq!(stamps(&app).await, vec![t0]);

    // A second community likewise starts current, leaving the first
    // community's watermark untouched — no unscheduled pass.
    let details = requests::CreateCommunity {
        name: "Second community".into(),
        description: None,
        currency: payloads::CurrencySettings {
            mode_config: test_helpers::default_currency_config(),
            name: "dollars".into(),
            symbol: "$".into(),
            minor_units: 2,
            balances_visible_to_members: true,
            new_members_default_active: true,
        },
    };
    let t1 = app.time_source.now();
    app.client.create_community(&details).await?;
    app.tick().await;
    assert_eq!(stamps(&app).await, vec![t0, t1]);

    // Past the interval everything reruns together.
    app.time_source.advance(Span::new().minutes(61));
    let t2 = app.time_source.now();
    app.tick().await;
    assert_eq!(stamps(&app).await, vec![t2, t2]);
    Ok(())
}

/// Mid-auction degradation (charges_enabled withdrawn) freezes the card
/// paths — pre-authorization errors contextually, a bid beyond balance
/// is honestly short — while balance bidding continues.
#[tokio::test]
async fn degradation_freezes_card_paths() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(5)).await?;
    let (space_a, auction_a) =
        create_open_auction(&app, community_id, "site a", dec!(2)).await?;
    let (space_b, auction_b) =
        create_open_auction(&app, community_id, "site b", dec!(10)).await?;

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
                "charges_enabled": false,
                "details_submitted": true,
            }},
        }),
    )
    .await?;

    app.login_bob().await?;
    let result = app
        .client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id: auction_a,
            amount: Some(dec!(2)),
        })
        .await;
    assert_api_error(result, ApiError::CardPaymentsNotEnabled);

    // Balance bidding continues (2 ≤ 5)…
    let rounds = app.client.list_auction_rounds(&auction_a).await?;
    app.client.create_bid(&space_a, &rounds[0].round_id).await?;
    // …but with no card path, a bid beyond balance is short.
    let rounds = app.client.list_auction_rounds(&auction_b).await?;
    let result = app.client.create_bid(&space_b, &rounds[0].round_id).await;
    assert_api_error(result, ApiError::InsufficientBalance);
    Ok(())
}

/// Community deletion refuses while card payments are in flight (live
/// holds, then a pending purchase), without having canceled the
/// subscription — the guard runs before the irreversible Stripe call —
/// and succeeds once everything is terminal.
#[tokio::test]
async fn delete_blocked_by_active_payments() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    let now = app.time_source.now().to_sqlx();
    sqlx::query(
        "INSERT INTO community_subscriptions ( \
            community_id, tier, status, billing_interval, \
            stripe_subscription_id, current_period_start, \
            current_period_end, cancel_at_period_end, \
            created_at, updated_at \
         ) VALUES ($1, 'paid', 'active', 'month', 'sub_mock_recon', \
                   $2, $2, false, $2, $2)",
    )
    .bind(community_id)
    .bind(now)
    .execute(&app.db_pool)
    .await?;

    app.login_alice().await?;
    let (_space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(2)),
        })
        .await?;

    app.login_alice().await?;
    let result = app.client.delete_community(&community_id).await;
    assert_api_error(result, ApiError::CommunityHasActivePayments);
    assert!(
        app.stripe_service
            .mock_canceled_subscriptions
            .lock()
            .unwrap()
            .is_empty()
    );

    // Conclusion releases the unused hold; the worker cancels it.
    run_until_ended(&app, &[auction_id]).await?;
    app.tick().await;
    let rows = intent_rows(&app, &auction_id).await?;
    assert!(rows.iter().all(|r| r.status == "canceled"));

    // A pending purchase blocks deletion the same way.
    sqlx::query(
        "INSERT INTO credit_purchases \
         (community_id, user_id, kind, status, amount, \
          created_at, updated_at) \
         VALUES ($1, $2, 'top_up', 'created', 5, $3, $3)",
    )
    .bind(community_id)
    .bind(bob)
    .bind(now)
    .execute(&app.db_pool)
    .await?;
    let result = app.client.delete_community(&community_id).await;
    assert_api_error(result, ApiError::CommunityHasActivePayments);

    sqlx::query(
        "UPDATE credit_purchases SET status = 'expired' \
         WHERE community_id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;
    app.client.delete_community(&community_id).await?;
    assert_eq!(
        *app.stripe_service
            .mock_canceled_subscriptions
            .lock()
            .unwrap(),
        vec!["sub_mock_recon".to_string()]
    );
    Ok(())
}
