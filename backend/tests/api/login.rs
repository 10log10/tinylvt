use reqwest::StatusCode;

use crate::helpers::{assert_is_redirect_to, spawn_app};

#[tokio::test]
async fn login_refused() -> anyhow::Result<()> {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": "random-username",
        "password": "random-password"
    });
    let response = app.post_login(&login_body).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn create_account() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let new_user_body = serde_json::json!({
        "username": "random-username",
        "password": "a-password",
        "email": "anemail@example.com",
    });
    let response = app
        .api_client
        .post(format!("{}/create_account", &app.address))
        .form(&new_user_body)
        .send()
        .await?;

    assert_is_redirect_to(&response, "/login");
    Ok(())
}
