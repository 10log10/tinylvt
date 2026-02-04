use api::scheduler;
use jiff::Span;
use payloads::requests;
use payloads::{
    AuctionId, EntryType, IdempotencyKey, SiteId, SpaceId, TreasuryRecipient,
};
use reqwest::StatusCode;
use rust_decimal::Decimal;
use test_helpers::{TestApp, assert_status_code, spawn_app};
use uuid::Uuid;

/// Helper to run an auction to settlement
async fn run_simple_auction(
    app: &TestApp,
    site_id: SiteId,
    // Vec of (round_num, space_id, username)
    bids: Vec<(usize, SpaceId, &str)>,
) -> anyhow::Result<AuctionId> {
    // Create auction starting now
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Create initial round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

    let mut round_index = 0;
    loop {
        let rounds = app.client.list_auction_rounds(&auction_id).await?;
        let current_round = &rounds[rounds.len() - 1];

        // Place bids for this round
        for (bid_round, space_id, username) in &bids {
            if *bid_round == round_index {
                match *username {
                    "alice" => app.login_alice().await?,
                    "bob" => app.login_bob().await?,
                    "charlie" => app.login_charlie().await?,
                    _ => panic!("Unknown user"),
                }
                app.client
                    .create_bid(space_id, &current_round.round_id)
                    .await?;
            }
        }

        // Advance time past round end
        app.time_source
            .set(current_round.round_details.end_at + Span::new().seconds(1));
        scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

        // Check if auction concluded
        let auction = app.client.get_auction(&auction_id).await?;
        if auction.end_at.is_some() {
            break;
        }

        round_index += 1;
    }

    Ok(auction_id)
}

#[tokio::test]
async fn test_get_member_currency_info_own_account() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Alice (leader) checks her own balance
    let info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: None,
        })
        .await?;

    // New account should have zero balance
    assert_eq!(info.balance, Decimal::ZERO);
    // Default distributed_clearing mode - no credit limit override
    assert_eq!(info.credit_limit, None);
    // Available credit is None (unlimited) when default_credit_limit is null
    assert_eq!(info.available_credit, None);

    Ok(())
}

#[tokio::test]
async fn test_get_member_currency_info_other_account_coleader()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Get Bob's user_id from community members
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    // Alice (leader) checks Bob's balance
    let info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(bob.user.user_id),
        })
        .await?;

    // Bob's account should exist with zero balance
    assert_eq!(info.balance, Decimal::ZERO);

    Ok(())
}

#[tokio::test]
async fn test_get_member_currency_info_other_account_member_fails()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Get Alice's user_id
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();

    // Bob (member) tries to check Alice's balance
    app.login_bob().await?;

    let result = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(alice.user.user_id),
        })
        .await;

    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn test_update_credit_limit_leader() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Get Bob's user_id
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    // Alice updates Bob's credit limit
    let new_limit = Decimal::new(500, 0);
    let account = app
        .client
        .update_credit_limit_override(&requests::UpdateCreditLimitOverride {
            community_id,
            member_user_id: bob.user.user_id,
            credit_limit_override: Some(new_limit),
        })
        .await?;

    assert_eq!(account.credit_limit_override, Some(new_limit));

    // Verify via currency info
    let info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(bob.user.user_id),
        })
        .await?;

    assert_eq!(info.credit_limit, Some(new_limit));
    assert_eq!(info.available_credit, Some(new_limit));

    Ok(())
}

#[tokio::test]
async fn test_update_credit_limit_member_fails() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Get Alice's user_id
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();

    // Bob tries to update Alice's credit limit
    app.login_bob().await?;

    let result = app
        .client
        .update_credit_limit_override(&requests::UpdateCreditLimitOverride {
            community_id,
            member_user_id: alice.user.user_id,
            credit_limit_override: Some(Decimal::new(500, 0)),
        })
        .await;

    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn test_create_transfer_success() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_points_allocation_mode(community_id).await?;

    // Get user IDs
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    // Issue credits to all active members (distributed_clearing mode)
    app.client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: TreasuryRecipient::AllActiveMembers,
            amount_per_recipient: Decimal::new(50, 0),
            note: Some("Initial credit".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Alice transfers to Bob
    app.client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to_user_id: bob.user.user_id,
            amount: Decimal::new(20, 0),
            note: Some("Test transfer".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Check Alice's balance (50 - 20 = 30)
    let alice_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: None,
        })
        .await?;
    assert_eq!(alice_info.balance, Decimal::new(30, 0));

    // Check Bob's balance (50 + 20 = 70)
    let bob_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(bob.user.user_id),
        })
        .await?;
    assert_eq!(bob_info.balance, Decimal::new(70, 0));

    Ok(())
}

