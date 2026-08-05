//! Stripe-backed auction funding tests. Schema-level constraint
//! round-trips (the partial unique indexes the concurrency design leans
//! on, the stripe_payment ledger linkage) and the balance-only derived
//! backing: the bid gate, outflow gating, and settlement.

use jiff::Span;
use payloads::{
    AccountOwner, ApiError, AuctionId, CommunityId, ReservePrice,
    TreasuryRecipient, UserId, requests,
};
use rust_decimal::{Decimal, dec};
use test_helpers::{TestApp, assert_api_error, spawn_app};
use uuid::Uuid;

pub async fn credit_member(
    app: &TestApp,
    community_id: CommunityId,
    user_id: UserId,
    amount: Decimal,
) -> anyhow::Result<()> {
    app.client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: TreasuryRecipient::SingleMember(user_id),
            amount_per_recipient: amount,
            note: None,
            idempotency_key: requests::ClientIdempotencyKey::new(),
        })
        .await?;
    Ok(())
}

/// Create an auction starting now with no eligibility requirements (so
/// any member can join in any round) on a space with the given reserve.
/// `site_name` must be unique within the community.
pub async fn create_open_auction(
    app: &TestApp,
    community_id: CommunityId,
    site_name: &str,
    reserve: Decimal,
) -> anyhow::Result<(payloads::SpaceId, AuctionId)> {
    let mut site_details = test_helpers::site_details_b(community_id);
    site_details.name = site_name.to_string();
    let site_id = app.client.create_site(&site_details).await?;
    let mut space_details = test_helpers::space_details_a(site_id);
    space_details.reserve_price = ReservePrice(reserve);
    let space_id = app.client.create_space(&space_details).await?;
    let mut auction_details =
        test_helpers::auction_details_a(site_id, &app.time_source);
    auction_details.start_at = Some(app.time_source.now());
    auction_details
        .auction_params
        .activity_rule_params
        .eligibility_progression = vec![];
    let auction_id = app.client.create_auction(&auction_details).await?;
    app.tick().await;
    Ok((space_id, auction_id))
}

/// Advance past the current round's end and tick, processing its results
/// (and creating the next round if the auction continues).
pub async fn finish_round(
    app: &TestApp,
    auction_id: &AuctionId,
) -> anyhow::Result<()> {
    let rounds = app.client.list_auction_rounds(auction_id).await?;
    let current = rounds.last().unwrap();
    app.time_source
        .set(current.round_details.end_at + Span::new().seconds(1));
    app.tick().await;
    Ok(())
}

/// Run auctions with no further bids to conclusion.
pub async fn run_until_ended(
    app: &TestApp,
    auction_ids: &[AuctionId],
) -> anyhow::Result<()> {
    loop {
        let mut earliest_end: Option<jiff::Timestamp> = None;
        for id in auction_ids {
            let auction = app.client.get_auction(id).await?;
            if auction.end_at.is_none() {
                let rounds = app.client.list_auction_rounds(id).await?;
                let end = rounds.last().unwrap().round_details.end_at;
                earliest_end = Some(match earliest_end {
                    Some(t) if t < end => t,
                    _ => end,
                });
            }
        }
        let Some(end) = earliest_end else {
            return Ok(());
        };
        app.time_source.set(end + Span::new().seconds(1));
        app.tick().await;
    }
}

fn constraint_name(err: sqlx::Error) -> String {
    err.as_database_error()
        .and_then(|e| e.constraint())
        .unwrap_or_default()
        .to_string()
}

/// The partial unique indexes: one pending intent per (member, auction)
/// (the idempotency-key seed for creation retries) and one active
/// intent per (member, auction) (the swap flip's atomicity anchor).
#[tokio::test]
async fn funding_intent_partial_unique_indexes() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let auction = app.create_test_auction(&site.site_id).await?;
    let members = app.client.get_members(&community_id).await?;
    let alice = &members[0];

    // One pending row per (member, auction).
    let insert_pending = "INSERT INTO funding_intents \
         (auction_id, user_id, origin, requested_amount, \
          created_at, updated_at) \
         VALUES ($1, $2, 'bid_flow', 5, NOW(), NOW())";
    sqlx::query(insert_pending)
        .bind(auction.auction_id)
        .bind(alice.user.user_id)
        .execute(&app.db_pool)
        .await?;
    let err = sqlx::query(insert_pending)
        .bind(auction.auction_id)
        .bind(alice.user.user_id)
        .execute(&app.db_pool)
        .await
        .unwrap_err();
    assert_eq!(constraint_name(err), "idx_funding_intents_pending");

    // One active intent per (member, auction) (authorized status keeps
    // the rows clear of the pending index).
    let insert_active = "INSERT INTO funding_intents \
         (auction_id, user_id, payment_intent_id, is_active, status, \
          origin, requested_amount, authorized_amount, capture_before, \
          created_at, updated_at) \
         VALUES ($1, $2, $3, TRUE, 'authorized', 'bid_flow', 5, 5, \
          NOW() + INTERVAL '6 days', NOW(), NOW())";
    sqlx::query(insert_active)
        .bind(auction.auction_id)
        .bind(alice.user.user_id)
        .bind("pi_test_1")
        .execute(&app.db_pool)
        .await?;
    let err = sqlx::query(insert_active)
        .bind(auction.auction_id)
        .bind(alice.user.user_id)
        .bind("pi_test_2")
        .execute(&app.db_pool)
        .await
        .unwrap_err();
    assert_eq!(constraint_name(err), "idx_funding_intents_active");

    Ok(())
}

