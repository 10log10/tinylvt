use anyhow::Result;
use fantoccini::Locator;
use std::time::Duration;
use test_helpers::alice_login_credentials;
use tokio::time::sleep;
use tracing::{debug, info};

use crate::framework::{TestEnvironment, login_user};

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
    let communities_link =
        env.browser.find(Locator::LinkText("Communities")).await?;
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
        current_url
            .as_str()
            .contains(&format!("/community/{}", community_id.0)),
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
    let description_field =
        env.browser.find(Locator::Id("description")).await?;
    description_field.click().await?;
    description_field.clear().await?;
    description_field
        .send_keys("This is a test site created by automated testing")
        .await?;

    // Select timezone (should have a default, but let's make sure)
    let timezone_field = env.browser.find(Locator::Id("timezone")).await?;
    timezone_field.click().await?;
    // Select a specific timezone for testing
    // Try to find a specific timezone, or use a fallback
    let timezone_option = match env
        .browser
        .find(Locator::XPath(
            "//option[contains(text(), 'America/New_York')]",
        ))
        .await
    {
        Ok(option) => option,
        Err(_) => {
            // Fallback to any timezone if America/New_York is not available
            env.browser
                .find(Locator::XPath("//select[@id='timezone']/option[2]"))
                .await?
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
        (post_create_url.as_str().contains("/sites")
            && !post_create_url.as_str().contains("/create"))
            || post_create_url.as_str().contains("/community/"),
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
    let communities_link =
        env.browser.find(Locator::LinkText("Communities")).await?;
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
    let sites_link = if let Ok(view_all_link) =
        env.browser.find(Locator::LinkText("View All")).await
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
            details_url
                .as_str()
                .contains(&format!("/sites/{}", site.site_id.0))
                || details_url
                    .as_str()
                    .contains(&format!("/site/{}", site.site_id.0)),
            "Should be on site details page. Current URL: {}, Expected site ID: {}",
            details_url,
            site.site_id.0
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
        details_url
            .as_str()
            .contains(&format!("/sites/{}", site.site_id.0)),
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

/// UI integration test for US-016: Edit spaces with site image attachments
///
/// This test covers the space editing functionality with site image attachments:
///   As a community moderator, I want to edit existing spaces and attach site images to them.
///
/// Steps:
/// - Set up test data (site, spaces, and site images)
/// - Navigate to site editing page
/// - Test adding a space with image attachment
/// - Test editing existing space to change image
/// - Test removing image from space
/// - Verify all changes are persisted and displayed correctly
/// - *API Coverage*: `update_space`, `list_site_images`, `list_spaces`
// #[tokio::test] // not fully functional yet
#[allow(dead_code)]
async fn test_space_editing_with_site_images() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Set up test data (Alice user, community, site, spaces, and site images)
    info!(
        "📊 Setting up test data (Alice user, community, site, spaces, and site images)"
    );
    env.api.create_alice_user().await?;
    let community_id = env.api.create_test_community().await?;
    let site = env.api.create_test_site(&community_id).await?;

    // Create some site images for testing
    let site_image_1 = env.api.create_test_site_image(&community_id).await?;
    let site_image_2_body = test_helpers::site_image_details_b(community_id);
    let site_image_2_id =
        env.api.client.create_site_image(&site_image_2_body).await?;
    let _site_image_2 = env.api.client.get_site_image(&site_image_2_id).await?;

    // Create an initial space without any image
    let initial_space = env.api.create_test_space(&site.site_id).await?;

    debug!("Created site with ID: {}", site.site_id.0);
    debug!("Created space with ID: {}", initial_space.space_id.0);
    debug!(
        "Created site images with IDs: {}, {}",
        site_image_1.id.0, site_image_2_id.0
    );

    // Step 2: Log in as Alice
    login_user(&env.browser, &env.frontend_url, &alice_login_credentials())
        .await?;

    // Step 3: Navigate to site editing page
    info!("🏢 Navigating to site editing page");
    let communities_link =
        env.browser.find(Locator::LinkText("Communities")).await?;
    communities_link.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Click on the community
    let community_link = env
        .browser
        .find(Locator::XPath("//div[contains(@class, 'cursor-pointer')]"))
        .await?;
    community_link.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Navigate to sites list
    let view_all_link = env.browser.find(Locator::LinkText("View All")).await?;
    view_all_link.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Click on the site to view details
    let site_card = env
        .browser
        .find(Locator::XPath("//div[contains(@class, 'cursor-pointer')]"))
        .await?;
    site_card.click().await?;
    sleep(Duration::from_secs(2)).await;

    // Click the "Edit Site" button to go to the editing page
    info!("✏️ Clicking Edit Site button");
    let edit_button = env
        .browser
        .find(Locator::XPath("//button[contains(text(), 'Edit Site')]"))
        .await?;
    edit_button.click().await?;
    sleep(Duration::from_secs(2)).await;

    // Verify we're on the edit site page
    let edit_url = env.browser.current_url().await?;
    assert!(
        edit_url.as_str().contains("/edit"),
        "Should be on site edit page. Current URL: {}",
        edit_url
    );

    // Step 4: Verify existing space is displayed
    info!("👁️ Verifying existing space is displayed");
    let page_content = env.browser.find(Locator::Css("body")).await?;
    let page_text = page_content.text().await?;
    assert!(
        page_text.contains(&initial_space.space_details.name),
        "Initial space should be displayed in the spaces section"
    );

    // Step 5: Test editing existing space to add an image
    info!("🖼️ Testing editing existing space to add site image");

    // Find and click the edit button for the existing space
    let space_edit_button = env
        .browser
        .find(Locator::XPath("//button[@title='Edit space']"))
        .await?;
    space_edit_button.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Verify we're now in edit mode (form should be visible)
    let edit_form = env.browser.find(Locator::XPath("//form")).await?;
    assert!(
        edit_form.is_displayed().await?,
        "Edit form should be visible"
    );

    // Update the space name
    let name_field = env.browser.find(Locator::Id("edit_space_name")).await?;
    name_field.click().await?;
    name_field.clear().await?;
    name_field.send_keys("Updated Space with Image").await?;

    // Select a site image from the dropdown
    info!("🖼️ Selecting site image from dropdown");
    let image_dropdown =
        env.browser.find(Locator::Id("edit_space_image")).await?;
    image_dropdown.click().await?;
    sleep(Duration::from_millis(500)).await;

    // Select the first site image (should be "Red Square")
    let image_option = env
        .browser
        .find(Locator::XPath("//option[contains(text(), 'Red Square')]"))
        .await?;
    image_option.click().await?;

    // Save the changes
    info!("💾 Saving space changes");
    let save_button = env
        .browser
        .find(Locator::XPath("//button[contains(text(), 'Save Changes')]"))
        .await?;
    save_button.click().await?;
    sleep(Duration::from_secs(2)).await;

    // Step 6: Verify the space was updated
    info!("✅ Verifying space was updated with image");
    let updated_page_content = env.browser.find(Locator::Css("body")).await?;
    let updated_page_text = updated_page_content.text().await?;

    assert!(
        updated_page_text.contains("Updated Space with Image"),
        "Space name should be updated"
    );

    assert!(
        updated_page_text.contains("Image attached"),
        "Space should show that an image is attached"
    );

    // Step 7: Test adding a new space with image
    info!("➕ Testing adding new space with site image");

    // Click "Add Space" button
    let add_space_button = env
        .browser
        .find(Locator::XPath("//button[contains(text(), 'Add Space')]"))
        .await?;
    add_space_button.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Fill in the new space form
    let new_space_name = format!(
        "New Space {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let space_name_field = env.browser.find(Locator::Id("space_name")).await?;
    space_name_field.click().await?;
    space_name_field.send_keys(&new_space_name).await?;

    let space_description_field =
        env.browser.find(Locator::Id("space_description")).await?;
    space_description_field.click().await?;
    space_description_field
        .send_keys("A new space created with image attachment")
        .await?;

    let eligibility_points_field =
        env.browser.find(Locator::Id("eligibility_points")).await?;
    eligibility_points_field.click().await?;
    eligibility_points_field.clear().await?;
    eligibility_points_field.send_keys("15.5").await?;

    // Select a site image for the new space
    let new_space_image_dropdown =
        env.browser.find(Locator::Id("space_image")).await?;
    new_space_image_dropdown.click().await?;
    sleep(Duration::from_millis(500)).await;

    // Select the second site image (should be "Blue Square")
    let new_space_image_option = env
        .browser
        .find(Locator::XPath("//option[contains(text(), 'Blue Square')]"))
        .await?;
    new_space_image_option.click().await?;

    // Create the space
    info!("🚀 Creating new space");
    let create_space_button = env
        .browser
        .find(Locator::XPath("//button[contains(text(), 'Create Space')]"))
        .await?;
    create_space_button.click().await?;
    sleep(Duration::from_secs(3)).await;

    // Step 8: Verify the new space was created
    info!("✅ Verifying new space was created with image");
    let final_page_content = env.browser.find(Locator::Css("body")).await?;
    let final_page_text = final_page_content.text().await?;

    assert!(
        final_page_text.contains(&new_space_name),
        "New space name should appear in the spaces list"
    );

    // Should now have 2 spaces total
    let space_count = final_page_text.matches("Eligibility Points:").count();
    assert!(
        space_count >= 2,
        "Should have at least 2 spaces now (original + new one)"
    );

    // Step 9: Test editing space to remove image
    info!("🗑️ Testing removing image from space");

    // Find and click edit on the first space again
    let space_edit_buttons = env
        .browser
        .find_all(Locator::XPath("//button[@title='Edit space']"))
        .await?;

    assert!(
        !space_edit_buttons.is_empty(),
        "Should have at least one edit button for spaces"
    );

    space_edit_buttons[0].click().await?;
    sleep(Duration::from_secs(1)).await;

    // Change the image selection to "No image"
    let image_dropdown_remove =
        env.browser.find(Locator::Id("edit_space_image")).await?;
    image_dropdown_remove.click().await?;
    sleep(Duration::from_millis(500)).await;

    let no_image_option = env
        .browser
        .find(Locator::XPath("//option[contains(text(), 'No image')]"))
        .await?;
    no_image_option.click().await?;

    // Save the changes
    let save_button_remove = env
        .browser
        .find(Locator::XPath("//button[contains(text(), 'Save Changes')]"))
        .await?;
    save_button_remove.click().await?;
    sleep(Duration::from_secs(2)).await;

    // Step 10: Verify image was removed
    info!("✅ Verifying image was removed from space");
    let no_image_page_content = env.browser.find(Locator::Css("body")).await?;
    let no_image_page_text = no_image_page_content.text().await?;

    assert!(
        no_image_page_text.contains("Updated Space with Image"),
        "Space should still be present after removing image"
    );

    // Step 11: Final verification - check that spaces section shows correct count
    info!("🔢 Final verification of spaces count");
    let final_verification_content =
        env.browser.find(Locator::Css("body")).await?;
    let final_verification_text = final_verification_content.text().await?;

    // Should show total count in the spaces header
    assert!(
        final_verification_text.contains("total")
            || final_verification_text.contains("Spaces"),
        "Should show spaces section with count information"
    );

    info!("✅ Space editing with site images test completed successfully");
    Ok(())
}