#[tokio::test]
async fn test_create_transfer_insufficient_balance() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Get Bob's user_id
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    // Set Alice's credit limit to 0 so she can't transfer
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    app.client
        .update_credit_limit_override(&requests::UpdateCreditLimitOverride {
            community_id,
            member_user_id: alice.user.user_id,
            credit_limit_override: Some(Decimal::ZERO),
        })
        .await?;

    // Alice tries to transfer when she has no credit
    let result = app
        .client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to_user_id: bob.user.user_id,
            amount: Decimal::new(10, 0), // Even small amount should fail
            note: Some("Too much".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await;

    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn test_create_transfer_idempotency() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_points_allocation_mode(community_id).await?;

    // Get user IDs
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    // Issue credits to all active members
    app.client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: TreasuryRecipient::AllActiveMembers,
            amount_per_recipient: Decimal::new(50, 0),
            note: Some("Initial credit".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    let idempotency_key = IdempotencyKey(Uuid::new_v4());

    // First transfer
    app.client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to_user_id: bob.user.user_id,
            amount: Decimal::new(20, 0),
            note: Some("First".into()),
            idempotency_key,
        })
        .await?;

    // Second transfer with same idempotency key should succeed
    app.client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to_user_id: bob.user.user_id,
            amount: Decimal::new(20, 0),
            note: Some("Second".into()),
            idempotency_key,
        })
        .await?;

    // Balance should only reflect one transfer
    let alice_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: None,
        })
        .await?;
    assert_eq!(alice_info.balance, Decimal::new(30, 0)); // 50 - 20

    Ok(())
}

#[tokio::test]
async fn test_get_treasury_account_coleader() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Alice (leader) gets treasury account
    let account = app
        .client
        .get_treasury_account(&requests::GetTreasuryAccount { community_id })
        .await?;

    // Treasury should have zero balance initially
    assert_eq!(account.balance_cached, Decimal::ZERO);
    // Treasury has unlimited credit
    assert_eq!(account.credit_limit_override, None);

    Ok(())
}

#[tokio::test]
async fn test_get_treasury_account_member_fails() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Bob (member) tries to get treasury account
    app.login_bob().await?;

    let result = app
        .client
        .get_treasury_account(&requests::GetTreasuryAccount { community_id })
        .await;

    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn test_treasury_credit_operation_all_active_members()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_points_allocation_mode(community_id).await?;

    // Credit all active members
    let result = app
        .client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: TreasuryRecipient::AllActiveMembers,
            amount_per_recipient: Decimal::new(50, 0),
            note: Some("Universal credit".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Should credit both Alice and Bob
    assert_eq!(result.recipient_count, 2);
    assert_eq!(result.total_amount, Decimal::new(100, 0)); // 2 * 50

    // Check Alice's balance
    let alice_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: None,
        })
        .await?;
    assert_eq!(alice_info.balance, Decimal::new(50, 0));

    // Get Bob's user_id
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    // Check Bob's balance
    let bob_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(bob.user.user_id),
        })
        .await?;
    assert_eq!(bob_info.balance, Decimal::new(50, 0));

    Ok(())
}

