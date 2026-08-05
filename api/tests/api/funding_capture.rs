//! Settlement capture and release tests: capture sizing at conclusion,
//! the capture worker's issuance entry and platform fee, releases for
//! losers and covered winners, sub-minimum forgiveness,
//! pending-capture-aware sizing across auctions, terminal capture
//! failures (hold gone at Stripe) leaving debt, and hold releases on
//! auction cancellation and member departure.

use api::store;
use payloads::{ApiError, AuctionId, CommunityId, UserId, requests};
use rust_decimal::{Decimal, dec};
use serde_json::json;
use test_helpers::{TestApp, assert_api_error, spawn_app};

use crate::funding::{
    create_open_auction, credit_member, finish_round, run_until_ended,
};
use crate::funding_auth::{bob_user_id, card_enabled_setup, intent_rows};

pub async fn member_balance(
    app: &TestApp,
    community_id: &CommunityId,
    user_id: &UserId,
) -> anyhow::Result<Decimal> {
    Ok(sqlx::query_scalar(
        "SELECT balance_cached FROM accounts \
         WHERE community_id = $1 AND owner_type = 'member_main' \
           AND owner_id = $2",
    )
    .bind(community_id)
    .bind(user_id)
    .fetch_one(&app.db_pool)
    .await?)
}

#[derive(Debug, sqlx::FromRow)]
struct CaptureRow {
    payment_intent_id: Option<String>,
    status: String,
    capture_amount: Option<Decimal>,
}

async fn capture_rows(
    app: &TestApp,
    auction_id: &AuctionId,
) -> anyhow::Result<Vec<CaptureRow>> {
    Ok(sqlx::query_as(
        "SELECT payment_intent_id, status::TEXT, capture_amount \
         FROM funding_intents \
         WHERE auction_id = $1 ORDER BY created_at",
    )
    .bind(auction_id)
    .fetch_all(&app.db_pool)
    .await?)
}

/// Set the mock PaymentIntent's status directly, simulating an
/// out-of-band state (e.g. holding it un-capturable so a capture stays
/// pending across ticks).
fn set_mock_intent_status(app: &TestApp, pi_id: &str, status: &str) {
    app.stripe_service
        .mock_payment_intents
        .lock()
        .unwrap()
        .get_mut(pi_id)
        .unwrap()
        .status = status.to_string();
}

/// Force a mock PaymentIntent into the out-of-band-captured state (the
/// dashboard's Capture button).
fn set_mock_captured(app: &TestApp, pi_id: &str, received_minor: i64) {
    let mut intents = app.stripe_service.mock_payment_intents.lock().unwrap();
    let intent = intents.get_mut(pi_id).unwrap();
    intent.status = "succeeded".to_string();
    intent.amount_received = received_minor;
}

/// Clear worker backoff so the next tick retries immediately.
async fn clear_worker_backoff(app: &TestApp) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE funding_intents \
         SET worker_failure_count = 0, worker_last_failed_at = NULL",
    )
    .execute(&app.db_pool)
    .await?;
    Ok(())
}

/// A card-backed winner's authorization is captured at conclusion, sized
/// to what balance couldn't cover, with the platform fee attached and a
/// stripe_payment issuance entry returning the balance to zero.
#[tokio::test]
async fn winner_capture_full_flow() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(4)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    // Bid 10 against balance 4: authorization of 6 backs the rest.
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    run_until_ended(&app, &[auction_id]).await?;

    // Settlement debited 10; the capture of 6 restored the balance to 0.
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    let rows = capture_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "captured");
    assert_eq!(rows[0].capture_amount, Some(dec!(6)));
    let pi_id = rows[0].payment_intent_id.clone().unwrap();
    {
        let intents = app.stripe_service.mock_payment_intents.lock().unwrap();
        let pi = intents.get(&pi_id).unwrap();
        assert_eq!(pi.status, "succeeded");
        assert_eq!(pi.amount_received, 600);
        // 1% platform fee on the 6.00 capture.
        assert_eq!(pi.application_fee_minor, Some(6));
    }
    let (entry_type, entry_pi): (String, Option<String>) = sqlx::query_as(
        "SELECT entry_type::TEXT, payment_intent_id FROM journal_entries \
         WHERE community_id = $1 AND entry_type = 'stripe_payment'",
    )
    .bind(community_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(entry_type, "stripe_payment");
    assert_eq!(entry_pi.as_deref(), Some(pi_id.as_str()));

    Ok(())
}

