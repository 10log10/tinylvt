//! Scheduling, expiry, and notification tests (phase 7): the T−24h
//! scheduled pre-auth task, the manual pre-authorize advance gate, the
//! age scan with its origin-split notifications, the stranded
//! pending-order age-out, the short-window mint predicate, the
//! time-based runaway cancel, and the notification outbox (decline,
//! backing-lost, capture-receipt emails).

use jiff::Span;
use payloads::{
    ApiError, AuctionId, CommunityId, requests,
    requests::{AuthorizeFunding, ScheduleAuction, UseProxyBidding},
};
use rust_decimal::dec;
use serde_json::json;
use test_helpers::{TestApp, assert_api_error, spawn_app};

use crate::funding::{create_open_auction, credit_member, run_until_ended};
use crate::funding_auth::{bob_user_id, card_enabled_setup, intent_rows};

/// Create an auction on a fresh site whose start is `start_from_now`
/// away (None = start unknown), without ticking the scheduler. The
/// logged-in user must be able to create sites (alice).
pub async fn create_scheduled_auction(
    app: &TestApp,
    community_id: CommunityId,
    site_name: &str,
    reserve: rust_decimal::Decimal,
    start_from_now: Option<Span>,
) -> anyhow::Result<(payloads::SpaceId, AuctionId)> {
    let mut site_details = test_helpers::site_details_b(community_id);
    site_details.name = site_name.to_string();
    let site_id = app.client.create_site(&site_details).await?;
    let mut space_details = test_helpers::space_details_a(site_id);
    space_details.reserve_price = payloads::ReservePrice(reserve);
    let space_id = app.client.create_space(&space_details).await?;
    let mut auction_details =
        test_helpers::auction_details_a(site_id, &app.time_source);
    let now = app.time_source.now();
    auction_details.start_at = start_from_now.map(|s| now + s);
    auction_details.possession_start_at = now + Span::new().hours(24 * 30);
    auction_details.possession_end_at = now + Span::new().hours(24 * 31);
    auction_details
        .auction_params
        .activity_rule_params
        .eligibility_progression = vec![];
    let auction_id = app.client.create_auction(&auction_details).await?;
    Ok((space_id, auction_id))
}

/// The intent origins for an auction, in creation order.
async fn intent_origins(
    app: &TestApp,
    auction_id: &AuctionId,
) -> anyhow::Result<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT origin::TEXT FROM funding_intents \
         WHERE auction_id = $1 ORDER BY created_at",
    )
    .bind(auction_id)
    .fetch_all(&app.db_pool)
    .await?)
}

/// Force a mock PaymentIntent into the out-of-band canceled state, as
/// a dashboard cancel leaves it — pokes retrieve live state, so tests
/// must stage the truth, not just the event payload.
fn set_mock_canceled(app: &TestApp, pi_id: &str) {
    let mut intents = app.stripe_service.mock_payment_intents.lock().unwrap();
    let intent = intents.get_mut(pi_id).unwrap();
    intent.status = "canceled".to_string();
    intent.cancellation_reason = Some("requested_by_customer".to_string());
}

/// Captured mock emails whose subject contains `needle`.
pub fn emails_matching(app: &TestApp, needle: &str) -> Vec<(String, String)> {
    app.email_service
        .mock_sent_emails
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.subject.contains(needle))
        .map(|e| (e.to.clone(), e.subject.clone()))
        .collect()
}

