use reqwest::StatusCode;

use payloads::requests;

use crate::helpers::spawn_app;

#[tokio::test]
async fn create_community() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_test_account().await;
    app.create_test_community().await;
    Ok(())
}

#[tokio::test]
async fn long_community_name_rejected() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_test_account().await;

    let body = requests::CreateCommunity {
        name: (0..300).map(|_| "X").collect::<String>(),
    };
    let response = app.post("create_community", &body).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    Ok(())
}
