use anyhow::Result;
use fantoccini::Locator;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

use crate::framework::TestEnvironment;

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