/// The T−24h task mints a strategy-sized `scheduled_preauth` hold for a
/// proxy participant with card + grant once the known start enters the
/// advance window — not before — and re-running is a no-op while the
/// hold is live.
#[tokio::test]
async fn scheduled_preauth_mints_at_advance_window() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (space_id, auction_id) = create_scheduled_auction(
        &app,
        community_id,
        "site a",
        dec!(10),
        Some(Span::new().hours(48)),
    )
    .await?;

    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id,
            value: dec!(40),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    // Start is 48h out — beyond the 24h advance; nothing mints.
    app.tick().await;
    assert!(intent_rows(&app, &auction_id).await?.is_empty());

    // 23h before start: the task mints the budget-sized hold.
    app.time_source.advance(Span::new().hours(25));
    app.tick().await;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "authorized");
    assert!(rows[0].is_active);
    assert_eq!(rows[0].authorized_amount, Some(dec!(40)));
    assert_eq!(
        intent_origins(&app, &auction_id).await?,
        vec!["scheduled_preauth"]
    );

    // Idempotent while the hold is live.
    app.tick().await;
    assert_eq!(intent_rows(&app, &auction_id).await?.len(), 1);
    Ok(())
}

/// The scheduled pre-auth task never shrinks a hold: a member-placed
/// hold above the strategy target is left alone (the task no-ops as
/// covered), while the member-present resize is the only path that
/// reduces.
#[tokio::test]
async fn scheduled_preauth_never_shrinks() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (space_id, auction_id) = create_scheduled_auction(
        &app,
        community_id,
        "site a",
        dec!(10),
        Some(Span::new().hours(12)),
    )
    .await?;

    // Inside the 24h advance window: bob manually authorizes well above
    // the 8 his budget strategy would size.
    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id,
            value: dec!(8),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;
    app.client
        .authorize_funding(&AuthorizeFunding {
            auction_id,
            amount: Some(dec!(30)),
        })
        .await?;

    // The scheduled task selects bob but leaves the larger hold alone.
    app.tick().await;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].authorized_amount, Some(dec!(30)));
    assert_eq!(
        intent_origins(&app, &auction_id).await?,
        vec!["member_preauth"]
    );
    Ok(())
}

/// Members without a saved card never select for scheduled pre-auth —
/// their proxies bid balance-only.
#[tokio::test]
async fn scheduled_preauth_skips_without_card() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_backed_credits_mode(&community_id).await?;
    sqlx::query(
        "UPDATE communities SET stripe_account_id = 'acct_mock_1', \
         stripe_charges_enabled = TRUE WHERE id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;
    let (space_id, auction_id) = create_scheduled_auction(
        &app,
        community_id,
        "site a",
        dec!(10),
        Some(Span::new().hours(12)),
    )
    .await?;

    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id,
            value: dec!(40),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    app.tick().await;
    assert!(intent_rows(&app, &auction_id).await?.is_empty());
    Ok(())
}

/// The manual pre-authorize endpoint opens the uniform 24h advance
/// before a known start — earlier requests reject with the opening
/// time — and accepts once inside the window.
#[tokio::test]
async fn preauth_advance_gate() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let start_at = app.time_source.now() + Span::new().hours(48);
    let (_, auction_id) = create_scheduled_auction(
        &app,
        community_id,
        "site a",
        dec!(10),
        Some(Span::new().hours(48)),
    )
    .await?;

    app.login_bob().await?;
    let result = app
        .client
        .authorize_funding(&AuthorizeFunding {
            auction_id,
            amount: Some(dec!(5)),
        })
        .await;
    assert_api_error(
        result,
        ApiError::PreauthNotYetOpen {
            authorize_from: start_at - Span::new().hours(24),
        },
    );
    assert!(intent_rows(&app, &auction_id).await?.is_empty());

    // 23h before start: inside the window; the hold mints.
    app.time_source.advance(Span::new().hours(25));
    app.client
        .authorize_funding(&AuthorizeFunding {
            auction_id,
            amount: Some(dec!(5)),
        })
        .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "authorized");
    assert_eq!(
        intent_origins(&app, &auction_id).await?,
        vec!["member_preauth"]
    );
    Ok(())
}