#[tokio::test]
async fn test_treasury_credit_operation_member_fails() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Bob tries treasury operation
    app.login_bob().await?;

    let result = app
        .client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: TreasuryRecipient::AllActiveMembers,
            amount_per_recipient: Decimal::new(50, 0),
            note: Some("Unauthorized".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await;

    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn test_get_member_transactions() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_points_allocation_mode(community_id).await?;

    // Get user IDs
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    // Perform several operations
    app.client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: TreasuryRecipient::AllActiveMembers,
            amount_per_recipient: Decimal::new(100, 0),
            note: Some("Initial credit".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    app.client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to_user_id: bob.user.user_id,
            amount: Decimal::new(20, 0),
            note: Some("Transfer to Bob".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Get Alice's transactions
    let transactions = app
        .client
        .get_member_transactions(&requests::GetMemberTransactions {
            community_id,
            member_user_id: None,
            limit: 10,
            offset: 0,
        })
        .await?;

    // Should have 2 transactions
    assert_eq!(transactions.len(), 2);

    // Verify both transactions exist (order may vary in tests due to timing)
    let notes: Vec<_> = transactions
        .iter()
        .filter_map(|t| t.note.as_ref())
        .collect();
    assert!(notes.contains(&&"Initial credit".to_string()));
    assert!(notes.contains(&&"Transfer to Bob".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_get_treasury_transactions() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_points_allocation_mode(community_id).await?;

    // Perform treasury operations
    app.client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: TreasuryRecipient::AllActiveMembers,
            amount_per_recipient: Decimal::new(50, 0),
            note: Some("Universal credit".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Get treasury transactions
    let transactions = app
        .client
        .get_treasury_transactions(&requests::GetTreasuryTransactions {
            community_id,
            limit: 10,
            offset: 0,
        })
        .await?;

    // Should have 1 transaction
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].note, Some("Universal credit".into()));

    Ok(())
}

#[tokio::test]
async fn test_transaction_pagination() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_points_allocation_mode(community_id).await?;

    // Create 5 transactions
    for i in 1..=5 {
        app.client
            .treasury_credit_operation(&requests::TreasuryCreditOperation {
                community_id,
                recipient: TreasuryRecipient::AllActiveMembers,
                amount_per_recipient: Decimal::new(10, 0),
                note: Some(format!("Transaction {}", i)),
                idempotency_key: IdempotencyKey(Uuid::new_v4()),
            })
            .await?;
    }

    // Get first 2 transactions
    let page1 = app
        .client
        .get_member_transactions(&requests::GetMemberTransactions {
            community_id,
            member_user_id: None,
            limit: 2,
            offset: 0,
        })
        .await?;
    assert_eq!(page1.len(), 2);

    // Get next 2 transactions
    let page2 = app
        .client
        .get_member_transactions(&requests::GetMemberTransactions {
            community_id,
            member_user_id: None,
            limit: 2,
            offset: 2,
        })
        .await?;
    assert_eq!(page2.len(), 2);

    // Get remaining transaction
    let page3 = app
        .client
        .get_member_transactions(&requests::GetMemberTransactions {
            community_id,
            member_user_id: None,
            limit: 2,
            offset: 4,
        })
        .await?;
    assert_eq!(page3.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_treasury_operation_prevents_negative_balance_distributed_clearing()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    // Default mode is distributed_clearing - don't change it

    // Attempt treasury credit operation without sufficient balance
    let result = app
        .client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: TreasuryRecipient::AllActiveMembers,
            amount_per_recipient: Decimal::new(100, 0),
            note: Some("Should fail".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await;

    // Should fail with insufficient balance
    assert!(result.is_err());
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("InsufficientBalance")
            || err_msg.contains("Insufficient balance")
    );

    Ok(())
}

#[tokio::test]
async fn test_treasury_operation_prevents_negative_balance_deferred_payment()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Set to deferred_payment mode
    sqlx::query(
        r#"
        UPDATE communities
        SET currency_mode = 'deferred_payment'
        WHERE id = $1
        "#,
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;

    // Get member for SingleMember operation (required for
    // deferred_payment)
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    // Attempt treasury credit operation without sufficient balance
    let result = app
        .client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: TreasuryRecipient::SingleMember(bob.user.user_id),
            amount_per_recipient: Decimal::new(100, 0),
            note: Some("Should fail".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await;

    // Should fail with insufficient balance
    assert!(result.is_err());
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("InsufficientBalance")
            || err_msg.contains("Insufficient balance")
    );

    Ok(())
}

#[tokio::test]
async fn test_auction_settlement_distributed_clearing() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    // Default mode is distributed_clearing - don't change it

    // Create site and two spaces
    let site = app.create_test_site(&community_id).await?;
    let space_a = app.create_test_space(&site.site_id).await?;
    let space_b_details = test_helpers::space_details_b(site.site_id);
    let space_b = app.client.create_space(&space_b_details).await?;

    // Run auction: Rounds 0-1 have bids, Round 2 has no bids → settlement
    // Space values will be 0 after round 0, 1 after round 1
    // Alternate bidders so they compete for each space
    let auction_id = run_simple_auction(
        &app,
        site.site_id,
        vec![
            (0, space_a.space_id, "alice"),
            (0, space_b, "bob"),
            (1, space_a.space_id, "bob"),
            (1, space_b, "alice"),
        ],
    )
    .await?;

    // Verify auction concluded
    let auction = app.client.get_auction(&auction_id).await?;
    assert!(auction.end_at.is_some());

    // Get final round for results
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let final_round = &rounds[rounds.len() - 1];
    let round_results = app
        .client
        .list_round_space_results_for_round(&final_round.round_id)
        .await?;

    let result_a = round_results
        .iter()
        .find(|r| r.space_id == space_a.space_id)
        .unwrap();
    let result_b = round_results
        .iter()
        .find(|r| r.space_id == space_b)
        .unwrap();

    // With bid_increment = 1.0, after two rounds with bids:
    // Space A value = 1.0 (won by Bob)
    // Space B value = 1.0 (won by Alice)
    assert_eq!(result_a.value, Decimal::new(1, 0));
    assert_eq!(result_b.value, Decimal::new(1, 0));

    // Get members for balance checks
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    // In distributed_clearing mode with 2 active members:
    // Total payments = 2.0 distributed equally (1.0 to each)
    // Alice: -1.0 (payment for space_b) + 1.0 (distribution) = 0
    // Bob: -1.0 (payment for space_a) + 1.0 (distribution) = 0
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

    // Verify transaction history shows settlement
    app.login_alice().await?;
    let alice_txns = app
        .client
        .get_member_transactions(&requests::GetMemberTransactions {
            community_id,
            member_user_id: None,
            limit: 10,
            offset: 0,
        })
        .await?;

    let settlement_txns: Vec<_> = alice_txns
        .iter()
        .filter(|t| t.entry_type == EntryType::AuctionSettlement)
        .collect();
    assert!(settlement_txns.len() > 0);

    Ok(())
}

#[tokio::test]
async fn test_auction_settlement_single_winner() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Create site and two spaces
    // Second space is a dummy to maintain eligibility
    let site = app.create_test_site(&community_id).await?;
    let space = app.create_test_space(&site.site_id).await?;
    let space_b_details = test_helpers::space_details_b(site.site_id);
    let space_b = app.client.create_space(&space_b_details).await?;

    // Run auction: Alice and Bob compete for space
    // Bob bids on space_b in round 0 just to maintain eligibility
    // Round 0: Alice bids on space, Bob bids on space_b (for eligibility)
    // Round 1: Bob bids on space, Alice bids on space_b (for eligibility)
    // Round 2: Alice bids on space, Bob has no bid (loses but already eligible)
    // Round 3: No bids, settlement triggers
    // space value = 2, space_b value = 1
    let _auction_id = run_simple_auction(
        &app,
        site.site_id,
        vec![
            (0, space.space_id, "alice"),
            (0, space_b, "bob"),
            (1, space.space_id, "bob"),
            (1, space_b, "alice"),
            (2, space.space_id, "alice"),
        ],
    )
    .await?;

    // Get members
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    // Alice won both spaces: space at value 2.0, space_b at value 1.0
    // Total Alice payment: 3.0
    // Distribution in distributed_clearing with 2 active: 1.5 each
    // Alice net: -3.0 (payment) + 1.5 (distribution) = -1.5
    // Bob net: +1.5 (distribution only)

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

    assert_eq!(alice_info.balance, Decimal::new(-15, 1)); // -1.5
    assert_eq!(bob_info.balance, Decimal::new(15, 1)); // 1.5

    Ok(())
}

#[tokio::test]
async fn test_auction_settlement_points_allocation() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_points_allocation_mode(community_id).await?;

    // Give Alice some initial balance via treasury operation
    app.client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: TreasuryRecipient::AllActiveMembers,
            amount_per_recipient: Decimal::new(100, 0),
            note: Some("Initial allocation".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Create auction and two spaces
    let site = app.create_test_site(&community_id).await?;
    let space = app.create_test_space(&site.site_id).await?;
    let space_b_details = test_helpers::space_details_b(site.site_id);
    let space_b = app.client.create_space(&space_b_details).await?;

    // Run auction: Alice and Bob compete, Alice wins both spaces
    // Round 0: Alice bids on space, Bob bids on space_b (for eligibility)
    // Round 1: Bob bids on space, Alice bids on space_b (for eligibility)
    // Round 2: Alice bids on space
    // Round 3: No bids, settlement triggers
    // space value = 2, space_b value = 1
    let _auction_id = run_simple_auction(
        &app,
        site.site_id,
        vec![
            (0, space.space_id, "alice"),
            (0, space_b, "bob"),
            (1, space.space_id, "bob"),
            (1, space_b, "alice"),
            (2, space.space_id, "alice"),
        ],
    )
    .await?;

    // In points_allocation mode, winners pay treasury (not members)
    // Alice won both spaces: total payment = 2 + 1 = 3
    // Alice: 100 (initial) - 3 (payment) = 97
    // Treasury: -200 (issued initial credits) + 3 (Alice payment) = -197

    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();

    let alice_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(alice.user.user_id),
        })
        .await?;

    assert_eq!(alice_info.balance, Decimal::new(97, 0));

    // Check treasury balance
    let treasury = app
        .client
        .get_treasury_account(&requests::GetTreasuryAccount { community_id })
        .await?;

    // Treasury issued 200 credits (100 to Alice, 100 to Bob)
    // Treasury received 3 credits from Alice's auction payments
    // Net: -200 + 3 = -197
    assert_eq!(treasury.balance_cached, Decimal::new(-197, 0));

    Ok(())
}

