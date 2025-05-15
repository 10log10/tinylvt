use payloads::requests;
use reqwest::StatusCode;

use crate::helpers::{assert_status_code, spawn_app};

#[tokio::test]
async fn login_refused() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // test a login with an invalid user
    let body = requests::CreateAccount {
        username: "random".into(),
        password: "random".into(),
        email: "random@example.com".into(),
    };
    let result = app.client.login(&body).await;

    match result {
        Err(payloads::ClientError::APIError(code, text)) => {
            assert_eq!(code, StatusCode::UNAUTHORIZED);
            assert_eq!(text, "Authentication failed: Invalid credentials");
        }
        _ => {
            panic!("Expected APIError");
        }
    }

    // login check should fail
    let is_logged_in = app.client.login_check().await?;
    assert!(!is_logged_in);

    Ok(())
}

#[tokio::test]
async fn create_account() -> anyhow::Result<()> {
    let app = spawn_app().await;

    app.create_alice_user().await?;

    // check for valid session
    let is_logged_in = app.client.login_check().await?;
    assert!(is_logged_in);

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
    let result = app.client.create_account(&body).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);

    body.username = "username".into();
    body.email =
        format!("{}@example.clom", (0..300).map(|_| "X").collect::<String>());
    let result = app.client.create_account(&body).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}