/// The age scan cancels a member-initiated pre-start hold whose
/// remaining window can no longer cover an immediate start, and
/// notifies once ("authorize again when the start is announced").
#[tokio::test]
async fn member_preauth_age_cancel_notifies() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    // Start unknown: authorization is allowed anytime, and holds that
    // linger recycle through the age scan.
    let (_, auction_id) =
        create_scheduled_auction(&app, community_id, "site a", dec!(10), None)
            .await?;

    app.login_bob().await?;
    app.client
        .authorize_funding(&AuthorizeFunding {
            auction_id,
            amount: Some(dec!(6)),
        })
        .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    let pi_id = rows[0].payment_intent_id.clone().unwrap();

    // 37h later the mock hold (assumed 4d window) retains 59h — under
    // the 60h viability floor (runtime 48h + margin 12h).
    app.time_source.advance(Span::new().hours(37));
    app.tick().await;

    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "canceled");
    assert!(!rows[0].is_active);
    let status = {
        let intents = app.stripe_service.mock_payment_intents.lock().unwrap();
        intents.get(&pi_id).unwrap().status.clone()
    };
    assert_eq!(status, "canceled");

    let sent = emails_matching(&app, "was released");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "bob@example.com");

    // The notice never repeats for the same hold.
    app.tick().await;
    assert_eq!(emails_matching(&app, "was released").len(), 1);
    Ok(())
}

/// A stranded `pending` order — its execute never ran (crash between
/// the order and execute transactions) — ages out: the intent worker
/// cancels it locally with no Stripe call once past the stale-order
/// threshold, so a later genuine need orders fresh at its own size
/// instead of reusing the stale amount.
#[tokio::test]
async fn stranded_pending_order_ages_out() -> anyhow::Result<()> {
    use jiff_sqlx::ToSqlx;

    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    let (_, auction_id) = create_scheduled_auction(
        &app,
        community_id,
        "site a",
        dec!(10),
        Some(Span::new().hours(26)),
    )
    .await?;

    // A committed 50 order that never executed.
    let now = app.time_source.now().to_sqlx();
    sqlx::query(
        "INSERT INTO funding_intents \
         (auction_id, user_id, status, origin, requested_amount, \
          created_at, updated_at) \
         VALUES ($1, $2, 'pending', 'bid_flow', 50, $3, $3)",
    )
    .bind(auction_id)
    .bind(bob)
    .bind(now)
    .execute(&app.db_pool)
    .await?;

    // Younger than the threshold: left alone for a retrying execute or
    // an adoption webhook.
    app.tick().await;
    assert_eq!(intent_rows(&app, &auction_id).await?[0].status, "pending");

    // Aged out: canceled locally — no Stripe object was ever created.
    app.time_source.advance(Span::new().hours(2));
    app.tick().await;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "canceled");
    assert_eq!(rows[0].payment_intent_id, None);
    assert!(
        app.stripe_service
            .mock_payment_intents
            .lock()
            .unwrap()
            .is_empty()
    );

    // The next genuine need orders fresh at its own size — without the
    // age-out, the immutable-order reuse would execute this as a 50
    // hold.
    app.login_bob().await?;
    app.client
        .authorize_funding(&AuthorizeFunding {
            auction_id,
            amount: Some(dec!(12)),
        })
        .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].status, "authorized");
    assert!(rows[1].is_active);
    assert_eq!(rows[1].authorized_amount, Some(dec!(12)));
    Ok(())
}