/// A loser's authorization is released at conclusion: marked
/// release_pending in the settlement transaction and canceled at Stripe
/// by the worker.
#[tokio::test]
async fn loser_hold_released() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let members = app.client.get_members(&community_id).await?;
    let alice = members
        .iter()
        .find(|m| m.user.username == "alice")
        .unwrap()
        .user
        .user_id;
    credit_member(&app, community_id, alice, dec!(20)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    // Bob holds a pre-authorization but never wins; alice takes the
    // space balance-backed.
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(5)),
        })
        .await?;
    app.login_alice().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    run_until_ended(&app, &[auction_id]).await?;

    let rows = capture_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "canceled");
    let pi_id = rows[0].payment_intent_id.clone().unwrap();
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
    Ok(())
}

/// A winner whose balance fully covers the debit keeps nothing on the
/// card: the hold releases and only the balance is consumed.
#[tokio::test]
async fn covered_winner_releases_hold() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(12)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(5)),
        })
        .await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    run_until_ended(&app, &[auction_id]).await?;

    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(2));
    let rows = capture_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "canceled");
    assert_eq!(rows[0].capture_amount, None);
    Ok(())
}

/// A capture remainder below the Stripe minimum can't ride the card:
/// the hold releases and a treasury issuance forgives the remainder,
/// returning the balance to zero.
#[tokio::test]
async fn subminimum_capture_forgiven() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(9.75)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    // The 0.25 gap authorizes at the 0.50 Stripe minimum; at conclusion
    // the 0.25 capture need is below the minimum and is forgiven.
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    run_until_ended(&app, &[auction_id]).await?;

    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    let rows = capture_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "canceled");
    let note: Option<String> = sqlx::query_scalar(
        "SELECT note FROM journal_entries \
         WHERE community_id = $1 AND entry_type = 'treasury_transfer' \
           AND auction_id = $2",
    )
    .bind(community_id)
    .bind(auction_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(
        note.as_deref(),
        Some("Forgiven card remainder below charge minimum")
    );
    Ok(())
}

/// Pending captures count as succeeded uniformly: a later bid is
/// backed by an earlier auction's still-pending capture (no hold is
/// minted at all), its settlement sizes against the incoming amount
/// rather than over-capturing the transiently negative balance, and
/// the pending capture then lands and restores the balance.
#[tokio::test]
async fn pending_capture_counts_in_sizing() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(4)).await?;
    let (space_a, auction_a) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    // Bob wins A (10 = 4 balance + 6 card), but the capture is stuck:
    // the mock intent is held un-capturable, so the row stays
    // capture_pending across ticks (worker backoff).
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_a).await?;
    app.client.create_bid(&space_a, &rounds[0].round_id).await?;
    let pi_a = intent_rows(&app, &auction_a).await?[0]
        .payment_intent_id
        .clone()
        .unwrap();
    set_mock_intent_status(&app, &pi_a, "processing");
    run_until_ended(&app, &[auction_a]).await?;
    let rows = capture_rows(&app, &auction_a).await?;
    assert_eq!(rows[0].status, "capture_pending");
    assert_eq!(rows[0].capture_amount, Some(dec!(6)));
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(-6));

    // With 7 more in credits (balance 1) and A's 6 incoming, bob's bid
    // of 3 in B is fully covered — no hold is minted — and B settles
    // against the transiently negative balance without a card charge.
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(7)).await?;
    let (space_b, auction_b) =
        create_open_auction(&app, community_id, "site b", dec!(3)).await?;
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_b).await?;
    app.client.create_bid(&space_b, &rounds[0].round_id).await?;
    run_until_ended(&app, &[auction_b]).await?;
    let rows = capture_rows(&app, &auction_b).await?;
    assert!(rows.is_empty(), "B must not need a hold: {rows:?}");
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(-2));

    // A's capture unblocks and lands: balance recovers to
    // 4 + 7 − 10 − 3 + 6 = 4.
    set_mock_intent_status(&app, &pi_a, "requires_capture");
    clear_worker_backoff(&app).await?;
    app.tick().await;
    let rows = capture_rows(&app, &auction_a).await?;
    assert_eq!(rows[0].status, "captured");
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(4));
    Ok(())
}

