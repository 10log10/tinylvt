use payloads::{IdempotencyKey, requests};
use reqwest::StatusCode;
use rust_decimal::Decimal;
use uuid::Uuid;

use test_helpers::{assert_status_code, spawn_app};

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

    assert_status_code(result, StatusCode::BAD_REQUEST);

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
    assert_status_code(result, StatusCode::BAD_REQUEST);

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
            to_user_id: bob_id,
            amount: Decimal::new(5000, 2), // 50.00
            note: Some("Test transfer".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
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