/// A scheduled hold outdated by a postponement age-cancels silently,
/// and the T−24h task re-mints a fresh hold once the new start enters
/// the advance window.
#[tokio::test]
async fn scheduled_hold_recycles_after_postponement() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (space_id, auction_id) = create_scheduled_auction(
        &app,
        community_id,
        "site a",
        dec!(10),
        Some(Span::new().hours(20)),
    )
    .await?;

    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id,
            value: dec!(40),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;
    app.tick().await;
    assert_eq!(intent_rows(&app, &auction_id).await?.len(), 1);

    // Postpone the start beyond what the hold's window can cover: the
    // hold ends at +96h but the new deadline needs +90h + 60h. The age
    // scan is start-aware, so the very next tick discovers the
    // invalidation — no waiting until the hold nears the start.
    app.login_alice().await?;
    app.client
        .schedule_auction(&ScheduleAuction {
            auction_id,
            start_at: Some(app.time_source.now() + Span::new().hours(90)),
        })
        .await?;
    app.tick().await;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "canceled");
    assert!(emails_matching(&app, "was released").is_empty());

    // 23h before the new start: a fresh scheduled hold mints.
    app.time_source.advance(Span::new().hours(67));
    app.tick().await;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].status, "authorized");
    assert!(rows[1].is_active);
    assert_eq!(
        intent_origins(&app, &auction_id).await?,
        vec!["scheduled_preauth", "scheduled_preauth"]
    );
    Ok(())
}

/// A fresh authorization whose capture window can't cover the auction's
/// fixed deadline plus margin is cancel-and-reject: the member gets a
/// decline-shaped contextual error, and a normal-window retry works.
#[tokio::test]
async fn short_window_auth_rejected() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    // The card reports a 50h window; the started auction's deadline
    // needs 48h runtime + 12h margin = 60h.
    *app.stripe_service.mock_capture_before_epoch.lock().unwrap() =
        Some((app.time_source.now() + Span::new().hours(50)).as_second());

    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let result = app.client.create_bid(&space_id, &rounds[0].round_id).await;
    assert_api_error(
        result,
        ApiError::CardDeclined {
            code: Some("hold_window_too_short".into()),
        },
    );
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "canceled");
    assert_eq!(
        rows[0].last_decline_code.as_deref(),
        Some("hold_window_too_short")
    );
    let pi_id = rows[0].payment_intent_id.clone().unwrap();
    let status = {
        let intents = app.stripe_service.mock_payment_intents.lock().unwrap();
        intents.get(&pi_id).unwrap().status.clone()
    };
    assert_eq!(status, "canceled");

    // A member-present retry with a normal window succeeds (the decline
    // pause applies to automatic attempts only).
    *app.stripe_service.mock_capture_before_epoch.lock().unwrap() = None;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(10));
    Ok(())
}

/// A backed-mode auction still live past its fixed deadline (start +
/// runtime) cancels via the runaway path: allocations release and the
/// holds cancel at Stripe — nobody pays.
#[tokio::test]
async fn runaway_deadline_cancels_auction() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = crate::funding_auth::bob_user_id(&app, &community_id).await?;
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

    // Simulate an auction that has been running past its deadline: back
    // the start up 3 days, then process the ended round.
    sqlx::query(
        "UPDATE auctions SET start_at = start_at - INTERVAL '3 days' \
         WHERE id = $1",
    )
    .bind(auction_id)
    .execute(&app.db_pool)
    .await?;
    let round_end = rounds[0].round_details.end_at;
    app.time_source.set(round_end + Span::new().seconds(1));
    app.tick().await;

    let auction = app.client.get_auction(&auction_id).await?;
    assert!(auction.end_at.is_some());
    assert!(auction.was_canceled);
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.balance_backing, dec!(0));
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "canceled");
    let status = {
        let intents = app.stripe_service.mock_payment_intents.lock().unwrap();
        intents.get(&pi_id).unwrap().status.clone()
    };
    assert_eq!(status, "canceled");
    Ok(())
}

