use anyhow::Result;
use fantoccini::Locator;
use std::time::Duration;
use test_helpers::alice_login_credentials;
use tokio::time::sleep;
use tracing::{debug, info};

use crate::framework::{login_user, TestEnvironment};

/// UI integration test for US-015: Create new site
///
/// This test covers the user story:
///   As a community moderator, I want to create and configure sites and spaces so members can bid on them.
///
/// Steps:
/// - Navigate to sites page for community
/// - Click create site button
/// - Fill site details form (name, description, timezone)
/// - Configure image selection (ignoring auction params, possession period, lead times, open hours for MVP)
/// - Verify site creation success
/// - *API Coverage*: `create_site`
#[tokio::test]
async fn test_create_new_site() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Set up test data (Alice user and her community)
    info!("📊 Setting up test data (Alice user and community)");
    env.api.create_alice_user().await?;
    let community_id = env.api.create_test_community().await?;
    debug!("Created community with ID: {}", community_id.0);

    // Step 2: Log in as Alice
    login_user(&env.browser, &env.frontend_url, &alice_login_credentials())
        .await?;

    // Step 3: Navigate to communities page first
    info!("🏘️ Navigating to communities page");
    let communities_link = env.browser.find(Locator::LinkText("Communities")).await?;
    communities_link.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Step 4: Click on the community to enter its dashboard
    info!("🏘️ Clicking on community to enter dashboard");
    let community_link = env
        .browser
        .find(Locator::XPath("//div[contains(@class, 'cursor-pointer')]"))
        .await?;
    community_link.click().await?;
    sleep(Duration::from_secs(2)).await;

    // Verify we're on the community dashboard
    let current_url = env.browser.current_url().await?;
    assert!(
        current_url.as_str().contains(&format!("/community/{}", community_id.0)),
        "Should be on community dashboard page. Current URL: {}",
        current_url
    );

    // Step 5: Click create site button directly from community dashboard
    info!("➕ Looking for create site button on community dashboard");
    
    // Optional: debug what's on the page if needed
    // let page_body = env.browser.find(Locator::Css("body")).await?;
    // let page_text = page_body.text().await?;
    // debug!("Community dashboard page content: {}", page_text);
    
    // Based on the dashboard content, look for the "Create" button in the Sites section
    // From the debug output, we can see there's a "Create" text right after "View All"
    let create_site_button = if let Ok(button) = env
        .browser
        .find(Locator::XPath("//a[contains(text(), 'Create')]"))
        .await
    {
        button
    } else if let Ok(link) = env
        .browser
        .find(Locator::XPath("//button[contains(text(), 'Create')]"))
        .await
    {
        link
    } else if let Ok(button) = env
        .browser
        .find(Locator::XPath("//a[contains(@href, 'create')]"))
        .await
    {
        button
    } else {
        // Fallback: look for any element with "Create" near sites content
        env.browser.find(Locator::LinkText("Create")).await?
    };
        
    create_site_button.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Step 6: Verify we're on the create site page
    let create_site_url = env.browser.current_url().await?;
    assert!(
        create_site_url.as_str().contains("/create"),
        "Should be on create site page"
    );

    // Step 7: Fill site creation form
    info!("📝 Filling site creation form");
    let site_name = format!(
        "Test Site {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    // Fill in the site name
    let name_field = env.browser.find(Locator::Id("name")).await?;
    name_field.click().await?;
    name_field.clear().await?;
    name_field.send_keys(&site_name).await?;

    // Fill in the description (optional)
    let description_field = env.browser.find(Locator::Id("description")).await?;
    description_field.click().await?;
    description_field.clear().await?;
    description_field.send_keys("This is a test site created by automated testing").await?;

    // Select timezone (should have a default, but let's make sure)
    let timezone_field = env.browser.find(Locator::Id("timezone")).await?;
    timezone_field.click().await?;
    // Select a specific timezone for testing
    // Try to find a specific timezone, or use a fallback
    let timezone_option = match env
        .browser
        .find(Locator::XPath("//option[contains(text(), 'America/New_York')]"))
        .await
    {
        Ok(option) => option,
        Err(_) => {
            // Fallback to any timezone if America/New_York is not available
            env.browser.find(Locator::XPath("//select[@id='timezone']/option[2]")).await?
        }
    };
    timezone_option.click().await?;

    // Note: We're ignoring advanced options for now as specified (auction params, possession period, etc.)
    // These fields might not even be implemented in the MVP UI

    // Step 8: Submit the form
    info!("🚀 Submitting site creation form");
    let submit_button = env
        .browser
        .find(Locator::Css("button[type='submit']"))
        .await?;
    submit_button.click().await?;
    sleep(Duration::from_secs(3)).await; // Site creation might take a moment

    // Step 9: Verify successful creation and redirect
    info!("🔍 Verifying site creation success");
    let post_create_url = env.browser.current_url().await?;
    
    // Should be redirected back to sites list or community dashboard
    assert!(
        (post_create_url.as_str().contains("/sites") && !post_create_url.as_str().contains("/create")) ||
        post_create_url.as_str().contains("/community/"),
        "Should be redirected to sites list or community dashboard after successful creation"
    );

    // Step 10: Verify the site appears in the UI
    info!("✅ Verifying site appears in sites list");
    
    // Wait for the page to load the sites
    sleep(Duration::from_secs(1)).await;

    // Look for the site name in the page content
    let page_content = env.browser.find(Locator::Css("body")).await?;
    let page_text = page_content.text().await?;
    assert!(
        page_text.contains(&site_name),
        "Site name should appear in the sites list"
    );

    // Verify that we no longer see "No sites" message
    assert!(
        !page_text.contains("No sites yet") && !page_text.contains("No sites"),
        "Should not show 'No sites' after creating one"
    );

    info!("✅ Create new site test completed successfully");
    Ok(())
}