// Currency Configuration Management Tests

#[tokio::test]
async fn update_currency_config_coleader_permissions() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Get initial currency config
    let communities = app.client.get_communities().await?;
    let community = communities.first().unwrap();
    let initial_config = community.currency.clone();

    // Bob (member) tries to update - should fail
    app.login_bob().await?;
    let new_config = payloads::CurrencyModeConfig::DistributedClearing(
        payloads::IOUConfig {
            default_credit_limit: Some(Decimal::from(200)),
            debts_callable: true,
        },
    );
    let body = requests::UpdateCurrencyConfig {
        community_id,
        currency: payloads::CurrencySettings {
            mode_config: new_config.clone(),
            name: "credits".to_string(),
            symbol: "C".to_string(),
            minor_units: 2,
            balances_visible_to_members: false,
        },
    };
    let result = app.client.update_currency_config(&body).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);

    // Alice (leader) updates - should succeed
    app.login_alice().await?;
    app.client.update_currency_config(&body).await?;

    // Verify changes persisted
    let communities = app.client.get_communities().await?;
    let updated_community = communities.first().unwrap();
    assert_eq!(updated_community.currency.mode_config, new_config);
    assert_eq!(updated_community.currency.name, "credits");
    assert_eq!(updated_community.currency.symbol, "C");
    assert!(!updated_community.currency.balances_visible_to_members);

    // Verify mode stayed the same
    assert_eq!(
        updated_community.currency.mode_config.mode(),
        initial_config.mode_config.mode()
    );

    Ok(())
}

