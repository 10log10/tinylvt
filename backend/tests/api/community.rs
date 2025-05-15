use reqwest::StatusCode;

use payloads::requests;

use crate::helpers::{assert_status_code, spawn_app};

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