/// stripe_payment entries must carry their PaymentIntent linkage; the
/// generated constraint name is also what the down migration drops.
#[tokio::test]
async fn stripe_payment_requires_payment_intent_id() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    let insert = "INSERT INTO journal_entries \
         (community_id, entry_type, idempotency_key, payment_intent_id, \
          created_at) \
         VALUES ($1, 'stripe_payment', $2, $3, NOW())";
    let err = sqlx::query(insert)
        .bind(community_id)
        .bind(Uuid::new_v4())
        .bind(Option::<String>::None)
        .execute(&app.db_pool)
        .await
        .unwrap_err();
    assert_eq!(constraint_name(err), "journal_entries_check1");

    sqlx::query(insert)
        .bind(community_id)
        .bind(Uuid::new_v4())
        .bind(Some("pi_test_3"))
        .execute(&app.db_pool)
        .await?;

    Ok(())
}

/// The status-conditional CHECKs: post-authorization intent rows must
/// carry the Stripe facts set at activation (code decodes them as
/// non-optional), capture_pending additionally the capture target, and
/// a payment profile's card columns are all-or-nothing.
#[tokio::test]
async fn funding_schema_checks() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let auction = app.create_test_auction(&site.site_id).await?;
    let members = app.client.get_members(&community_id).await?;
    let alice = &members[0];

    // An authorized row without authorized_amount/capture_before.
    let err = sqlx::query(
        "INSERT INTO funding_intents \
         (auction_id, user_id, payment_intent_id, status, origin, \
          requested_amount, created_at, updated_at) \
         VALUES ($1, $2, 'pi_check_1', 'authorized', 'bid_flow', 5, \
          NOW(), NOW())",
    )
    .bind(auction.auction_id)
    .bind(alice.user.user_id)
    .execute(&app.db_pool)
    .await
    .unwrap_err();
    assert_eq!(
        constraint_name(err),
        "funding_intents_authorized_columns_check"
    );

    // A capture_pending row without capture_amount.
    let err = sqlx::query(
        "INSERT INTO funding_intents \
         (auction_id, user_id, payment_intent_id, status, origin, \
          requested_amount, authorized_amount, capture_before, \
          created_at, updated_at) \
         VALUES ($1, $2, 'pi_check_2', 'capture_pending', 'bid_flow', 5, 5, \
          NOW() + INTERVAL '6 days', NOW(), NOW())",
    )
    .bind(auction.auction_id)
    .bind(alice.user.user_id)
    .execute(&app.db_pool)
    .await
    .unwrap_err();
    assert_eq!(constraint_name(err), "funding_intents_capture_amount_check");

    // A card row with only some display columns set.
    let err = sqlx::query(
        "INSERT INTO user_payment_profiles \
         (user_id, stripe_customer_id, payment_method_id, \
          created_at, updated_at) \
         VALUES ($1, 'cus_check', 'pm_check', NOW(), NOW())",
    )
    .bind(alice.user.user_id)
    .execute(&app.db_pool)
    .await
    .unwrap_err();
    assert_eq!(
        constraint_name(err),
        "user_payment_profiles_card_columns_check"
    );

    Ok(())
}