#[tokio::test]
async fn currency_mode_immutable() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;

    // Create community with DistributedClearing mode
    let body = requests::CreateCommunity {
        name: "Test community".to_string(),
        new_members_default_active: true,
        currency: payloads::CurrencySettings {
            mode_config: payloads::CurrencyModeConfig::DistributedClearing(
                payloads::IOUConfig {
                    default_credit_limit: Some(Decimal::from(100)),
                    debts_callable: true,
                },
            ),
            name: "dollars".to_string(),
            symbol: "$".to_string(),
            minor_units: 2,
            balances_visible_to_members: true,
        },
    };
    let community_id = app.client.create_community(&body).await?;

    // Try to change mode to DeferredPayment - should fail
    let new_config =
        payloads::CurrencyModeConfig::DeferredPayment(payloads::IOUConfig {
            default_credit_limit: Some(Decimal::from(100)),
            debts_callable: true,
        });
    let update_body = requests::UpdateCurrencyConfig {
        community_id,
        currency: payloads::CurrencySettings {
            mode_config: new_config,
            name: "dollars".to_string(),
            symbol: "$".to_string(),
            minor_units: 2,
            balances_visible_to_members: true,
        },
    };
    let result = app.client.update_currency_config(&update_body).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);

    // Verify mode unchanged
    let communities = app.client.get_communities().await?;
    let community = communities.first().unwrap();
    assert!(matches!(
        community.currency.mode_config,
        payloads::CurrencyModeConfig::DistributedClearing(_)
    ));

    Ok(())
}

