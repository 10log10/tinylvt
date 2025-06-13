use payloads::requests;
use reqwest::StatusCode;

use test_helpers::{assert_status_code, spawn_app};

#[tokio::test]
async fn login_refused() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // test a login with an invalid user
    let body = requests::LoginCredentials {
        username: "random".into(),
        password: "random".into(),
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

#[tokio::test]
async fn update_profile_success() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // Create a user and login
    app.create_alice_user().await?;

    // Check initial profile
    let profile = app.client.user_profile().await?;
    println!("Initial profile: {:?}", profile);
    assert_eq!(profile.username, "alice");
    assert_eq!(profile.display_name, None);

    // Update display name
    let update_request = requests::UpdateProfile {
        display_name: Some("Alice Smith".to_string()),
    };
    println!("Sending update request: {:?}", update_request);
    let updated_profile = app.client.update_profile(&update_request).await?;
    println!("Updated profile: {:?}", updated_profile);
    assert_eq!(updated_profile.display_name, Some("Alice Smith".to_string()));
    assert_eq!(updated_profile.username, "alice"); // Username should remain unchanged

    // Verify the change persists
    let profile = app.client.user_profile().await?;
    println!("Final profile: {:?}", profile);
    assert_eq!(profile.display_name, Some("Alice Smith".to_string()));

    Ok(())
}

#[tokio::test]
async fn update_profile_clear_display_name() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // Create a user and login
    app.create_alice_user().await?;

    // Set a display name first
    let update_request = requests::UpdateProfile {
        display_name: Some("Alice Smith".to_string()),
    };
    app.client.update_profile(&update_request).await?;

    // Clear the display name
    let update_request = requests::UpdateProfile {
        display_name: None,
    };
    let updated_profile = app.client.update_profile(&update_request).await?;
    assert_eq!(updated_profile.display_name, None);

    Ok(())
}

#[tokio::test]
async fn update_profile_display_name_too_long() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // Create a user and login
    app.create_alice_user().await?;

    // Try to set a display name that's too long
    let long_display_name = (0..256).map(|_| "X").collect::<String>();
    let update_request = requests::UpdateProfile {
        display_name: Some(long_display_name),
    };
    
    let result = app.client.update_profile(&update_request).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn update_profile_requires_authentication() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // Try to update profile without being logged in
    let update_request = requests::UpdateProfile {
        display_name: Some("Alice Smith".to_string()),
    };
    
    let result = app.client.update_profile(&update_request).await;
    assert_status_code(result, StatusCode::UNAUTHORIZED);

    Ok(())
}
