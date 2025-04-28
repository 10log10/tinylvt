use payloads::requests;
use reqwest::StatusCode;

use crate::helpers::spawn_app;

#[tokio::test]
async fn login_refused() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // test a login with an invalid user
    let login_body = serde_json::json!({
        "username": "random-username",
        "password": "random-password"
    });
    let response = app.post("login", &login_body).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.text().await?, "Authentication failed");

    // login check should fail
    let response = app.post_login_check().await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.text().await?, "");

    Ok(())
}

#[tokio::test]
async fn create_account() -> anyhow::Result<()> {
    let app = spawn_app().await;

    app.create_test_account().await;

    // check for valid session
    let response = app.post_login_check().await;

    assert_eq!(response.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn long_username_email_rejected() -> anyhow::Result<()> {
    let app = spawn_app().await;

    let mut body = requests::CreateAccount {
        username: (0..52).map(|_| "X").collect::<String>(),
        email: "anemail@example.com".into(),
        password: "a-password".into(),
    };
    let response = app.post("create_account", &body).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    body.username = "username".into();
    body.email =
        format!("{}@example.clom", (0..300).map(|_| "X").collect::<String>());
    let response = app.post("create_account", &body).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    Ok(())
}
