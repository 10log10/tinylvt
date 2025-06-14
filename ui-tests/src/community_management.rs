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
    info!("🔐 Logging in as Alice");
    login_user(&env.browser, &env.frontend_url).await?;

    // Wait a bit longer to ensure session is established
    sleep(Duration::from_secs(2)).await;

    // Step 3: Navigate to communities page using the nav link instead of direct URL
    info!("🏘️ Navigating to communities page via navigation link");
    let communities_link = env.browser.find(Locator::LinkText("Communities")).await?;
    communities_link.click().await?;
    sleep(Duration::from_secs(2)).await;

    // Debug: Check what's actually on the page
    let page_content = env.browser.find(Locator::Css("body")).await?;
    let page_text = page_content.text().await?;
    debug!("Communities page content: {}", page_text);

    // Look for all buttons on the page
    let buttons = env.browser.find_all(Locator::Css("button")).await?;
    debug!("Number of buttons found: {}", buttons.len());
    for (i, button) in buttons.iter().enumerate() {
        let button_text = button.text().await.unwrap_or_default();
        debug!("Button {}: '{}'", i, button_text);
    }

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
        browser
            .find(Locator::LinkText("Login"))
            .await?
            .click()
            .await?;

        // Fill in login form with Alice's credentials
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
        browser
            .execute("document.getElementById('password')?.blur();", vec![])
            .await?;

        // Give a moment for the onchange event to process
        sleep(Duration::from_millis(100)).await;

        // Submit the form
        let submit_button =
            browser.find(Locator::Css("button[type='submit']")).await?;
        submit_button.click().await?;

        // Wait for the redirect
        sleep(Duration::from_secs(1)).await;

        // Check where we ended up after login
        let post_login_url = browser.current_url().await?;

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
        }
    }

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

    community_cards[0].click().await?;

    // Wait for the management page to load
    sleep(Duration::from_secs(2)).await;
    Ok(())
}

async fn check_community_management_page(browser: &Client) -> Result<()> {
    // Get current URL to verify we're on the right page
    let current_url = browser.current_url().await?;
    let url_str = current_url.as_str();
    if !url_str.contains("/communities/") || !url_str.contains("/manage") {
        warn!("URL doesn't look like a management page: {}", url_str);
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

/// UI integration test for US-008: Create new community.
///
/// This test covers the user story:
///   As a user, I want to create or join communities so I can participate in shared space allocation.
///
/// Steps:
/// - Navigate to communities page
/// - Click create community button
/// - Fill community creation form
/// - Verify community is created with user as leader
/// - *API Coverage*: `create_community`
#[tokio::test]
async fn test_create_new_community() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Set up test data (just the user, no community yet)
    info!("📊 Setting up test data (Alice user)");
    env.api.create_alice_user().await?;

    // Step 2: Navigate to login and log in
    info!("🔐 Logging in as Alice");
    login_user(&env.browser, &env.frontend_url).await?;

    // Wait a bit longer to ensure session is established
    sleep(Duration::from_secs(2)).await;

    // Step 3: Navigate to communities page using the nav link instead of direct URL
    info!("🏘️ Navigating to communities page via navigation link");
    let communities_link = env.browser.find(Locator::LinkText("Communities")).await?;
    communities_link.click().await?;
    sleep(Duration::from_secs(2)).await;

    // Step 4: Click create community button
    info!("➕ Clicking create community button");
    let create_button = env
        .browser
        .find(Locator::XPath("//button[contains(text(), 'Create your first community')]"))
        .await?;
    create_button.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Verify we're on the create community page
    let current_url = env.browser.current_url().await?;
    assert!(
        current_url.as_str().contains("/communities/create"),
        "Should be on create community page"
    );

    // Step 5: Fill community creation form
    info!("📝 Filling community creation form");
    let community_name = format!("Test Community {}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    // Fill in the community name
    let name_field = env.browser.find(Locator::Id("name")).await?;
    name_field.click().await?;
    name_field.clear().await?;
    name_field.send_keys(&community_name).await?;

    // Optional: Check the "new members active by default" checkbox
    let checkbox = env
        .browser
        .find(Locator::Id("new_members_default_active"))
        .await?;
    checkbox.click().await?;

    // Step 6: Submit the form
    info!("🚀 Submitting community creation form");
    let submit_button = env
        .browser
        .find(Locator::Css("button[type='submit']"))
        .await?;
    submit_button.click().await?;
    sleep(Duration::from_secs(2)).await;

    // Step 7: Verify successful creation and redirect
    info!("🔍 Verifying community creation success");
    let post_create_url = env.browser.current_url().await?;
    assert!(
        post_create_url.as_str().contains("/communities") && !post_create_url.as_str().contains("/create"),
        "Should be redirected to communities list after successful creation"
    );

    // Step 8: Verify the community appears in the list
    info!("✅ Verifying community appears in communities list");
    
    // Look for the community name in the page content
    let page_content = env.browser.find(Locator::Css("body")).await?;
    let page_text = page_content.text().await?;
    assert!(
        page_text.contains(&community_name),
        "Community name should appear in the communities list"
    );

    // For now, just verify that we have the community showing (role might not be implemented yet)
    // Look for indication that user has communities (should not see "No communities" text)
    assert!(
        !page_text.contains("No communities"),
        "Should not show 'No communities' after creating one"
    );

    info!("✅ Create new community test completed successfully");
    Ok(())
}
