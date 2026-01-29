use payloads::requests;
use payloads::{IdempotencyKey, TreasuryRecipient};
use reqwest::StatusCode;
use rust_decimal::Decimal;
use test_helpers::{assert_status_code, spawn_app};
use uuid::Uuid;

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
        .update_credit_limit(&requests::UpdateCreditLimit {
            community_id,
            member_user_id: bob.user.user_id,
            credit_limit: Some(new_limit),
        })
        .await?;

    assert_eq!(account.credit_limit, Some(new_limit));

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
        .update_credit_limit(&requests::UpdateCreditLimit {
            community_id,
            member_user_id: alice.user.user_id,
            credit_limit: Some(Decimal::new(500, 0)),
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
        .update_credit_limit(&requests::UpdateCreditLimit {
            community_id,
            member_user_id: alice.user.user_id,
            credit_limit: Some(Decimal::ZERO),
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
    assert_eq!(account.credit_limit, None);

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