/// Backing is auth-first: a hold covering the full commitment frees the
/// member's balance for outflows, and the capture is sized from what
/// balance remains at settlement — spending the freed balance grows the
/// capture to the full won amount.
#[tokio::test]
async fn outflow_of_auth_freed_balance_grows_capture() -> anyhow::Result<()> {
    use payloads::AccountOwner;

    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(4)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    // A hold for the full reserve, then the bid: the commitment is
    // entirely card-covered, so no balance is claimed.
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(10)),
        })
        .await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;

    // The freed 4 leaves via a transfer (the stored model trapped it).
    app.client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to: AccountOwner::Treasury,
            amount: dec!(4),
            note: None,
            idempotency_key: requests::ClientIdempotencyKey::new(),
        })
        .await?;

    // Settlement finds no balance left; the capture covers the full 10
    // (it would have been 6 had the balance stayed).
    run_until_ended(&app, &[auction_id]).await?;
    let rows = capture_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].capture_amount, Some(dec!(10)));
    app.tick().await;
    assert_eq!(capture_rows(&app, &auction_id).await?[0].status, "captured");
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    Ok(())
}

/// Balance freed by being outbid in another live auction counts as
/// usable at capture sizing (backing is derived from live commitments),
/// so the card is charged only what genuinely free balance can't cover.
#[tokio::test]
async fn outbid_balance_covers_capture() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    let members = app.client.get_members(&community_id).await?;
    let alice = members
        .iter()
        .find(|m| m.user.username == "alice")
        .unwrap()
        .user
        .user_id;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(10)).await?;
    credit_member(&app, community_id, alice, dec!(11)).await?;

    // Bob's 10 is fully allocated to his reserve-10 bid in B.
    let (space_b, auction_b) =
        create_open_auction(&app, community_id, "site b", dec!(10)).await?;
    app.login_bob().await?;
    let rounds_b = app.client.list_auction_rounds(&auction_b).await?;
    app.client
        .create_bid(&space_b, &rounds_b[0].round_id)
        .await?;

    // Bob bids 5 in A while B's bid still locks his balance: no surplus
    // exists yet, so the bid is necessarily card-backed.
    app.login_alice().await?;
    let (space_a, auction_a) =
        create_open_auction(&app, community_id, "site a", dec!(5)).await?;
    app.login_bob().await?;
    let rounds_a = app.client.list_auction_rounds(&auction_a).await?;
    app.client
        .create_bid(&space_a, &rounds_a[0].round_id)
        .await?;

    // Alice outbids bob in B: his lock there drops to zero, freeing
    // his balance.
    finish_round(&app, &auction_b).await?;
    let rounds_b = app.client.list_auction_rounds(&auction_b).await?;
    app.login_alice().await?;
    app.client
        .create_bid(&space_b, &rounds_b.last().unwrap().round_id)
        .await?;

    // A concludes (bob wins 5) while B is still live. B holds no balance
    // commitment anymore, so free balance covers the win: the debit lands on
    // balance, and the hold releases instead of capturing.
    run_until_ended(&app, &[auction_a]).await?;
    let rows = capture_rows(&app, &auction_a).await?;
    assert_eq!(rows.len(), 1);
    assert!(
        rows[0].status == "release_pending" || rows[0].status == "canceled",
        "A's hold must release, not capture (got {})",
        rows[0].status
    );
    assert_eq!(rows[0].capture_amount, None);
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(5));
    app.login_bob().await?;
    let funding_b = app.client.get_auction_funding(&auction_b).await?;
    assert_eq!(funding_b.balance_backing, dec!(5));
    assert_eq!(funding_b.commitment, Decimal::ZERO);

    // B settles normally: alice pays and bob's balance is intact.
    run_until_ended(&app, &[auction_b]).await?;
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(5));
    Ok(())
}

