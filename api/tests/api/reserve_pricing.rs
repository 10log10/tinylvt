//! Tests for per-space reserve prices, including negative reserves (chores).

use api::scheduler;
use jiff::Span;
use payloads::{IdempotencyKey, ReservePrice, TreasuryRecipient, requests};
use rust_decimal::Decimal;
use test_helpers::spawn_app;
use uuid::Uuid;

/// (a) Round 0 with positive reserve: first-time bid locks in the reserve
/// as the bid amount.
#[tokio::test]
async fn round_zero_positive_reserve_sets_value() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Create a space with a positive reserve price of 5.
    let mut space_details = test_helpers::space_details_a(site.site_id);
    space_details.reserve_price = ReservePrice(Decimal::new(5, 0));
    let space_id = app.client.create_space(&space_details).await?;

    // Run a single round where Alice bids on the space.
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_0 = &rounds[0];

    app.login_alice().await?;
    app.client.create_bid(&space_id, &round_0.round_id).await?;

    // Advance past round end so round 0 resolves.
    app.time_source
        .set(round_0.round_details.end_at + Span::new().seconds(1));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    let results = app
        .client
        .list_round_space_results_for_round(&round_0.round_id)
        .await?;
    let result = results.iter().find(|r| r.space_id == space_id).unwrap();
    assert_eq!(result.value, Decimal::new(5, 0));

    Ok(())
}

/// (b) Round 0 with negative reserve: chore opens at the reserve, winner is
/// credited at settlement.
#[tokio::test]
async fn round_zero_negative_reserve_settles_to_winner() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Negative reserve requires distributed_clearing (the default).
    let mut space_details = test_helpers::space_details_a(site.site_id);
    space_details.reserve_price = ReservePrice(Decimal::new(-5, 0));
    let space_id = app.client.create_space(&space_details).await?;

    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_0 = &rounds[0];

    // Bob takes on the chore.
    app.login_bob().await?;
    app.client.create_bid(&space_id, &round_0.round_id).await?;

    // The pending chore bid must not contribute negative locked balance --
    // Bob hasn't won yet, and a later round could displace him, so the
    // bid shouldn't let him pre-spend the chore compensation.
    let bob_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: None,
        })
        .await?;
    assert_eq!(bob_info.locked_balance, Decimal::ZERO);

    // Round 0 ends with Bob's chore bid, round 1 has no bids, settlement
    // triggers.
    loop {
        let rounds = app.client.list_auction_rounds(&auction_id).await?;
        let current = rounds.last().unwrap().clone();
        app.time_source
            .set(current.round_details.end_at + Span::new().seconds(1));
        scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
        let auction = app.client.get_auction(&auction_id).await?;
        if auction.end_at.is_some() {
            break;
        }
    }

    app.login_alice().await?;
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    // Distributed clearing settlement of -5 across 2 active members:
    // each active member is debited 2.5 (one half of -5),
    // Bob (winner) is credited 5 in addition to his share.
    let alice_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(alice.user.user_id),
        })
        .await?;
    let bob_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(bob.user.user_id),
        })
        .await?;
    // Alice's share of -5: she is debited 2.5 (balance becomes -2.5).
    assert_eq!(alice_info.balance, Decimal::new(-25, 1));
    // Bob: -2.5 (share) + 5 (winner credit) = 2.5.
    assert_eq!(bob_info.balance, Decimal::new(25, 1));

    Ok(())
}

