use payloads::{AccountOwner, ApiError, requests};
use rust_decimal::Decimal;

use test_helpers::{assert_api_error, spawn_app};

#[tokio::test]
async fn create_community() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    app.create_test_community().await?;
    Ok(())
}

#[tokio::test]
async fn long_community_name_rejected() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;

    let body = requests::CreateCommunity {
        name: (0..300).map(|_| "X").collect::<String>(),
        description: None,
        currency: payloads::CurrencySettings {
            mode_config: test_helpers::default_currency_config(),
            name: "dollars".to_string(),
            symbol: "$".to_string(),
            minor_units: 2,
            balances_visible_to_members: true,
            new_members_default_active: true,
        },
    };
    let result = app.client.create_community(&body).await;

    assert_api_error(result, ApiError::FieldTooLong);

    Ok(())
}

#[tokio::test]
async fn community_invite_flow() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_two_person_community().await?;

    // check that the listed members are correct
    let communities = app.client.get_communities().await?;
    let community_id = communities.first().unwrap().id;
    let members = app.client.get_members(&community_id).await?;
    assert_eq!(members.len(), 2);
    Ok(())
}

#[tokio::test]
async fn membership_schedule_set_read_update() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.create_schedule(&community_id).await?;

    api::store::update_is_active_from_schedule(&app.db_pool, &app.time_source)
        .await?;
    let members = app.client.get_members(&community_id).await?;

    for member in &members {
        match member.user.username.as_str() {
            "alice" => assert!(member.is_active),
            "bob" => assert!(!member.is_active),
            _ => (),
        };
    }

    app.time_source.advance(jiff::Span::new().hours(2));
    api::store::update_is_active_from_schedule(&app.db_pool, &app.time_source)
        .await?;
    let members = app.client.get_members(&community_id).await?;

    // all members now inactive
    for member in &members {
        assert!(!member.is_active);
    }

    Ok(())
}

#[tokio::test]
async fn community_role_information_returned() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // Create Alice and her community (Alice will be the leader)
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    // Create Bob and invite him to Alice's community
    app.create_bob_user().await?;
    app.login_alice().await?;
    let _invite_id = app.invite_bob().await?;

    // Bob accepts the invite (Bob will be a member)
    app.login_bob().await?;
    app.accept_invite().await?;

    // Test Alice's perspective (should be leader)
    app.login_alice().await?;
    let alice_communities = app.client.get_communities().await?;
    assert_eq!(alice_communities.len(), 1);
    let alice_community = &alice_communities[0];
    assert_eq!(alice_community.id, community_id);
    assert_eq!(alice_community.name, "Test community");
    assert_eq!(alice_community.user_role, payloads::Role::Leader);
    assert!(alice_community.user_is_active);

    // Test Bob's perspective (should be member)
    app.login_bob().await?;
    let bob_communities = app.client.get_communities().await?;
    assert_eq!(bob_communities.len(), 1);
    let bob_community = &bob_communities[0];
    assert_eq!(bob_community.id, community_id);
    assert_eq!(bob_community.name, "Test community");
    assert_eq!(bob_community.user_role, payloads::Role::Member);
    assert!(bob_community.user_is_active);

    Ok(())
}

#[tokio::test]
async fn delete_community_leader_only() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Bob (member) tries to delete - should fail
    app.login_bob().await?;
    let result = app.client.delete_community(&community_id).await;
    assert_api_error(result, ApiError::RequiresLeaderPermissions);

    // Verify community still exists
    app.login_alice().await?;
    let communities = app.client.get_communities().await?;
    assert_eq!(communities.len(), 1);

    // Alice (leader) deletes - should succeed
    app.client.delete_community(&community_id).await?;

    // Verify community is gone
    let communities = app.client.get_communities().await?;
    assert!(communities.is_empty());

    Ok(())
}

/// Community deletion should succeed even when there's financial history.
/// This tests that delete_community properly clears journal_entries first
/// to unblock the cascade.
#[tokio::test]
async fn delete_community_with_financial_history() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Get Bob's user_id
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    let bob_id = bob.user.user_id;

    // Alice transfers funds to Bob (creates journal entries)
    app.login_alice().await?;
    app.client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to: AccountOwner::Member(bob_id),
            amount: Decimal::new(5000, 2), // 50.00
            note: Some("Test transfer".into()),
            idempotency_key: requests::ClientIdempotencyKey::new(),
        })
        .await?;

    // Verify journal entries exist
    let entry_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM journal_entries WHERE community_id = $1",
    )
    .bind(community_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert!(entry_count > 0, "Should have journal entries");

    // Alice deletes community - should succeed despite financial history
    app.client.delete_community(&community_id).await?;

    // Verify community is gone
    let communities = app.client.get_communities().await?;
    assert!(communities.is_empty());

    // Verify journal entries are also gone
    let entry_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM journal_entries WHERE community_id = $1",
    )
    .bind(community_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(entry_count, 0, "Journal entries should be deleted");

    Ok(())
}

#[tokio::test]
async fn email_multi_use_invite_rejected() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    let result = app
        .client
        .invite_member(&requests::InviteCommunityMember {
            community_id,
            new_member_email: Some(test_helpers::bob_credentials().email),
            single_use: false,
        })
        .await;
    assert_api_error(result, ApiError::EmailInviteMustBeSingleUse);

    Ok(())
}