#[tokio::test]
async fn currency_config_validation() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    // Test currency name too long (> 50 chars)
    let long_name = (0..51).map(|_| "X").collect::<String>();
    let body = requests::UpdateCurrencyConfig {
        community_id,
        currency: payloads::CurrencySettings {
            mode_config: test_helpers::default_currency_config(),
            name: long_name,
            symbol: "$".to_string(),
            minor_units: 2,
            balances_visible_to_members: true,
        },
    };
    let result = app.client.update_currency_config(&body).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);

    // Test currency symbol too long (> 5 chars)
    let body = requests::UpdateCurrencyConfig {
        community_id,
        currency: payloads::CurrencySettings {
            mode_config: test_helpers::default_currency_config(),
            name: "dollars".to_string(),
            symbol: "TOOLONG".to_string(),
            minor_units: 2,
            balances_visible_to_members: true,
        },
    };
    let result = app.client.update_currency_config(&body).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);

    // Test valid update succeeds
    let body = requests::UpdateCurrencyConfig {
        community_id,
        currency: payloads::CurrencySettings {
            mode_config: test_helpers::default_currency_config(),
            name: "points".to_string(),
            symbol: "P".to_string(),
            minor_units: 0,
            balances_visible_to_members: false,
        },
    };
    app.client.update_currency_config(&body).await?;

    // Verify changes
    let communities = app.client.get_communities().await?;
    let community = communities.first().unwrap();
    assert_eq!(community.currency.name, "points");
    assert_eq!(community.currency.symbol, "P");
    assert!(!community.currency.balances_visible_to_members);

    Ok(())
}

#[tokio::test]
async fn update_currency_config_fields() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;

    // Create community with specific IOU config
    let body = requests::CreateCommunity {
        name: "Test community".to_string(),
        new_members_default_active: true,
        currency: payloads::CurrencySettings {
            mode_config: payloads::CurrencyModeConfig::DistributedClearing(
                payloads::IOUConfig {
                    default_credit_limit: Some(Decimal::from(100)),
                    debts_callable: true,
                },
            ),
            name: "dollars".to_string(),
            symbol: "$".to_string(),
            minor_units: 2,
            balances_visible_to_members: true,
        },
    };
    let community_id = app.client.create_community(&body).await?;

    // Update all configurable fields
    let new_config = payloads::CurrencyModeConfig::DistributedClearing(
        payloads::IOUConfig {
            default_credit_limit: Some(Decimal::from(250)),
            debts_callable: false,
        },
    );
    let update_body = requests::UpdateCurrencyConfig {
        community_id,
        currency: payloads::CurrencySettings {
            mode_config: new_config.clone(),
            name: "credits".to_string(),
            symbol: "¢".to_string(),
            minor_units: 3,
            balances_visible_to_members: false,
        },
    };
    app.client.update_currency_config(&update_body).await?;

    // Verify all fields updated correctly
    let communities = app.client.get_communities().await?;
    let community = communities.first().unwrap();
    assert_eq!(community.currency.mode_config, new_config);
    assert_eq!(community.currency.name, "credits");
    assert_eq!(community.currency.symbol, "¢");
    assert!(!community.currency.balances_visible_to_members);

    // Verify specific config fields
    if let payloads::CurrencyModeConfig::DistributedClearing(cfg) =
        &community.currency.mode_config
    {
        assert_eq!(cfg.default_credit_limit, Some(Decimal::from(250)));
        assert!(!cfg.debts_callable);
    } else {
        panic!("Expected DistributedClearing config");
    }

    Ok(())
}