/// An out-of-band cancellation of a capture-owing intent is terminal:
/// the row fails, the uncollected amount stays as the member's negative
/// balance, and the lost credit-back shrinks their derived backing by
/// itself — no re-fit step exists.
#[tokio::test]
async fn out_of_band_cancel_fails_capture_to_debt() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(4)).await?;

    // Auction C with day-long rounds stays live throughout; bob's 4 in
    // balance backs his standing bid there.
    let mut site_details = test_helpers::site_details_b(community_id);
    site_details.name = "site c".to_string();
    let site_id = app.client.create_site(&site_details).await?;
    let mut space_details = test_helpers::space_details_a(site_id);
    space_details.reserve_price = payloads::ReservePrice(dec!(4));
    let space_c = app.client.create_space(&space_details).await?;
    let mut auction_details =
        test_helpers::auction_details_a(site_id, &app.time_source);
    auction_details.start_at = Some(app.time_source.now());
    auction_details.auction_params.round_duration = jiff::Span::new().hours(24);
    auction_details
        .auction_params
        .activity_rule_params
        .eligibility_progression = vec![];
    let auction_c = app.client.create_auction(&auction_details).await?;
    app.tick().await;
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_c).await?;
    app.client.create_bid(&space_c, &rounds[0].round_id).await?;

    // Bob wins A fully on the card (balance is tied up in C); the
    // pending capture of 10 is what keeps C's balance commitment backed.
    app.login_alice().await?;
    let (space_a, auction_a) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_a).await?;
    app.client.create_bid(&space_a, &rounds[0].round_id).await?;
    let pi_a = intent_rows(&app, &auction_a).await?[0]
        .payment_intent_id
        .clone()
        .unwrap();
    set_mock_intent_status(&app, &pi_a, "processing");
    run_until_ended(&app, &[auction_a]).await?;
    assert_eq!(
        capture_rows(&app, &auction_a).await?[0].capture_amount,
        Some(dec!(10))
    );
    let funding_c = app.client.get_auction_funding(&auction_c).await?;
    assert_eq!(funding_c.balance_backing, dec!(4));

    // The community cancels the intent from their dashboard: terminal.
    // The debt stands and C's backing derives to the balance that
    // actually exists (none).
    set_mock_intent_status(&app, &pi_a, "canceled");
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "payment_intent.canceled",
            "account": "acct_mock_1",
            "data": {"object": {
                "id": pi_a,
                "cancellation_reason": "requested_by_customer",
            }},
        }),
    )
    .await?;
    assert_eq!(capture_rows(&app, &auction_a).await?[0].status, "failed");
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(-6));
    let funding_c = app.client.get_auction_funding(&auction_c).await?;
    assert_eq!(funding_c.balance_backing, dec!(0));
    assert_eq!(funding_c.commitment, dec!(4));

    // The invariant suite as oracle: the post-failure end state is
    // legal — shortfalls tolerated, no drift.
    let report = app.reconcile().await;
    assert_eq!(report.errors().count(), 0, "{:?}", report.violations);
    Ok(())
}

/// A capture attempted after its window expired still goes to Stripe;
/// when the hold genuinely expired (Stripe canceled the intent), the
/// discovery is terminal: the row fails and the debt remains as the
/// negative balance.
#[tokio::test]
async fn missed_window_marks_failed() -> anyhow::Result<()> {
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
    // Freeze the capture so conclusion leaves the row capture_pending.
    set_mock_intent_status(&app, &pi_id, "processing");
    run_until_ended(&app, &[auction_id]).await?;
    assert_eq!(
        capture_rows(&app, &auction_id).await?[0].status,
        "capture_pending"
    );

    // Five days later the 4-day window has closed and Stripe expired
    // the hold; the capture attempt discovers the gone hold and the
    // debt persists.
    {
        let mut intents =
            app.stripe_service.mock_payment_intents.lock().unwrap();
        let intent = intents.get_mut(&pi_id).unwrap();
        intent.status = "canceled".to_string();
        intent.cancellation_reason = Some("expired".to_string());
    }
    app.time_source
        .set(app.time_source.now() + jiff::Span::new().hours(24 * 5));
    app.tick().await;
    assert_eq!(capture_rows(&app, &auction_id).await?[0].status, "failed");
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(-6));
    Ok(())
}