#[tokio::test]
async fn acceptance_closes_invite_and_records_provenance() -> anyhow::Result<()>
{
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    let invite_id = app.invite_bob().await?;
    app.create_bob_user().await?;
    app.login_bob().await?;
    app.accept_invite().await?;

    // Provenance is recorded on the membership row.
    let member_invite: Option<payloads::InviteId> = sqlx::query_scalar(
        "SELECT cm.invite_id FROM community_members cm
        JOIN users u ON u.id = cm.user_id
        WHERE cm.community_id = $1 AND u.username = 'bob'",
    )
    .bind(community_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(member_invite, Some(invite_id));

    // The consumed invite disappears from the recipient's received list.
    let received = app.client.get_received_invites().await?;
    assert!(received.is_empty());

    // The invite is closed but kept as a record in the issued list.
    app.login_alice().await?;
    let issued = app.client.get_issued_invites(&community_id).await?;
    assert_eq!(issued.len(), 1);
    assert_eq!(issued[0].id, invite_id);
    assert!(issued[0].deleted_at.is_some());

    // A closed invite rejects acceptance, and its link no longer resolves
    // to a community for the accept page.
    app.create_charlie_user().await?;
    app.login_charlie().await?;
    let result = app.client.accept_invite(&invite_id).await;
    assert_api_error(result, ApiError::CommunityInviteClosed);
    let result = app.client.get_invite_community_name(&invite_id).await;
    assert_api_error(result, ApiError::CommunityInviteClosed);

    // Moderator+ sees the invite email beside the member; the member who
    // joined without an invite (the leader) has none.
    app.login_alice().await?;
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    assert_eq!(
        bob.invite_email,
        Some(test_helpers::bob_credentials().email)
    );
    let alice = members.iter().find(|m| m.user.username == "alice").unwrap();
    assert_eq!(alice.invite_email, None);

    // Plain members don't see provenance.
    app.login_bob().await?;
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    assert_eq!(bob.invite_email, None);

    Ok(())
}

#[tokio::test]
async fn multi_use_invite_lifecycle() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    let invite_id = app
        .client
        .invite_member(&requests::InviteCommunityMember {
            community_id,
            new_member_email: None,
            single_use: false,
        })
        .await?;

    app.create_bob_user().await?;
    app.login_bob().await?;
    app.client.accept_invite(&invite_id).await?;
    app.create_charlie_user().await?;
    app.login_charlie().await?;
    app.client.accept_invite(&invite_id).await?;

    // Stays open across acceptances.
    app.login_alice().await?;
    let issued = app.client.get_issued_invites(&community_id).await?;
    assert_eq!(issued.len(), 1);
    assert!(issued[0].deleted_at.is_none());

    // Revoking a referenced invite closes it as a read-only record, keeping
    // the members and their provenance.
    app.client
        .delete_invite(&requests::DeleteInvite {
            community_id,
            invite_id,
        })
        .await?;
    let issued = app.client.get_issued_invites(&community_id).await?;
    assert_eq!(issued.len(), 1);
    assert!(issued[0].deleted_at.is_some());
    let members = app.client.get_members(&community_id).await?;
    assert_eq!(members.len(), 3);
    let referencing: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM community_members WHERE invite_id = $1",
    )
    .bind(invite_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(referencing, 2);

    // A closed invite is not revocable again.
    let result = app
        .client
        .delete_invite(&requests::DeleteInvite {
            community_id,
            invite_id,
        })
        .await;
    assert_api_error(result, ApiError::CommunityInviteNotFound);

    Ok(())
}

#[tokio::test]
async fn revoking_unreferenced_invite_removes_it() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    let invite_id = app.create_link_invite().await?;

    app.client
        .delete_invite(&requests::DeleteInvite {
            community_id,
            invite_id,
        })
        .await?;
    let issued = app.client.get_issued_invites(&community_id).await?;
    assert!(issued.is_empty());

    Ok(())
}

#[tokio::test]
async fn profile_link_set_and_moderation() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    app.login_bob().await?;
    app.client
        .set_profile_link(&requests::SetProfileLink {
            community_id,
            profile_link: Some("https://example.com/bob".into()),
        })
        .await?;

    // Visible to everyone in the member list.
    app.login_alice().await?;
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    assert_eq!(bob.profile_link.as_deref(), Some("https://example.com/bob"));
    let alice_id = members
        .iter()
        .find(|m| m.user.username == "alice")
        .unwrap()
        .user
        .user_id;
    let bob_id = bob.user.user_id;

    // Over-long links are rejected.
    app.login_bob().await?;
    let result = app
        .client
        .set_profile_link(&requests::SetProfileLink {
            community_id,
            profile_link: Some("x".repeat(300)),
        })
        .await;
    assert_api_error(result, ApiError::FieldTooLong);

    // Clearing one's own link with None.
    app.client
        .set_profile_link(&requests::SetProfileLink {
            community_id,
            profile_link: None,
        })
        .await?;
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    assert_eq!(bob.profile_link, None);

    // Members cannot clear someone else's link.
    let result = app
        .client
        .clear_profile_link(&requests::ClearProfileLink {
            community_id,
            user_id: alice_id,
        })
        .await;
    assert_api_error(result, ApiError::RequiresModeratorPermissions);

    // Moderator+ can clear another member's link.
    app.client
        .set_profile_link(&requests::SetProfileLink {
            community_id,
            profile_link: Some("https://example.com/bob".into()),
        })
        .await?;
    app.login_alice().await?;
    app.client
        .clear_profile_link(&requests::ClearProfileLink {
            community_id,
            user_id: bob_id,
        })
        .await?;
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    assert_eq!(bob.profile_link, None);

    Ok(())
}
