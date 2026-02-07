use payloads::{IdempotencyKey, requests};
use reqwest::StatusCode;
use rust_decimal::Decimal;
use test_helpers::{assert_status_code, spawn_app};
use uuid::Uuid;

// ============================================================================
// Permission Tests
// ============================================================================

#[tokio::test]
async fn remove_member_requires_moderator() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Get Alice's user_id
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();

    // Bob (member) tries to remove Alice - should fail
    app.login_bob().await?;
    let request = requests::RemoveMember {
        community_id,
        member_user_id: alice.user.user_id,
    };
    let result = app.client.remove_member(&request).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn cannot_remove_self() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Get Alice's user_id
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();

    // Alice tries to remove herself - should fail
    app.login_alice().await?;
    let request = requests::RemoveMember {
        community_id,
        member_user_id: alice.user.user_id,
    };
    let result = app.client.remove_member(&request).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn leader_can_remove_member() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Get Bob's user_id
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    let bob_id = bob.user.user_id;

    // Alice (leader) removes Bob (member) - should succeed
    app.login_alice().await?;
    let request = requests::RemoveMember {
        community_id,
        member_user_id: bob_id,
    };
    app.client.remove_member(&request).await?;

    // Verify Bob is no longer a member
    let members_after = app.client.get_members(&community_id).await?;
    assert_eq!(members_after.len(), 1);
    assert!(members_after.iter().all(|m| m.user.user_id != bob_id));

    Ok(())
}

// ============================================================================
// Leave Community Tests
// ============================================================================

#[tokio::test]
async fn leave_community_basic() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Get Alice's user_id for verification
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    let alice_id = alice.user.user_id;

    // Bob leaves the community
    app.login_bob().await?;
    let request = requests::LeaveCommunity { community_id };
    app.client.leave_community(&request).await?;

    // Verify Bob is no longer a member
    app.login_alice().await?;
    let members_after = app.client.get_members(&community_id).await?;
    assert_eq!(members_after.len(), 1);
    assert_eq!(members_after[0].user.user_id, alice_id);

    // Verify Bob's communities list is empty
    app.login_bob().await?;
    let communities = app.client.get_communities().await?;
    assert!(communities.is_empty());

    Ok(())
}

#[tokio::test]
async fn leader_cannot_leave() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Alice (leader) tries to leave - should fail
    app.login_alice().await?;
    let request = requests::LeaveCommunity { community_id };
    let result = app.client.leave_community(&request).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

// ============================================================================
// Orphaned Accounts Tests
// ============================================================================

#[tokio::test]
async fn get_orphaned_accounts() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Get Bob's user_id
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    let bob_id = bob.user.user_id;

    // Give Bob some balance via transfer from Alice
    app.login_alice().await?;
    app.client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to_user_id: bob_id,
            amount: Decimal::new(1000, 2), // 10.00
            note: None,
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Bob leaves
    app.login_bob().await?;
    app.client
        .leave_community(&requests::LeaveCommunity { community_id })
        .await?;

    // Alice queries orphaned accounts
    app.login_alice().await?;
    let orphaned = app.client.get_orphaned_accounts(&community_id).await?;
    assert_eq!(orphaned.orphaned_accounts.len(), 1);
    let orphaned_account = &orphaned.orphaned_accounts[0];
    assert_eq!(
        orphaned_account.previous_owner.as_ref().unwrap().user_id,
        bob_id
    );
    assert_eq!(
        orphaned_account.account.balance_cached,
        Decimal::new(1000, 2)
    );

    Ok(())
}

#[tokio::test]
async fn get_orphaned_accounts_requires_coleader() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Bob (member, not coleader) tries to get orphaned accounts - should fail
    app.login_bob().await?;
    let result = app.client.get_orphaned_accounts(&community_id).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

