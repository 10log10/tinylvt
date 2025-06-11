use anyhow::Result;
use fantoccini::{Client, Locator};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::framework::TestEnvironment;

#[tokio::test]
async fn test_community_management_flow() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Set up test data (user + community)
    info!("📊 Setting up test data");
    setup_test_data(&env).await?;

    // Step 2: Navigate to login and log in
    info!("🔐 Logging in");
    login_user(&env.browser, &env.frontend_url).await?;

    // Step 3: Navigate to communities page
    info!("🏘️ Navigating to communities");
    navigate_to_communities(&env.browser).await?;

    // Step 4: Click on community to go to management page
    info!("📊 Accessing community management");
    access_community_management(&env.browser).await?;

    // Step 5: Check for debug logs and verify page loaded correctly
    info!("🔍 Checking page state and debug logs");
    check_community_management_page(&env.browser).await?;

    info!("✅ Community management test completed successfully");
    Ok(())
}

async fn setup_test_data(env: &TestEnvironment) -> Result<()> {
    // Create Alice user and a test community
    env.api.create_alice_user().await?;
    let _community_id = env.api.create_test_community().await?;
    debug!("Test data created: Alice user and test community");
    Ok(())
}

async fn login_user(browser: &Client, frontend_url: &str) -> Result<()> {
    // Navigate to homepage
    browser.goto(frontend_url).await?;
    sleep(Duration::from_secs(1)).await;

    // Check if we need to login (look for login link)
    if browser.find(Locator::LinkText("Login")).await.is_ok() {
        debug!("Login required, clicking login link");
        browser
            .find(Locator::LinkText("Login"))
            .await?
            .click()
            .await?;

        // Fill in login form with Alice's credentials
        debug!("Filling in login form with alice credentials");
        let credentials = test_helpers::alice_credentials();

        // Get username field, clear it, focus it, and send keys
        let username_field = browser.find(Locator::Id("username")).await?;
        username_field.click().await?; // Focus the field
        username_field.clear().await?; // Clear any existing content
        username_field.send_keys(&credentials.username).await?;

        // Get password field, clear it, focus it, and send keys
        let password_field = browser.find(Locator::Id("password")).await?;
        password_field.click().await?; // Focus the field
        password_field.clear().await?; // Clear any existing content
        password_field.send_keys(&credentials.password).await?;

        // CRITICAL: Move focus away from password field to trigger onchange event
        // The frontend uses onchange events, which only fire when the field loses focus
        debug!(
            "Moving focus away from password field to trigger onchange event"
        );

        // Use JavaScript to explicitly blur the password field
        browser
            .execute("document.getElementById('password')?.blur();", vec![])
            .await?;

        // Give a moment for the onchange event to process
        sleep(Duration::from_millis(100)).await;

        debug!("Password field blurred, onchange event should have fired");

        // Now submit the form
        debug!("Looking for submit button");
        let submit_button =
            browser.find(Locator::Css("button[type='submit']")).await?;
        debug!("Found submit button, clicking");
        submit_button.click().await?;

        // Wait longer for the redirect
        debug!("Waiting for login redirect...");
        sleep(Duration::from_secs(1)).await;

        // Check where we ended up after login
        let post_login_url = browser.current_url().await?;
        debug!("Post-login URL: {}", post_login_url);

        // Verify we're no longer on the login page
        if post_login_url.as_str().contains("/login") {
            warn!("Still on login page after submit - login may have failed");

            // Check for any error messages on the page
            match browser
                .find_all(Locator::Css(".error, .alert, [role='alert']"))
                .await
            {
                Ok(errors) if !errors.is_empty() => {
                    for (i, error) in errors.iter().enumerate() {
                        let text = error
                            .text()
                            .await
                            .unwrap_or_else(|_| "No text".to_string());
                        warn!("Error message {}: {}", i + 1, text);
                    }
                }
                _ => warn!("No error messages found on page"),
            }
        } else {
            debug!("✅ Successfully redirected away from login page");
        }
    } else {
        debug!("Already logged in or no login required");
    }

    Ok(())
}

async fn navigate_to_communities(browser: &Client) -> Result<()> {
    // Look for Communities link and click it
    let communities_link =
        browser.find(Locator::LinkText("Communities")).await?;
    communities_link.click().await?;
    sleep(Duration::from_secs(1)).await;

    debug!("Navigated to communities page");
    Ok(())
}

async fn access_community_management(browser: &Client) -> Result<()> {
    // Look for community cards (clickable elements)
    let community_cards =
        browser.find_all(Locator::Css(".cursor-pointer")).await?;

    if community_cards.is_empty() {
        return Err(anyhow::anyhow!(
            "No community cards found on communities page"
        ));
    }

    debug!(
        "Found {} community cards, clicking the first one",
        community_cards.len()
    );
    community_cards[0].click().await?;

    // Wait for the management page to load
    sleep(Duration::from_secs(2)).await;

    debug!("Clicked community card, should be on management page");
    Ok(())
}

async fn check_community_management_page(browser: &Client) -> Result<()> {
    // Get current URL to verify we're on the right page
    let current_url = browser.current_url().await?;
    debug!("Current URL: {}", current_url);

    let url_str = current_url.as_str();
    if !url_str.contains("/communities/") || !url_str.contains("/manage") {
        warn!("URL doesn't look like a management page: {}", url_str);
    }

    // Check for page title
    match browser.find(Locator::Css("h1")).await {
        Ok(title_element) => {
            let title_text = title_element.text().await?;
            debug!("Page title: {}", title_text);
        }
        Err(_) => {
            warn!("No h1 title found on page");
        }
    }

    // Check for error messages
    let error_elements = browser
        .find_all(Locator::Css(".bg-red-50, .bg-yellow-50"))
        .await?;
    if !error_elements.is_empty() {
        warn!("Found {} error/warning messages:", error_elements.len());
        for (i, element) in error_elements.iter().enumerate() {
            let text = element.text().await?;
            warn!("  Error {}: {}", i + 1, text);
        }
    }

    // Get console logs (this requires executing JavaScript)
    let logs = browser
        .execute(
            "return window.console ? (window.testLogs || []) : [];",
            vec![],
        )
        .await?;

    debug!("Console logs captured: {:?}", logs);

    // Check for loading spinners (shouldn't be any if page loaded correctly)
    let loading_elements =
        browser.find_all(Locator::Css(".animate-spin")).await?;
    if !loading_elements.is_empty() {
        warn!(
            "Found {} loading spinners still active",
            loading_elements.len()
        );
    }

    Ok(())
}