/// Bidding in backed_credits gates on derived backing (balance commitments
/// need no writes); settlement debits the winner exactly as informal
/// modes do (same entry shape), and the concluded auction drops out of
/// the funding view.
#[tokio::test]
async fn bid_backing_lifecycle_and_settlement() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_backed_credits_mode(&community_id).await?;
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    credit_member(&app, community_id, bob.user.user_id, dec!(50)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;

    // The full balance backs this auction (nothing else commits it).
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.balance_backing, dec!(50));
    assert_eq!(funding.commitment, dec!(10));

    // Round 0 processes (bob winning at the reserve), then a bid-less
    // round concludes the auction and settles.
    run_until_ended(&app, &[auction_id]).await?;

    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.balance_backing, Decimal::ZERO);
    assert_eq!(funding.commitment, Decimal::ZERO);

    app.login_alice().await?;
    let bob_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(bob.user.user_id),
        })
        .await?;
    assert_eq!(bob_info.balance, dec!(40));

    // Ledger parity with informal-mode settlement: one auction_settlement
    // entry, winner debited the full amount, treasury credited.
    let amounts: Vec<Decimal> = sqlx::query_scalar(
        "SELECT jl.amount FROM journal_lines jl \
         JOIN journal_entries je ON jl.entry_id = je.id \
         WHERE je.auction_id = $1 AND je.entry_type = 'auction_settlement' \
         ORDER BY jl.amount",
    )
    .bind(auction_id)
    .fetch_all(&app.db_pool)
    .await?;
    assert_eq!(amounts, vec![dec!(-10), dec!(10)]);

    Ok(())
}

/// A bid the member's balance cannot back is rejected exactly like the
/// informal modes' credit check; topping up makes the same bid succeed.
#[tokio::test]
async fn bid_exceeding_balance_rejected() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_backed_credits_mode(&community_id).await?;
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    credit_member(&app, community_id, bob.user.user_id, dec!(5)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;

    app.login_bob().await?;
    let result = app.client.create_bid(&space_id, &rounds[0].round_id).await;
    assert_api_error(result, ApiError::InsufficientBalance);

    app.login_alice().await?;
    credit_member(&app, community_id, bob.user.user_id, dec!(5)).await?;
    app.login_bob().await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;

    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.balance_backing, dec!(10));

    Ok(())
}

/// Backing is derived from live commitments, so being outbid frees the
/// balance immediately for another auction's bid — no release or
/// reclaim step exists.
#[tokio::test]
async fn outbid_balance_immediately_backs_other_auction() -> anyhow::Result<()>
{
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_backed_credits_mode(&community_id).await?;
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    credit_member(&app, community_id, bob.user.user_id, dec!(10)).await?;
    credit_member(&app, community_id, alice.user.user_id, dec!(11)).await?;
    let (space_a, auction_a) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    // Round 0: bob bids the reserve.
    let rounds = app.client.list_auction_rounds(&auction_a).await?;
    app.login_bob().await?;
    app.client.create_bid(&space_a, &rounds[0].round_id).await?;
    finish_round(&app, &auction_a).await?;

    // Round 1: alice outbids at reserve + increment.
    let rounds = app.client.list_auction_rounds(&auction_a).await?;
    app.login_alice().await?;
    app.client
        .create_bid(&space_a, &rounds.last().unwrap().round_id)
        .await?;
    finish_round(&app, &auction_a).await?;

    // Bob is outbid: his commitment in A is gone, so his balance is
    // fully free again — auction A holds no balance commitment on it.
    app.login_bob().await?;
    let funding_a = app.client.get_auction_funding(&auction_a).await?;
    assert_eq!(funding_a.commitment, Decimal::ZERO);
    assert_eq!(funding_a.balance_backing, dec!(10));

    // A bid in a second auction spends it immediately.
    app.login_alice().await?;
    let (space_b, auction_b) =
        create_open_auction(&app, community_id, "site b", dec!(10)).await?;
    let rounds_b = app.client.list_auction_rounds(&auction_b).await?;
    app.login_bob().await?;
    app.client
        .create_bid(&space_b, &rounds_b[0].round_id)
        .await?;

    let funding_b = app.client.get_auction_funding(&auction_b).await?;
    assert_eq!(funding_b.commitment, dec!(10));
    // B's commitment now requires the balance, leaving A nothing.
    let funding_a = app.client.get_auction_funding(&auction_a).await?;
    assert_eq!(funding_a.balance_backing, Decimal::ZERO);

    // Both auctions settle: alice pays 11 in A, bob pays 10 in B, and
    // the treasury's net position returns to the credits it issued.
    run_until_ended(&app, &[auction_a, auction_b]).await?;
    app.login_alice().await?;
    let treasury = app
        .client
        .get_treasury_account(&requests::GetTreasuryAccount { community_id })
        .await?;
    assert_eq!(treasury.balance_cached, Decimal::ZERO);

    Ok(())
}