/// A capture that succeeded at Stripe but crashed before the local
/// commit converges to `captured` even after the capture window has
/// expired: the retry replays the capture under the same idempotency
/// key and Stripe returns the stored success, so the collected money
/// is credited instead of buried as member debt.
#[tokio::test]
async fn expired_window_replays_succeeded_capture() -> anyhow::Result<()> {
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
    // Freeze the capture so conclusion leaves the row capture_pending.
    set_mock_intent_status(&app, &pi_id, "processing");
    run_until_ended(&app, &[auction_id]).await?;
    assert_eq!(
        capture_rows(&app, &auction_id).await?[0].status,
        "capture_pending"
    );

    // The worker's capture succeeded at Stripe but the process crashed
    // before the local commit: stage the Stripe-side result of that
    // call, keyed as the worker keys it.
    let intent_id: String = sqlx::query_scalar(
        "SELECT id::TEXT FROM funding_intents WHERE auction_id = $1",
    )
    .bind(auction_id)
    .fetch_one(&app.db_pool)
    .await?;
    {
        let mut intents =
            app.stripe_service.mock_payment_intents.lock().unwrap();
        let intent = intents.get_mut(&pi_id).unwrap();
        intent.status = "succeeded".to_string();
        intent.amount_received = 600;
        intent.capture_idempotency_key = Some(format!("{intent_id}:capture"));
    }

    // Retries were delayed past the 4-day window; the attempt still
    // goes to Stripe and replays to success.
    app.time_source
        .set(app.time_source.now() + jiff::Span::new().hours(24 * 5));
    app.tick().await;
    assert_eq!(capture_rows(&app, &auction_id).await?[0].status, "captured");
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    Ok(())
}

/// A hold Stripe already canceled while its webhook was missed (e.g.
/// the API was down through an expiry) converges when the cancel
/// worker's own call reports it: the row records Stripe's truth
/// (`expired`) instead of retrying the 400 on backoff forever.
#[tokio::test]
async fn already_canceled_hold_converges_without_retry() -> anyhow::Result<()> {
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

    // Stripe expired the hold out-of-band and the webhook never
    // arrived; locally the row still looks live.
    {
        let mut intents =
            app.stripe_service.mock_payment_intents.lock().unwrap();
        let intent = intents.get_mut(&pi_id).unwrap();
        intent.status = "canceled".to_string();
        intent.cancellation_reason = Some("expired".to_string());
    }

    // The cancel path (here via auction cancellation) discovers the
    // truth from the cancel call itself and converges to `expired`
    // with no failure backoff recorded.
    app.login_alice().await?;
    app.client.cancel_auction(&auction_id).await?;
    app.tick().await;
    let rows = capture_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "expired");
    let failure_count: i16 = sqlx::query_scalar(
        "SELECT worker_failure_count FROM funding_intents \
         WHERE payment_intent_id = $1",
    )
    .bind(&pi_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(failure_count, 0);
    Ok(())
}

/// A capture attempt against a hold Stripe already canceled (webhook
/// missed) is terminal on the spot: the reserved funds are gone, so the
/// row fails and the debt stands — no retry churn.
#[tokio::test]
async fn capture_of_already_canceled_hold_fails_terminally()
-> anyhow::Result<()> {
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
    // Freeze the capture so conclusion leaves the row capture_pending.
    set_mock_intent_status(&app, &pi_id, "processing");
    run_until_ended(&app, &[auction_id]).await?;
    assert_eq!(
        capture_rows(&app, &auction_id).await?[0].status,
        "capture_pending"
    );

    // The hold gets canceled at Stripe and the webhook is missed; the
    // next capture attempt discovers it and fails terminally.
    set_mock_intent_status(&app, &pi_id, "canceled");
    clear_worker_backoff(&app).await?;
    app.tick().await;
    assert_eq!(capture_rows(&app, &auction_id).await?[0].status, "failed");
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(-6));
    Ok(())
}