/// In points_allocation mode, orphaned balance is transferred to treasury
#[tokio::test]
async fn resolve_to_treasury() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Switch to points_allocation mode (non-distributed_clearing goes to
    // treasury)
    app.set_points_allocation_mode(community_id).await?;

    // Get Bob's user_id
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    let bob_id = bob.user.user_id;

    // Give Bob some balance via treasury grant
    app.login_alice().await?;
    app.client
        .treasury_credit_operation(&requests::TreasuryCreditOperation {
            community_id,
            recipient: payloads::TreasuryRecipient::SingleMember(bob_id),
            amount_per_recipient: Decimal::new(5000, 2), // 50.00
            note: Some("Test grant".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Bob leaves
    app.login_bob().await?;
    app.client
        .leave_community(&requests::LeaveCommunity { community_id })
        .await?;

    // Alice gets orphaned accounts
    app.login_alice().await?;
    let orphaned = app.client.get_orphaned_accounts(&community_id).await?;
    let orphaned_account = &orphaned.orphaned_accounts[0];
    assert_eq!(
        orphaned_account.account.balance_cached,
        Decimal::new(5000, 2)
    );

    // Check treasury balance before resolution (should be -50 from grant)
    let treasury_before = app
        .client
        .get_treasury_account(&requests::GetTreasuryAccount { community_id })
        .await?;
    assert_eq!(treasury_before.balance_cached, Decimal::new(-5000, 2));

    // In points_allocation mode, resolution transfers to treasury
    app.client
        .resolve_orphaned_balance(&requests::ResolveOrphanedBalance {
            community_id,
            orphaned_account_id: orphaned_account.account.id,
            note: None,
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Verify orphaned account no longer appears (zero balance filtered out)
    let orphaned_after =
        app.client.get_orphaned_accounts(&community_id).await?;
    assert!(
        orphaned_after.orphaned_accounts.is_empty(),
        "Resolved orphaned account should not appear in list"
    );

    // Verify treasury received the funds (points_allocation mode)
    let treasury_after = app
        .client
        .get_treasury_account(&requests::GetTreasuryAccount { community_id })
        .await?;
    // Treasury was -50, received Bob's 50 back = 0
    assert_eq!(treasury_after.balance_cached, Decimal::ZERO);

    // Verify Alice did NOT receive the funds (unlike distributed_clearing)
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    // Alice should still be at 0 (she didn't receive Bob's balance)
    assert_eq!(alice.balance.unwrap(), Decimal::ZERO);

    Ok(())
}

/// In distributed_clearing mode, orphaned balance is distributed to active
/// members
#[tokio::test]
async fn resolve_distributed_clearing_distributes_to_members()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_three_person_community().await?;

    // Get user IDs
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    let charlie = members
        .iter()
        .find(|m| m.user.username == "charlie")
        .unwrap();

    let alice_id = alice.user.user_id;
    let bob_id = bob.user.user_id;
    let charlie_id = charlie.user.user_id;

    // Give Bob some balance
    app.login_alice().await?;
    app.client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to_user_id: bob_id,
            amount: Decimal::new(10000, 2), // 100.00
            note: Some("Test transfer".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Get initial balances of remaining members
    let members_before = app.client.get_members(&community_id).await?;
    let alice_balance_before = members_before
        .iter()
        .find(|m| m.user.user_id == alice_id)
        .unwrap()
        .balance
        .unwrap();
    let charlie_balance_before = members_before
        .iter()
        .find(|m| m.user.user_id == charlie_id)
        .unwrap()
        .balance
        .unwrap();

    // Bob leaves
    app.login_bob().await?;
    app.client
        .leave_community(&requests::LeaveCommunity { community_id })
        .await?;

    // Resolve - in distributed_clearing, automatically distributes to members
    app.login_alice().await?;
    let orphaned = app.client.get_orphaned_accounts(&community_id).await?;
    let orphaned_account = &orphaned.orphaned_accounts[0];
    app.client
        .resolve_orphaned_balance(&requests::ResolveOrphanedBalance {
            community_id,
            orphaned_account_id: orphaned_account.account.id,
            note: None,
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Verify balances increased (100 split between Alice and Charlie = 50 each)
    let members_after = app.client.get_members(&community_id).await?;
    let alice_balance_after = members_after
        .iter()
        .find(|m| m.user.user_id == alice_id)
        .unwrap()
        .balance
        .unwrap();
    let charlie_balance_after = members_after
        .iter()
        .find(|m| m.user.user_id == charlie_id)
        .unwrap()
        .balance
        .unwrap();

    assert_eq!(
        alice_balance_after,
        alice_balance_before + Decimal::new(5000, 2)
    );
    assert_eq!(
        charlie_balance_after,
        charlie_balance_before + Decimal::new(5000, 2)
    );

    Ok(())
}

#[tokio::test]
async fn resolve_with_negative_balance() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Get user IDs
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    let alice_id = alice.user.user_id;

    // Give Bob a negative balance by transferring from him to Alice
    app.login_bob().await?;
    app.client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to_user_id: alice_id,
            amount: Decimal::new(3000, 2), // 30.00
            note: Some("Test transfer".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    let bob_balance = bob.balance.unwrap();
    assert_eq!(bob_balance, Decimal::ZERO); // Bob started at zero

    let members_after_transfer = app.client.get_members(&community_id).await?;
    let bob_after_transfer = members_after_transfer
        .iter()
        .find(|m| m.user.username == "bob")
        .unwrap();
    assert_eq!(bob_after_transfer.balance.unwrap(), Decimal::new(-3000, 2));

    // Bob leaves with negative balance
    app.client
        .leave_community(&requests::LeaveCommunity { community_id })
        .await?;

    // Resolve to treasury
    app.login_alice().await?;
    let orphaned = app.client.get_orphaned_accounts(&community_id).await?;
    let orphaned_account = &orphaned.orphaned_accounts[0];
    app.client
        .resolve_orphaned_balance(&requests::ResolveOrphanedBalance {
            community_id,
            orphaned_account_id: orphaned_account.account.id,
            note: None,
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Verify orphaned account no longer appears (zero balance filtered out)
    let orphaned_after =
        app.client.get_orphaned_accounts(&community_id).await?;
    assert!(
        orphaned_after.orphaned_accounts.is_empty(),
        "Resolved orphaned account should not appear in list"
    );

    // In distributed_clearing mode, the debt is distributed to active members
    // (Alice is the only remaining active member)
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    // Alice received 30 from Bob (so Alice has +30, Bob had -30)
    // Then Bob's -30 debt is distributed to Alice = 30 - 30 = 0
    assert_eq!(alice.balance.unwrap(), Decimal::ZERO);

    Ok(())
}

#[tokio::test]
async fn zero_balance_orphaned_account_not_returned() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Bob leaves with zero balance
    app.login_bob().await?;
    app.client
        .leave_community(&requests::LeaveCommunity { community_id })
        .await?;

    // Zero-balance orphaned accounts should not appear in the list
    app.login_alice().await?;
    let orphaned = app.client.get_orphaned_accounts(&community_id).await?;
    assert!(
        orphaned.orphaned_accounts.is_empty(),
        "Zero-balance orphaned accounts should be filtered out"
    );

    Ok(())
}

#[tokio::test]
async fn idempotency_orphaned_resolution() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Get Bob's user_id
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    let bob_id = bob.user.user_id;

    // Give Bob balance and have him leave
    app.login_alice().await?;
    app.client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to_user_id: bob_id,
            amount: Decimal::new(10000, 2), // 100.00
            note: Some("Test transfer".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    app.login_bob().await?;
    app.client
        .leave_community(&requests::LeaveCommunity { community_id })
        .await?;

    // Resolve with idempotency key
    app.login_alice().await?;
    let orphaned = app.client.get_orphaned_accounts(&community_id).await?;
    let orphaned_account = &orphaned.orphaned_accounts[0];
    let idempotency_key = IdempotencyKey(Uuid::new_v4());

    app.client
        .resolve_orphaned_balance(&requests::ResolveOrphanedBalance {
            community_id,
            orphaned_account_id: orphaned_account.account.id,
            note: None,
            idempotency_key,
        })
        .await?;

    // Try again with same key - should succeed without duplicating
    app.client
        .resolve_orphaned_balance(&requests::ResolveOrphanedBalance {
            community_id,
            orphaned_account_id: orphaned_account.account.id,
            note: None,
            idempotency_key,
        })
        .await?;

    // In distributed_clearing mode, the balance is distributed to active
    // members (Alice). Verify Alice's balance is correct (not doubled).
    let members = app.client.get_members(&community_id).await?;
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    // Alice started at 0, transferred 100 to Bob (-100), then received Bob's
    // 100 back = 0
    assert_eq!(alice.balance.unwrap(), Decimal::ZERO);

    Ok(())
}

// ============================================================================
// Rejoin Flow Tests
// ============================================================================

#[tokio::test]
async fn rejoin_after_leaving() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Get Bob's user_id
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    let bob_id = bob.user.user_id;

    // Give Bob some balance
    app.login_alice().await?;
    app.client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to_user_id: bob_id,
            amount: Decimal::new(7500, 2), // 75.00
            note: Some("Test transfer".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Bob leaves
    app.login_bob().await?;
    app.client
        .leave_community(&requests::LeaveCommunity { community_id })
        .await?;

    // Verify Bob's account is orphaned with balance preserved
    app.login_alice().await?;
    let orphaned = app.client.get_orphaned_accounts(&community_id).await?;
    assert_eq!(orphaned.orphaned_accounts.len(), 1);
    assert_eq!(
        orphaned.orphaned_accounts[0].account.balance_cached,
        Decimal::new(7500, 2)
    );

    // Alice invites Bob again
    app.invite_bob().await?;

    // Bob accepts and rejoins
    app.login_bob().await?;
    app.accept_invite().await?;

    // Verify Bob is a member again
    let members_after = app.client.get_members(&community_id).await?;
    assert_eq!(members_after.len(), 2);
    let bob_member = members_after
        .iter()
        .find(|m| m.user.user_id == bob_id)
        .unwrap();

    // Verify Bob's balance was preserved
    assert_eq!(bob_member.balance.unwrap(), Decimal::new(7500, 2));

    // Verify orphaned accounts list is now empty
    app.login_alice().await?;
    let orphaned_after =
        app.client.get_orphaned_accounts(&community_id).await?;
    assert!(orphaned_after.orphaned_accounts.is_empty());

    Ok(())
}