#[tokio::test]
async fn test_reset_all_balances_basic() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_points_allocation_mode(community_id).await?;

    // Get user IDs
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    // Give Alice and Bob some balances via treasury operations
    let alice_amount = Decimal::new(100, 0);
    let bob_amount = Decimal::new(200, 0);

    // Credit Alice
    app.client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: TreasuryRecipient::SingleMember(alice.user.user_id),
            amount_per_recipient: alice_amount,
            note: Some("Test credit for Alice".to_string()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Credit Bob
    app.client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: TreasuryRecipient::SingleMember(bob.user.user_id),
            amount_per_recipient: bob_amount,
            note: Some("Test credit for Bob".to_string()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Verify initial balances
    let alice_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(alice.user.user_id),
        })
        .await?;
    assert_eq!(alice_info.balance, alice_amount);

    app.login_bob().await?;
    let bob_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: None,
        })
        .await?;
    assert_eq!(bob_info.balance, bob_amount);

    // Reset all balances (alice is leader)
    app.login_alice().await?;
    let result = app
        .client
        .reset_all_balances(&requests::ResetAllBalances {
            community_id,
            note: Some("Test reset".to_string()),
        })
        .await?;

    assert_eq!(result.accounts_reset, 2);
    assert_eq!(result.total_transferred, alice_amount + bob_amount);

    // Verify all balances are now zero
    let alice_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(alice.user.user_id),
        })
        .await?;
    assert_eq!(alice_info.balance, Decimal::ZERO);

    app.login_bob().await?;
    let bob_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: None,
        })
        .await?;
    assert_eq!(bob_info.balance, Decimal::ZERO);

    // Verify treasury received the total (negative = debit, positive = credit)
    // In points_allocation mode, treasury starts negative and receives
    // positive debits from reset
    app.login_alice().await?;
    let treasury = app
        .client
        .get_treasury_account(&requests::GetTreasuryAccount { community_id })
        .await?;
    // Treasury started at -(alice_amount + bob_amount) and received
    // +(alice_amount + bob_amount) from reset, so should be 0
    assert_eq!(treasury.balance_cached, Decimal::ZERO);

    Ok(())
}