/// (c) Round 0 with negative reserve but no bidders: no round_space_result
/// is created. A chore nobody takes is not paid for.
#[tokio::test]
async fn negative_reserve_no_bidders_creates_no_result() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let mut space_details = test_helpers::space_details_a(site.site_id);
    space_details.reserve_price = ReservePrice(Decimal::new(-5, 0));
    let space_id = app.client.create_space(&space_details).await?;

    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Run rounds with no bids until the auction concludes.
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    loop {
        let rounds = app.client.list_auction_rounds(&auction_id).await?;
        let current = rounds.last().unwrap().clone();
        app.time_source
            .set(current.round_details.end_at + Span::new().seconds(1));
        scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
        let auction = app.client.get_auction(&auction_id).await?;
        if auction.end_at.is_some() {
            break;
        }
    }

    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    for round in &rounds {
        let results = app
            .client
            .list_round_space_results_for_round(&round.round_id)
            .await?;
        assert!(
            !results.iter().any(|r| r.space_id == space_id),
            "expected no round_space_result for space with no bidders",
        );
    }

    // No balance changes for either member.
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    let alice_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(alice.user.user_id),
        })
        .await?;
    let bob_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(bob.user.user_id),
        })
        .await?;
    assert_eq!(alice_info.balance, Decimal::ZERO);
    assert_eq!(bob_info.balance, Decimal::ZERO);

    Ok(())
}

/// (h) Editing reserve_price on a space with auction history triggers
/// copy-on-write. A pending bid placed before any prior round result reads
/// the reserve live at settlement, so in-place edits would retroactively
/// change the bid's value -- copy-on-write retires the old space (and its
/// bids with it) instead.
#[tokio::test]
async fn reserve_price_update_triggers_copy_on_write() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let mut details = test_helpers::space_details_a(site.site_id);
    details.reserve_price = ReservePrice(Decimal::new(0, 0));
    let space_id = app.client.create_space(&details).await?;

    // Give the space auction history with a bid.
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let _ = app.client.create_auction(&auction_details).await?;
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    let auctions = app.client.list_auctions(&site.site_id).await?;
    let rounds = app
        .client
        .list_auction_rounds(&auctions[0].auction_id)
        .await?;
    app.login_bob().await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    app.login_alice().await?;

    let mut updated = details.clone();
    updated.reserve_price = ReservePrice(Decimal::new(3, 0));
    let req = requests::UpdateSpace {
        space_id,
        space_details: updated,
    };
    let result = app.client.update_space(&req).await?;
    assert!(
        result.was_copied,
        "reserve_price changes should trigger copy-on-write"
    );
    assert_eq!(
        result.space.space_details.reserve_price,
        ReservePrice(Decimal::new(3, 0)),
    );
    assert_eq!(result.old_space_id, Some(space_id));

    Ok(())
}

