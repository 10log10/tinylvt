use jiff_sqlx::ToSqlx;
use payloads::requests;
use reqwest::StatusCode;
use test_helpers::{assert_status_code, spawn_app};

#[tokio::test]
async fn test_email_verification_flow() {
    let app = spawn_app().await;
    app.test_email_verification_flow().await.unwrap();
}

#[tokio::test]
async fn test_password_reset_flow() {
    let app = spawn_app().await;
    app.test_password_reset_flow().await.unwrap();
}

#[tokio::test]
async fn test_verify_email_with_invalid_token() {
    let app = spawn_app().await;

    let invalid_token = "00000000-0000-0000-0000-000000000000";
    let verify_request = requests::VerifyEmail {
        token: invalid_token.to_string(),
    };

    let result = app.client.verify_email(&verify_request).await;
    assert_status_code(result, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_verify_email_with_malformed_token() {
    let app = spawn_app().await;

    let malformed_token = "not-a-uuid";
    let verify_request = requests::VerifyEmail {
        token: malformed_token.to_string(),
    };

    let result = app.client.verify_email(&verify_request).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_verify_email_with_used_token() {
    let app = spawn_app().await;

    let credentials = requests::CreateAccount {
        email: "test-used-token@example.com".to_string(),
        username: "testusedtoken".to_string(),
        password: "password123".to_string(),
    };

    // 1. Create account
    app.create_unverified_user(&credentials).await.unwrap();

    // 2. Get verification token
    let token = app
        .get_verification_token_from_db(&credentials.email)
        .await
        .unwrap();

    // 3. Use token once (should succeed)
    let verify_request = requests::VerifyEmail {
        token: token.clone(),
    };
    app.client.verify_email(&verify_request).await.unwrap();

    // 4. Try to use token again (should fail)
    let verify_request = requests::VerifyEmail { token };
    let result = app.client.verify_email(&verify_request).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_reset_password_with_invalid_token() {
    let app = spawn_app().await;

    let invalid_token = "00000000-0000-0000-0000-000000000000";
    let reset_request = requests::ResetPassword {
        token: invalid_token.to_string(),
        password: "newpassword123".to_string(),
    };

    let result = app.client.reset_password(&reset_request).await;
    assert_status_code(result, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_reset_password_with_malformed_token() {
    let app = spawn_app().await;

    let malformed_token = "not-a-uuid";
    let reset_request = requests::ResetPassword {
        token: malformed_token.to_string(),
        password: "newpassword123".to_string(),
    };

    let result = app.client.reset_password(&reset_request).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_forgot_password_prevents_email_enumeration() {
    let app = spawn_app().await;

    // Test with non-existent email - should still return success
    let forgot_request = requests::ForgotPassword {
        email: "nonexistent@example.com".to_string(),
    };

    let result = app.client.forgot_password(&forgot_request).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_resend_verification_email_requires_auth() {
    let app = spawn_app().await;

    // Without logging in, should fail with unauthorized
    let result = app.client.resend_verification_email().await;
    assert_status_code(result, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_resend_verification_email_works() {
    let app = spawn_app().await;

    let credentials = requests::CreateAccount {
        email: "test-resend@example.com".to_string(),
        username: "testresend".to_string(),
        password: "password123".to_string(),
    };

    // 1. Create unverified account and login
    app.create_unverified_user(&credentials).await.unwrap();
    app.client
        .login(&requests::LoginCredentials {
            username: credentials.username.clone(),
            password: credentials.password.clone(),
        })
        .await
        .unwrap();

    // 2. Request resend verification email
    app.client.resend_verification_email().await.unwrap();

    // 3. Get a valid verification token and use it
    let token = app
        .get_verification_token_from_db(&credentials.email)
        .await
        .unwrap();

    let verify_request = requests::VerifyEmail { token };
    app.client.verify_email(&verify_request).await.unwrap();

    // 4. Email should now be verified
    assert!(app.is_email_verified(&credentials.email).await.unwrap());
}

#[tokio::test]
async fn test_resend_verification_email_already_verified() {
    let app = spawn_app().await;

    let credentials = requests::CreateAccount {
        email: "test-already-verified@example.com".to_string(),
        username: "testalreadyverified".to_string(),
        password: "password123".to_string(),
    };

    // 1. Create unverified account
    app.create_unverified_user(&credentials).await.unwrap();

    // 2. Verify email through the normal flow
    let token = app
        .get_verification_token_from_db(&credentials.email)
        .await
        .unwrap();
    let verify_request = requests::VerifyEmail { token };
    app.client.verify_email(&verify_request).await.unwrap();

    // 3. Login
    app.client
        .login(&requests::LoginCredentials {
            username: credentials.username.clone(),
            password: credentials.password.clone(),
        })
        .await
        .unwrap();

    // 4. Request resend verification email for already verified account
    // Should return success (email already verified message)
    let result = app.client.resend_verification_email().await;
    assert!(result.is_ok());

    // No new unused token should exist since email is already verified
    let token_result =
        app.get_verification_token_from_db(&credentials.email).await;
    assert!(token_result.is_err());
}

#[tokio::test]
async fn test_expired_token_handling() {
    let app = spawn_app().await;

    let credentials = requests::CreateAccount {
        email: "test-expired@example.com".to_string(),
        username: "testexpired".to_string(),
        password: "password123".to_string(),
    };

    // 1. Create account
    app.create_unverified_user(&credentials).await.unwrap();

    // 2. Get verification token
    let token = app
        .get_verification_token_from_db(&credentials.email)
        .await
        .unwrap();

    // 3. Manually expire the token in the database using the mocked time source
    let expired_time = app.time_source.now() - jiff::Span::new().hours(1);
    sqlx::query("UPDATE tokens SET expires_at = $1 WHERE id = $2::uuid")
        .bind(expired_time.to_sqlx())
        .bind(&token)
        .execute(&app.db_pool)
        .await
        .unwrap();

    // 4. Try to use expired token
    let verify_request = requests::VerifyEmail { token };
    let result = app.client.verify_email(&verify_request).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);
}