/// A capture_pending row that fails pre-Stripe validation records worker
/// backoff instead of being re-selected and error-logged every tick. The
/// malformed state (NULL community stripe_account_id) is synthetic — no
/// code path nulls the account id once set — so this exercises the
/// defense-in-depth backoff, not a reachable scenario.
#[tokio::test]
async fn malformed_capture_row_records_backoff() -> anyhow::Result<()> {
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
    // Freeze the capture so conclusion leaves the row capture_pending.
    set_mock_intent_status(&app, &pi_id, "processing");
    run_until_ended(&app, &[auction_id]).await?;
    assert_eq!(
        capture_rows(&app, &auction_id).await?[0].status,
        "capture_pending"
    );

    sqlx::query(
        "UPDATE communities SET stripe_account_id = NULL WHERE id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;
    clear_worker_backoff(&app).await?;
    app.tick().await;

    // The validation failure recorded backoff; the row stays
    // non-terminal.
    let (status, failure_count): (String, i16) = sqlx::query_as(
        "SELECT status::TEXT, worker_failure_count FROM funding_intents \
         WHERE payment_intent_id = $1",
    )
    .bind(&pi_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(status, "capture_pending");
    assert_eq!(failure_count, 1);

    // Inside the backoff window the row isn't re-selected: no second
    // failure recorded.
    app.tick().await;
    let failure_count: i16 = sqlx::query_scalar(
        "SELECT worker_failure_count FROM funding_intents \
         WHERE payment_intent_id = $1",
    )
    .bind(&pi_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(failure_count, 1);
    Ok(())
}

/// Canceling an auction releases its authorizations alongside its
/// allocations, in the cancel transaction.
#[tokio::test]
async fn canceled_auction_releases_holds() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(5)),
        })
        .await?;

    app.login_alice().await?;
    app.client.cancel_auction(&auction_id).await?;
    assert_eq!(
        capture_rows(&app, &auction_id).await?[0].status,
        "release_pending"
    );
    app.tick().await;
    let rows = capture_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "canceled");
    let pi_id = rows[0].payment_intent_id.clone().unwrap();
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
    Ok(())
}

/// Deleting a canceled auction is refused while intent wind-down is in
/// flight (a release_pending hold the worker hasn't canceled at Stripe
/// yet) and succeeds once every intent row is terminal — the cascade
/// then erases only rows whose holds are already resolved.
#[tokio::test]
async fn delete_blocked_until_holds_released() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(5)),
        })
        .await?;

    app.login_alice().await?;
    app.client.cancel_auction(&auction_id).await?;
    assert_eq!(
        capture_rows(&app, &auction_id).await?[0].status,
        "release_pending"
    );
    assert_api_error(
        app.client.delete_auction(&auction_id).await,
        ApiError::AuctionHasActivePayments,
    );

    // The worker cancels the hold at Stripe; the now-terminal row no
    // longer blocks, and the delete cascades it away.
    app.tick().await;
    assert_eq!(capture_rows(&app, &auction_id).await?[0].status, "canceled");
    app.client.delete_auction(&auction_id).await?;
    assert!(capture_rows(&app, &auction_id).await?.is_empty());
    Ok(())
}

/// A cancel-side row whose capture window has already passed converges
/// to `expired` locally, with no Stripe call — the hold released on its
/// own, and rows whose PaymentIntent no call can reach (e.g. stranded
/// by a connected-account replacement) drain instead of retrying the
/// cancel forever.
#[tokio::test]
async fn expired_window_cancel_converges_locally() -> anyhow::Result<()> {
    use jiff_sqlx::ToSqlx;

    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(5)),
        })
        .await?;
    app.client
        .leave_community(&requests::LeaveCommunity { community_id })
        .await?;
    assert_eq!(
        capture_rows(&app, &auction_id).await?[0].status,
        "release_pending"
    );

    // The hold's window lapses before the worker's cancel succeeds.
    sqlx::query("UPDATE funding_intents SET capture_before = $1")
        .bind((app.time_source.now() - jiff::Span::new().hours(1)).to_sqlx())
        .execute(&app.db_pool)
        .await?;

    app.tick().await;
    let rows = capture_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "expired");
    // No cancel was issued at Stripe; the mock hold is untouched.
    let pi_id = rows[0].payment_intent_id.clone().unwrap();
    assert_eq!(
        app.stripe_service
            .mock_payment_intents
            .lock()
            .unwrap()
            .get(&pi_id)
            .unwrap()
            .status,
        "requires_capture"
    );
    Ok(())
}