/// (j) Chore auction in distributed_clearing with no active members parks
/// debt on treasury; coleader redistributes via treasury_credit_operation
/// with a negative amount.
#[tokio::test]
async fn chore_settlement_parks_debt_on_treasury_then_redistributes()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Deactivate both members so the no-active-members fallback fires on
    // settlement. We do this by directly clearing community_members'
    // is_active flag via SQL.
    sqlx::query(
        "UPDATE community_members SET is_active = false
         WHERE community_id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;

    let mut space_details = test_helpers::space_details_a(site.site_id);
    space_details.reserve_price = ReservePrice(Decimal::new(-10, 0));
    let space_id = app.client.create_space(&space_details).await?;

    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_0 = &rounds[0];

    // Bob can still bid even if inactive -- bid creation isn't gated on
    // active. He'll be the winner, and the active members list (empty)
    // means the chore debt parks on treasury.
    app.login_bob().await?;
    app.client.create_bid(&space_id, &round_0.round_id).await?;

    loop {
        let rounds = app.client.list_auction_rounds(&auction_id).await?;
        let current = rounds.last().unwrap().clone();
        app.time_source
            .set(current.round_details.end_at + Span::new().seconds(1));
        scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
        let auction = app.client.get_auction(&auction_id).await?;
        if auction.end_at.is_some() {
            break;
        }
    }

    // Treasury should now be at -10 (debit), winner Bob at +10.
    app.login_alice().await?;
    let treasury = app
        .client
        .get_treasury_account(&requests::GetTreasuryAccount { community_id })
        .await?;
    assert_eq!(treasury.balance_cached, Decimal::new(-10, 0));

    // Reactivate members to redistribute.
    sqlx::query(
        "UPDATE community_members SET is_active = true
         WHERE community_id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;

    // Coleader (Alice) redistributes the chore debt by sending a negative
    // amount_per_recipient via DistributedClearing/AllActiveMembers, which
    // debits each active member and credits the treasury back to zero.
    app.client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: TreasuryRecipient::AllActiveMembers,
            amount_per_recipient: Decimal::new(-5, 0),
            note: Some("Redistribute chore debt".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    let treasury_after = app
        .client
        .get_treasury_account(&requests::GetTreasuryAccount { community_id })
        .await?;
    assert_eq!(treasury_after.balance_cached, Decimal::ZERO);

    Ok(())
}

/// (k) treasury_credit_operation rejects negative amounts in modes other
/// than (DistributedClearing, AllActiveMembers).
#[tokio::test]
async fn treasury_credit_rejects_negative_in_other_modes() -> anyhow::Result<()>
{
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_points_allocation_mode(community_id).await?;

    let result = app
        .client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: TreasuryRecipient::AllActiveMembers,
            amount_per_recipient: Decimal::new(-1, 0),
            note: None,
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await;
    assert!(
        result.is_err(),
        "negative treasury op should be rejected in points_allocation"
    );

    Ok(())
}

/// (l) Chore settlement in deferred_payment: treasury goes negative
/// (owes the winner), winner's balance goes positive. The member can
/// later mark the IOU paid externally via a member->treasury transfer.
#[tokio::test]
async fn chore_settlement_deferred_payment() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    sqlx::query(
        "UPDATE communities SET currency_mode = 'deferred_payment' \
         WHERE id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;
    let site = app.create_test_site(&community_id).await?;

    let mut space_details = test_helpers::space_details_a(site.site_id);
    space_details.reserve_price = ReservePrice(Decimal::new(-10, 0));
    let space_id = app.client.create_space(&space_details).await?;

    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_0 = &rounds[0];

    app.login_bob().await?;
    app.client.create_bid(&space_id, &round_0.round_id).await?;

    loop {
        let rounds = app.client.list_auction_rounds(&auction_id).await?;
        let current = rounds.last().unwrap().clone();
        app.time_source
            .set(current.round_details.end_at + Span::new().seconds(1));
        scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
        let auction = app.client.get_auction(&auction_id).await?;
        if auction.end_at.is_some() {
            break;
        }
    }

    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    app.login_alice().await?;
    let treasury = app
        .client
        .get_treasury_account(&requests::GetTreasuryAccount { community_id })
        .await?;
    assert_eq!(treasury.balance_cached, Decimal::new(-10, 0));
    let bob_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(bob.user.user_id),
        })
        .await?;
    assert_eq!(bob_info.balance, Decimal::new(10, 0));

    Ok(())
}

/// (m) Chore settlement in prepaid_credits: treasury goes negative,
/// winner is credited. The credits are spendable on later auctions.
#[tokio::test]
async fn chore_settlement_prepaid_credits() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    sqlx::query(
        "UPDATE communities \
            SET currency_mode = 'prepaid_credits', default_credit_limit = 0 \
            WHERE id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;
    let site = app.create_test_site(&community_id).await?;

    let mut space_details = test_helpers::space_details_a(site.site_id);
    space_details.reserve_price = ReservePrice(Decimal::new(-10, 0));
    let space_id = app.client.create_space(&space_details).await?;

    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_0 = &rounds[0];

    app.login_bob().await?;
    app.client.create_bid(&space_id, &round_0.round_id).await?;

    loop {
        let rounds = app.client.list_auction_rounds(&auction_id).await?;
        let current = rounds.last().unwrap().clone();
        app.time_source
            .set(current.round_details.end_at + Span::new().seconds(1));
        scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
        let auction = app.client.get_auction(&auction_id).await?;
        if auction.end_at.is_some() {
            break;
        }
    }

    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    app.login_alice().await?;
    let treasury = app
        .client
        .get_treasury_account(&requests::GetTreasuryAccount { community_id })
        .await?;
    assert_eq!(treasury.balance_cached, Decimal::new(-10, 0));
    let bob_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(bob.user.user_id),
        })
        .await?;
    assert_eq!(bob_info.balance, Decimal::new(10, 0));

    Ok(())
}