/// A backed-mode auction whose next round would end past the runaway
/// deadline cancels at round creation even though the deadline itself
/// hasn't passed yet: no round may straddle the deadline, which keeps
/// every hold's capture window covering conclusion by construction.
#[tokio::test]
async fn runaway_straddling_round_cancels_auction() -> anyhow::Result<()> {
    use jiff_sqlx::ToSqlx;

    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    let pi_id = rows[0].payment_intent_id.clone().unwrap();

    // Rounds are one minute; move the start back so the deadline falls
    // 30 seconds after round 0 ends — after round 0 concludes, but
    // before round 1 would end.
    let round_end = rounds[0].round_details.end_at;
    let new_start = round_end + Span::new().seconds(30)
        - Span::new().hours(payloads::AUCTION_RUNAWAY_DEADLINE_HOURS);
    sqlx::query("UPDATE auctions SET start_at = $1 WHERE id = $2")
        .bind(new_start.to_sqlx())
        .bind(auction_id)
        .execute(&app.db_pool)
        .await?;

    // Process round 0 one second after it ends, still before the
    // deadline: the auction cancels rather than create a straddling
    // round.
    app.time_source.set(round_end + Span::new().seconds(1));
    app.tick().await;

    let auction = app.client.get_auction(&auction_id).await?;
    assert!(auction.was_canceled);
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 1);
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.balance_backing, dec!(0));
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "canceled");
    let status = {
        let intents = app.stripe_service.mock_payment_intents.lock().unwrap();
        intents.get(&pi_id).unwrap().status.clone()
    };
    assert_eq!(status, "canceled");
    Ok(())
}

/// A proxy-context decline enqueues the card-action-needed email
/// exactly once: the decline pause stops repeat attempts, and the
/// outbox dedup key stops repeat sends.
#[tokio::test]
async fn proxy_decline_notifies_once() -> anyhow::Result<()> {
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
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id,
            value: dec!(40),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;
    app.tick().await;

    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "canceled");
    assert_eq!(
        rows[0].last_decline_code.as_deref(),
        Some("insufficient_funds")
    );
    let sent = emails_matching(&app, "card hold declined");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "bob@example.com");

    app.tick().await;
    assert_eq!(emails_matching(&app, "card hold declined").len(), 1);
    Ok(())
}

/// An out-of-band cancellation of an authorization backing standing
/// bids notifies the member and the community leadership.
#[tokio::test]
async fn out_of_band_cancel_notifies() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    let pi_id = rows[0].payment_intent_id.clone().unwrap();

    set_mock_canceled(&app, &pi_id);
    api::store::connect::handle_connect_webhook_event(
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
    app.tick().await;

    let member = emails_matching(&app, "Your card hold in");
    assert_eq!(member.len(), 1);
    assert_eq!(member[0].0, "bob@example.com");
    let steward = emails_matching(&app, "canceled outside TinyLVT");
    assert_eq!(steward.len(), 1);
    assert_eq!(steward[0].0, "alice@example.com");
    Ok(())
}

/// An out-of-band cancellation of an active hold with no standing bids
/// still notifies the member — an active hold means they wanted it
/// active until the auction concludes — but not the community
/// leadership, since nothing was backing bids.
#[tokio::test]
async fn out_of_band_cancel_idle_hold_notifies_member_only()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_scheduled_auction(&app, community_id, "site a", dec!(10), None)
            .await?;

    app.login_bob().await?;
    app.client
        .authorize_funding(&AuthorizeFunding {
            auction_id,
            amount: Some(dec!(6)),
        })
        .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    let pi_id = rows[0].payment_intent_id.clone().unwrap();

    set_mock_canceled(&app, &pi_id);
    api::store::connect::handle_connect_webhook_event(
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
    app.tick().await;

    let member = emails_matching(&app, "Your card hold in");
    assert_eq!(member.len(), 1);
    assert_eq!(member[0].0, "bob@example.com");
    assert!(emails_matching(&app, "canceled outside TinyLVT").is_empty());
    Ok(())
}

/// A settlement capture emails the member a receipt with the charged
/// amount.
#[tokio::test]
async fn capture_receipt_emailed() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = crate::funding_auth::bob_user_id(&app, &community_id).await?;
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

    let sent = emails_matching(&app, "card payment");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "bob@example.com");
    assert!(sent[0].1.contains("$6.00"), "subject: {}", sent[0].1);
    Ok(())
}