/// Outflows only spend unclaimed balance: a transfer that would eat a
/// live bid's backing is rejected, while an outbid commitment's balance
/// commitment vanishes with it — nothing traps the funds.
#[tokio::test]
async fn transfer_gated_on_unclaimed_balance() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_backed_credits_mode(&community_id).await?;
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    credit_member(&app, community_id, bob.user.user_id, dec!(50)).await?;
    credit_member(&app, community_id, alice.user.user_id, dec!(31)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(30)).await?;

    // Bob bids: 30 committed (requiring 30 of balance), 20 uncommitted.
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.login_bob().await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;

    // The live bid's backing is not spendable.
    let result = app
        .client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to: AccountOwner::Treasury,
            amount: dec!(30),
            note: None,
            idempotency_key: requests::ClientIdempotencyKey::new(),
        })
        .await;
    assert_api_error(result, ApiError::InsufficientBalance);

    // The unallocated remainder is.
    app.client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to: AccountOwner::Treasury,
            amount: dec!(20),
            note: None,
            idempotency_key: requests::ClientIdempotencyKey::new(),
        })
        .await?;

    // Alice outbids bob; his balance commitment vanishes with it, so
    // the remaining 30 is transferable.
    finish_round(&app, &auction_id).await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.login_alice().await?;
    app.client
        .create_bid(&space_id, &rounds.last().unwrap().round_id)
        .await?;
    finish_round(&app, &auction_id).await?;

    app.login_bob().await?;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.commitment, Decimal::ZERO);
    assert_eq!(funding.balance_backing, dec!(30));

    app.client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to: AccountOwner::Treasury,
            amount: dec!(30),
            note: None,
            idempotency_key: requests::ClientIdempotencyKey::new(),
        })
        .await?;

    let bob_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: None,
        })
        .await?;
    assert_eq!(bob_info.balance, Decimal::ZERO);

    Ok(())
}

/// A bid claim that passes the liveness check, then blocks on the
/// member's account row while the conclusion transaction commits, is
/// rejected on resume instead of writing a bid row and allocation onto
/// the concluded auction. The account row is the one serialization
/// point between claims and conclusion (disjoint advisory locks), so
/// the test holds it directly while concluding.
#[tokio::test]
async fn bid_blocked_across_conclusion_is_rejected() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_backed_credits_mode(&community_id).await?;
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    credit_member(&app, community_id, bob.user.user_id, dec!(100)).await?;

    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_id = rounds[0].round_id;
    let round_end = rounds[0].round_details.end_at;

    // Hold bob's account row lock, standing in for the settlement
    // entry's debit lock inside a conclusion transaction.
    let mut blocker = app.db_pool.begin().await?;
    sqlx::query(
        "SELECT id FROM accounts WHERE community_id = $1 \
         AND owner_type = 'member_main' AND owner_id = $2 FOR UPDATE",
    )
    .bind(community_id)
    .bind(bob.user.user_id)
    .fetch_one(&mut *blocker)
    .await?;

    // Bob's bid passes the pre-lock liveness check (the round is live)
    // and queues behind the held account lock.
    app.login_bob().await?;
    let bid_client = payloads::APIClient {
        address: app.client.address.clone(),
        inner_client: app.client.inner_client.clone(),
    };
    let bid = tokio::spawn(async move {
        bid_client.create_bid(&space_id, &round_id).await
    });

    // Wait until the claim is genuinely parked on the row lock.
    let mut parked = false;
    for _ in 0..500 {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity \
             WHERE datname = current_database() \
               AND wait_event_type = 'Lock'",
        )
        .fetch_one(&app.db_pool)
        .await?;
        if waiting > 0 {
            parked = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(parked, "bid claim never blocked on the account lock");

    // The auction concludes while the claim is blocked: no committed
    // bids, so the round's end concludes it, and conclusion never
    // touches bob's account row (he won nothing).
    app.time_source.set(round_end + Span::new().seconds(1));
    app.tick().await;
    let auction = app.client.get_auction(&auction_id).await?;
    assert!(auction.end_at.is_some(), "auction should have concluded");

    // Release the lock; the woken claim re-checks liveness and rejects.
    blocker.rollback().await?;
    let result = bid.await?;
    assert_api_error(result, ApiError::RoundEnded);

    let bids: i64 =
        sqlx::query_scalar("SELECT count(*) FROM bids WHERE round_id = $1")
            .bind(round_id)
            .fetch_one(&app.db_pool)
            .await?;
    assert_eq!(bids, 0, "no bid row may land in the concluded round");
    let report = app.reconcile().await;
    assert!(
        report.violations.is_empty(),
        "unexpected findings: {:?}",
        report.violations
    );

    Ok(())
}
