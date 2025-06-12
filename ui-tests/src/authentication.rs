use anyhow::Result;
use fantoccini::Locator;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

use crate::framework::TestEnvironment;

/// UI integration test for US-001: Create new account with email verification.
///
/// This test covers the user story:
///   As a user, I want to create and manage my account so I can participate in the system.
///
/// Steps:
/// - Navigate to the registration page
/// - Fill out the registration form (email, username, password, confirm password)
/// - Submit the form
/// - Verify that the 'Check your email' heading is shown, indicating the verification prompt
#[tokio::test]
async fn test_account_registration_flow() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Navigate to registration page
    info!("📝 Navigating to registration page");
    env.browser
        .goto(&format!("{}/register", env.frontend_url))
        .await?;
    sleep(Duration::from_secs(1)).await;

    // Step 2: Fill registration form
    info!("🔑 Filling registration form");
    let username = format!("testuser_{}", rand::random::<u32>());
    let email = format!("{}@example.com", username);
    let password = "TestPassword123!";

    let email_field = env.browser.find(Locator::Id("email")).await?;
    email_field.click().await?;
    email_field.clear().await?;
    email_field.send_keys(&email).await?;

    let username_field = env.browser.find(Locator::Id("username")).await?;
    username_field.click().await?;
    username_field.clear().await?;
    username_field.send_keys(&username).await?;

    let password_field = env.browser.find(Locator::Id("password")).await?;
    password_field.click().await?;
    password_field.clear().await?;
    password_field.send_keys(password).await?;

    let confirm_field =
        env.browser.find(Locator::Id("confirm-password")).await?;
    confirm_field.click().await?;
    confirm_field.clear().await?;
    confirm_field.send_keys(password).await?;
    // Blur confirm password field to trigger onchange
    env.browser
        .execute(
            "document.getElementById('confirm-password')?.blur();",
            vec![],
        )
        .await?;
    sleep(Duration::from_millis(100)).await;
    info!("Filled confirm password field");

    // Blur password field to trigger onchange
    env.browser
        .execute("document.getElementById('password')?.blur();", vec![])
        .await?;
    sleep(Duration::from_millis(100)).await;

    // Step 3: Submit the form
    info!("🚀 Submitting registration form");
    let submit_button = env
        .browser
        .find(Locator::Css("button[type='submit']"))
        .await?;
    submit_button.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Step 4: Verify registration success and email verification prompt
    info!("🔍 Verifying registration success and email verification prompt");
    let current_url = env.browser.current_url().await?;
    debug!("Current URL after registration: {}", current_url);

    // Look for the heading 'Check your email' (VerifyEmailPrompt)
    let heading = env
        .browser
        .find(Locator::XPath(
            "//h3[contains(text(), 'Please verify your email')]",
        ))
        .await?;
    let heading_text = heading.text().await.unwrap_or_default();
    info!("Found heading after registration: {}", heading_text);

    info!("✅ Registration test completed successfully");
    Ok(())
}

/// UI integration test for US-002: Login with valid credentials.
///
/// This test covers the user story:
///   As a user, I want to log in with valid credentials and have my session persist.
///
/// Steps:
/// - Ensure Alice user exists and is verified
/// - Navigate to the login page
/// - Fill out the login form (username, password)
/// - Submit the form
/// - Verify successful login and redirect
/// - Reload the page and verify session persistence
#[tokio::test]
async fn test_login_with_valid_credentials() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Ensure Alice user exists and is verified
    info!("👤 Ensuring Alice user exists and is verified");
    env.api.create_alice_user().await?;
    let credentials = test_helpers::alice_credentials();

    // Step 2: Navigate to login page
    info!("🔑 Navigating to login page");
    env.browser
        .goto(&format!("{}/login", env.frontend_url))
        .await?;
    sleep(Duration::from_secs(1)).await;

    // Step 3: Fill login form
    info!("✍️ Filling login form");
    let username_field = env.browser.find(Locator::Id("username")).await?;
    username_field.click().await?;
    username_field.clear().await?;
    username_field.send_keys(&credentials.username).await?;

    let password_field = env.browser.find(Locator::Id("password")).await?;
    password_field.click().await?;
    password_field.clear().await?;
    password_field.send_keys(&credentials.password).await?;

    // Blur password field to trigger onchange
    env.browser
        .execute("document.getElementById('password')?.blur();", vec![])
        .await?;
    sleep(Duration::from_millis(100)).await;

    // Step 4: Submit the form
    info!("🚀 Submitting login form");
    let submit_button = env
        .browser
        .find(Locator::Css("button[type='submit']"))
        .await?;
    submit_button.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Step 5: Verify successful login and redirect
    info!("🔍 Verifying successful login and redirect");
    let current_url = env.browser.current_url().await?;
    debug!("Current URL after login: {}", current_url);
    assert!(
        !current_url.as_str().contains("/login"),
        "Should not remain on login page after successful login"
    );

    // Optionally, check for a user-specific element (e.g., username in nav)
    // let user_nav = env.browser.find(Locator::Css("#user-nav")).await?;
    // let nav_text = user_nav.text().await.unwrap_or_default();
    // assert!(nav_text.contains(&credentials.username));

    // Step 6: Reload the page and verify session persistence
    info!("🔄 Reloading page to verify session persistence");
    env.browser.refresh().await?;
    sleep(Duration::from_secs(1)).await;
    let url_after_reload = env.browser.current_url().await?;
    debug!("URL after reload: {}", url_after_reload);
    assert!(
        !url_after_reload.as_str().contains("/login"),
        "Session should persist after reload"
    );

    info!("✅ Login test completed successfully");
    Ok(())
}