/// Departure never releases an authorization backing standing bids —
/// bids are commitments that survive leaving, so the departed member's
/// win still settles and captures. Only idle holds release.
#[tokio::test]
async fn departure_keeps_auth_backing_bids() -> anyhow::Result<()> {
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
    app.client
        .leave_community(&requests::LeaveCommunity { community_id })
        .await?;

    // The bid-backing hold survives the departure.
    let rows = capture_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "authorized");

    // The auction concludes normally: bob wins and the capture repays
    // the settlement debit on his persisting account.
    app.login_alice().await?;
    run_until_ended(&app, &[auction_id]).await?;
    let rows = capture_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "captured");
    assert_eq!(rows[0].capture_amount, Some(dec!(6)));
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    Ok(())
}

/// Leaving a community releases the member's idle authorizations there
/// (no commitment, so the hold serves nothing).
#[tokio::test]
async fn departure_releases_holds() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(5)),
        })
        .await?;
    app.client
        .leave_community(&requests::LeaveCommunity { community_id })
        .await?;

    assert_eq!(
        capture_rows(&app, &auction_id).await?[0].status,
        "release_pending"
    );
    app.tick().await;
    assert_eq!(capture_rows(&app, &auction_id).await?[0].status, "canceled");
    Ok(())
}

/// A capture-owing hold captured from the dashboard before the worker
/// gets to it: the worker's capture call conflicts, and the conflict
/// arm converges from live state — the row books as captured instead of
/// looping on backoff.
#[tokio::test]
async fn dashboard_capture_of_stuck_capture_booked_by_worker()
-> anyhow::Result<()> {
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
    // Freeze the capture so conclusion leaves the row capture_pending.
    set_mock_intent_status(&app, &pi_id, "processing");
    run_until_ended(&app, &[auction_id]).await?;
    assert_eq!(
        capture_rows(&app, &auction_id).await?[0].status,
        "capture_pending"
    );

    // An organizer captures the sized amount from the dashboard; the
    // worker's own capture then conflicts and converges to captured.
    set_mock_captured(&app, &pi_id, 600);
    clear_worker_backoff(&app).await?;
    app.tick().await;
    let rows = capture_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "captured");
    assert_eq!(rows[0].capture_amount, Some(dec!(6)));
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    let entries: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM journal_entries \
         WHERE community_id = $1 AND entry_type = 'stripe_payment'",
    )
    .bind(community_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(entries, 1);

    let report = app.reconcile().await;
    assert_eq!(report.errors().count(), 0, "{:?}", report.violations);
    Ok(())
}

/// A hold owed a release but captured from the dashboard first: the
/// worker's cancel conflicts, and the conflict arm books the collected
/// money as captured instead of leaving the row wedged release_pending.
#[tokio::test]
async fn dashboard_capture_of_owed_release_booked_by_worker()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(5)),
        })
        .await?;

    app.login_alice().await?;
    app.client.cancel_auction(&auction_id).await?;
    let rows = capture_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "release_pending");
    let pi_id = rows[0].payment_intent_id.clone().unwrap();

    // The dashboard capture lands before the worker's cancel.
    set_mock_captured(&app, &pi_id, 500);
    app.tick().await;
    let rows = capture_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "captured");
    assert_eq!(rows[0].capture_amount, Some(dec!(5)));
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(5));

    let report = app.reconcile().await;
    assert_eq!(report.errors().count(), 0, "{:?}", report.violations);
    Ok(())
}