/// UI integration test for US-016: View and edit existing site (basic viewing)
///
/// This test covers viewing a site that was created, focusing on the basic viewing functionality.
/// Editing functionality can be expanded later.
///
/// Steps:
/// - Set up a site using API
/// - Navigate to sites page
/// - Click on the site to view details
/// - Verify site information is displayed correctly
/// - *API Coverage*: `get_site`, `list_sites`
#[tokio::test]
async fn test_view_existing_site() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Set up test data (Alice user, community, and a site)
    info!("📊 Setting up test data (Alice user, community, and site)");
    env.api.create_alice_user().await?;
    let community_id = env.api.create_test_community().await?;
    let site = env.api.create_test_site(&community_id).await?;
    debug!("Created site with ID: {}", site.site_id.0);

    // Step 2: Log in as Alice
    login_user(&env.browser, &env.frontend_url, &alice_login_credentials())
        .await?;

    // Step 3: Navigate to sites page
    info!("🏢 Navigating to sites page");
    let communities_link = env.browser.find(Locator::LinkText("Communities")).await?;
    communities_link.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Click on the community
    let community_link = env
        .browser
        .find(Locator::XPath("//div[contains(@class, 'cursor-pointer')]"))
        .await?;
    community_link.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Debug: see what's on the community dashboard when there's already a site
    let dashboard_body = env.browser.find(Locator::Css("body")).await?;
    let dashboard_text = dashboard_body.text().await?;
    debug!("Community dashboard with existing site: {}", dashboard_text);
    
    // Navigate to sites - look for "View All" since there should be a site already
    let sites_link = if let Ok(view_all_link) = env
        .browser
        .find(Locator::LinkText("View All"))
        .await
    {
        view_all_link
    } else if let Ok(view_all_link) = env
        .browser
        .find(Locator::XPath("//a[contains(text(), 'View All')]"))
        .await
    {
        view_all_link
    } else {
        // If we can't navigate to the sites list, just click on the site card directly
        // The dashboard shows the site, so we can click on it
        info!("Could not find View All link, clicking on site card directly");
        let site_card = env
            .browser
            .find(Locator::XPath("//div[contains(@class, 'cursor-pointer')]"))
            .await?;
        site_card.click().await?;
        sleep(Duration::from_secs(2)).await;
        
        // Skip the rest of the navigation since we went directly to site details
        let details_url = env.browser.current_url().await?;
        debug!("Site details URL after clicking site card: {}", details_url);
        debug!("Expected site ID in URL: {}", site.site_id.0);
        assert!(
            details_url.as_str().contains(&format!("/sites/{}", site.site_id.0)) ||
            details_url.as_str().contains(&format!("/site/{}", site.site_id.0)),
            "Should be on site details page. Current URL: {}, Expected site ID: {}",
            details_url, site.site_id.0
        );
        
        // Jump to verification step
        info!("✅ Verifying site information is displayed correctly");
        let page_content = env.browser.find(Locator::Css("body")).await?;
        let page_text = page_content.text().await?;

        // Check that site name appears
        assert!(
            page_text.contains(&site.site_details.name),
            "Site name should be displayed on the details page"
        );

        // Check that site description appears (if it exists)
        if let Some(description) = &site.site_details.description {
            assert!(
                page_text.contains(description),
                "Site description should be displayed if it exists"
            );
        }

        // Check that timezone appears
        assert!(
            page_text.contains(&site.site_details.timezone),
            "Site timezone should be displayed"
        );

        info!("✅ View existing site test completed successfully");
        return Ok(());
    };
    sites_link.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Step 4: Click on the site to view its details
    info!("👁️ Clicking on site to view details");
    let site_card = env
        .browser
        .find(Locator::XPath("//div[contains(@class, 'cursor-pointer')]"))
        .await?;
    site_card.click().await?;
    sleep(Duration::from_secs(2)).await;

    // Step 5: Verify we're on the site details page
    info!("🔍 Verifying we're on site details page");
    let details_url = env.browser.current_url().await?;
    assert!(
        details_url.as_str().contains(&format!("/sites/{}", site.site_id.0)),
        "Should be on site details page"
    );

    // Step 6: Verify site information is displayed
    info!("✅ Verifying site information is displayed correctly");
    let page_content = env.browser.find(Locator::Css("body")).await?;
    let page_text = page_content.text().await?;

    // Check that site name appears
    assert!(
        page_text.contains(&site.site_details.name),
        "Site name should be displayed on the details page"
    );

    // Check that site description appears (if it exists)
    if let Some(description) = &site.site_details.description {
        assert!(
            page_text.contains(description),
            "Site description should be displayed if it exists"
        );
    }

    // Check that timezone appears
    assert!(
        page_text.contains(&site.site_details.timezone),
        "Site timezone should be displayed"
    );

    info!("✅ View existing site test completed successfully");
    Ok(())
} 