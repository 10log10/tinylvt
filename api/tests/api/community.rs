use reqwest::StatusCode;

use payloads::requests;

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
        new_members_default_active: true,
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
        match member.username.as_str() {
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
