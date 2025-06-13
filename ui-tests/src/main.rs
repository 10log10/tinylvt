// This crate uses standard Rust tests with #[tokio::test]
// Run with: cargo test
//
// For human-in-the-loop debugging, we could add a main() function later
// that sets up test data and opens a headed browser for manual inspection.

use anyhow::Result;
use fantoccini::Locator;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

mod framework;

use crate::framework::TestEnvironment;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing using the same setup as the API
    let subscriber = api::telemetry::get_subscriber("info".into());
    api::telemetry::init_subscriber(subscriber);

    info!("🚀 Starting UI test environment with Alice user login");

    // Set up the test environment (API server, frontend, browser) in headed mode
    info!("🔧 Setting up test environment with headed browser");
    let env = TestEnvironment::setup_headed().await?;

    // Create Alice user
    info!("👤 Creating Alice user");
    env.api.create_alice_user().await?;
    let credentials = test_helpers::alice_credentials();
    info!("✅ Alice user created: {}", credentials.username);

    // Navigate to login page
    info!("🔑 Navigating to login page");
    env.browser
        .goto(&format!("{}/login", env.frontend_url))
        .await?;
    sleep(Duration::from_secs(1)).await;

    // Fill login form
    info!("✍️ Filling login form with Alice's credentials");
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

    // Submit the form
    info!("🚀 Submitting login form");
    let submit_button = env
        .browser
        .find(Locator::Css("button[type='submit']"))
        .await?;
    submit_button.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Verify successful login
    info!("🔍 Verifying successful login");
    let current_url = env.browser.current_url().await?;
    debug!("Current URL after login: {}", current_url);

    if current_url.as_str().contains("/login") {
        eprintln!("❌ Login failed - still on login page");
        return Err(anyhow::anyhow!("Login failed"));
    }

    info!("✅ Successfully logged in as Alice!");
    info!("🌐 Browser is now open at: {}", current_url);
    info!("👋 Press Ctrl+C to exit and close the browser");

    // Set up Ctrl+C handler
    let ctrl_c = tokio::signal::ctrl_c();

    // Keep the browser open until Ctrl+C
    tokio::select! {
        _ = ctrl_c => {
            info!("📝 Received keyboard interrupt, shutting down...");
        }
    }

    info!("🧹 Cleaning up and closing browser");
    Ok(())
}