#[tokio::test]
async fn test_reset_all_balances_blocked_during_auction() -> anyhow::Result<()>
{
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Create an auction
    let auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    let _auction_id = app.client.create_auction(&auction_details).await?;

    // Try to reset balances - should fail
    let result = app
        .client
        .reset_all_balances(&requests::ResetAllBalances {
            community_id,
            note: Some("Should fail".to_string()),
        })
        .await;

    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn test_reset_all_balances_member_permission_denied() -> anyhow::Result<()>
{
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Bob is just a member
    app.login_bob().await?;

    let result = app
        .client
        .reset_all_balances(&requests::ResetAllBalances {
            community_id,
            note: Some("Should fail".to_string()),
        })
        .await;

    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn test_locked_balance_during_auction() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_points_allocation_mode(community_id).await?;

    // Give Alice and Bob initial balances
    app.client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: TreasuryRecipient::AllActiveMembers,
            amount_per_recipient: Decimal::new(100, 0),
            note: Some("Initial credit".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Create site and two spaces
    let site = app.create_test_site(&community_id).await?;
    let space_a = app.create_test_space(&site.site_id).await?;
    let space_b_details = test_helpers::space_details_b(site.site_id);
    let space_b = app.client.create_space(&space_b_details).await?;

    // Create auction starting now
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Create initial round (round 0)
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_0 = &rounds[0];

    // Get member user IDs
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    let _bob = members.iter().find(|m| m.user.username == "bob").unwrap();

    // Round 0: Alice bids on space_a, Bob bids on space_b
    app.login_alice().await?;
    app.client
        .create_bid(&space_a.space_id, &round_0.round_id)
        .await?;

    app.login_bob().await?;
    app.client.create_bid(&space_b, &round_0.round_id).await?;

    // Check locked balances during round 0 (first round, no previous prices)
    // Bid amount for round 0 is 0 (no previous price)
    app.login_alice().await?;
    let alice_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(alice.user.user_id),
        })
        .await?;
    assert_eq!(alice_info.locked_balance, Decimal::ZERO);

    app.login_bob().await?;
    let bob_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: None,
        })
        .await?;
    assert_eq!(bob_info.locked_balance, Decimal::ZERO);

    // Advance time to end of round 0 and process
    app.time_source
        .set(round_0.round_details.end_at + Span::new().seconds(1));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

    // Get round 1
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_1 = &rounds[1];

    // Round 1: Bob bids on space_a, Alice bids on space_b
    app.login_bob().await?;
    app.client
        .create_bid(&space_a.space_id, &round_1.round_id)
        .await?;

    app.login_alice().await?;
    app.client.create_bid(&space_b, &round_1.round_id).await?;

    // Check locked balances during round 1
    // After round 0, space prices are 0
    // So bid amount for round 1 = 0 + bid_increment (1.0) = 1.0
    app.login_alice().await?;
    let alice_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(alice.user.user_id),
        })
        .await?;
    // Alice has a bid on space_b at price 1.0
    assert_eq!(alice_info.locked_balance, Decimal::new(1, 0));
    // Available credit = balance - locked + limit = 100 - 1 + 0 = 99
    assert_eq!(alice_info.available_credit, Some(Decimal::new(99, 0)));

    app.login_bob().await?;
    let bob_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: None,
        })
        .await?;
    // Bob has a bid on space_a at price 1.0
    assert_eq!(bob_info.locked_balance, Decimal::new(1, 0));
    assert_eq!(bob_info.available_credit, Some(Decimal::new(99, 0)));

    // Advance time to end of round 1 and process
    app.time_source
        .set(round_1.round_details.end_at + Span::new().seconds(1));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

    // Get round 2
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_2 = &rounds[2];

    // Round 2: Alice bids on space_a (outbidding Bob)
    // Alice is already winning space_b from round 1
    app.login_alice().await?;
    app.client
        .create_bid(&space_a.space_id, &round_2.round_id)
        .await?;

    // Check Alice's locked balance
    // After round 1, space_a price = 1.0, space_b price = 1.0
    // Alice has:
    // - Standing high bid on space_b from round 1: 1.0 (locked)
    // - New bid on space_a in round 2: 1.0 + 1.0 = 2.0 (locked)
    // Total locked: 1.0 + 2.0 = 3.0
    let alice_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(alice.user.user_id),
        })
        .await?;
    assert_eq!(alice_info.locked_balance, Decimal::new(3, 0));
    // Available = 100 - 3 + 0 = 97
    assert_eq!(alice_info.available_credit, Some(Decimal::new(97, 0)));

    // Bob has no bids in round 2, but was the high bidder on space_a
    // after round 1
    app.login_bob().await?;
    let bob_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: None,
        })
        .await?;
    // Bob's winning bid from round 1 on space_a is still locked (price 1.0)
    // even though Alice outbid him in round 2 (round 2 not yet processed)
    assert_eq!(bob_info.locked_balance, Decimal::new(1, 0));

    // Advance to end of round 2 and process
    app.time_source
        .set(round_2.round_details.end_at + Span::new().seconds(1));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

    // Get round 3
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_3 = &rounds[3];

    // Round 3: No bids (will trigger settlement)
    // Advance to end of round 3 and process
    app.time_source
        .set(round_3.round_details.end_at + Span::new().seconds(1));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

    // Verify auction concluded
    let auction = app.client.get_auction(&auction_id).await?;
    assert!(auction.end_at.is_some());

    // After settlement, locked balances should be zero
    app.login_alice().await?;
    let alice_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: Some(alice.user.user_id),
        })
        .await?;
    assert_eq!(alice_info.locked_balance, Decimal::ZERO);

    app.login_bob().await?;
    let bob_info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: None,
        })
        .await?;
    assert_eq!(bob_info.locked_balance, Decimal::ZERO);

    Ok(())
}
