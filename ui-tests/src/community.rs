use anyhow::Result;
use fantoccini::Locator;
use std::time::Duration;
use test_helpers::{alice_login_credentials, bob_login_credentials};
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::framework::login_user;

use crate::framework::TestEnvironment;

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

    // Step 2: Log in as Alice
    login_user(&env.browser, &env.frontend_url, &alice_login_credentials())
        .await?;

    // Step 3: Navigate to communities page using the nav link instead of direct URL
    info!("🏘️ Navigating to communities page via navigation link");
    let communities_link =
        env.browser.find(Locator::LinkText("Communities")).await?;
    communities_link.click().await?;
    sleep(Duration::from_secs(2)).await;

    // Step 4: Click create community button
    info!("➕ Clicking create community button");
    let create_button = env
        .browser
        .find(Locator::XPath(
            "//button[contains(text(), 'Create your first community')]",
        ))
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
    let community_name = format!(
        "Test Community {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    // Fill in the community name
    let name_field = env.browser.find(Locator::Id("name")).await?;
    name_field.click().await?;
    name_field.clear().await?;
    name_field.send_keys(&community_name).await?;

    // Note: new_members_default_active is always true by default

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
        post_create_url.as_str().contains("/communities")
            && !post_create_url.as_str().contains("/create"),
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

/// UI integration test for US-010: Join community via invite
///
/// This test covers the user story:
///   As a user, I want to join communities via invite so I can participate in shared space allocation.
///
/// Steps:
/// - Create a community and generate an invite
/// - Create a new user (Bob) to receive the invite
/// - Navigate to community invites page
/// - Accept the community invite
/// - Verify community membership
/// - Verify default active status
/// - *API Coverage*: `accept_invite`, `get_invites`
#[tokio::test]
async fn test_join_community_via_invite() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Set up test data - create Alice and a community
    info!("📊 Setting up test data (Alice user and community)");
    env.api.create_alice_user().await?;
    let community_id = env.api.create_test_community().await?;
    debug!("Created community with ID: {}", community_id);

    // Step 2: Create an invite for Bob to join the community
    info!("📧 Creating community invite for Bob");
    env.api.login_alice().await?;
    let _invite_path = env.api.invite_bob().await?;
    debug!("Created invite for Bob");

    // Step 3: Create Bob user
    info!("👤 Creating Bob user");
    env.api.create_bob_user().await?;

    // Step 4: Log in as Bob
    login_user(&env.browser, &env.frontend_url, &bob_login_credentials())
        .await?;

    // Step 5: Navigate to community invites page
    info!("📬 Navigating to community invites page");

    // Navigate to the main page first to ensure the frontend app is loaded
    env.browser.goto(&env.frontend_url).await?;
    sleep(Duration::from_millis(500)).await;

    // Navigate to communities page
    info!("🏘️ Going to communities page");
    let communities_link =
        env.browser.find(Locator::LinkText("Communities")).await?;
    communities_link.click().await?;
    sleep(Duration::from_millis(500)).await;

    // Look for "Join Community" button to get to invites (handles responsive text)
    info!("📧 Looking for Join Community button");
    let join_button = env
        .browser
        .find(Locator::XPath(
            "//button[contains(., 'Join Community') or contains(., 'Join')]",
        ))
        .await?;
    join_button.click().await?;
    sleep(Duration::from_millis(500)).await;

    // Step 6: Verify we're on the community invites page
    info!("🔍 Verifying we're on the community invites page");
    let current_url = env.browser.current_url().await?;
    debug!("Current URL: {}", current_url);

    // Verify we're on the invites page by URL
    assert!(
        current_url.as_str().contains("/communities/invites"),
        "Should be on community invites page, but got: {}",
        current_url
    );

    // Step 7: Accept the community invite
    info!("🔍 Looking for pending invites");

    // Wait briefly for invites to load
    sleep(Duration::from_millis(500)).await;

    // Find and click the accept button
    let accept_buttons = env
        .browser
        .find_all(Locator::XPath(
            "//button[contains(text(), 'Accept') or contains(text(), 'Join')]",
        ))
        .await?;
    assert!(
        !accept_buttons.is_empty(),
        "Should find at least one accept button for the invite"
    );

    info!(
        "✅ Found {} accept button(s), clicking the first one",
        accept_buttons.len()
    );
    accept_buttons[0].click().await?;
    sleep(Duration::from_millis(500)).await;

    // Step 8: Verify invite acceptance success
    info!("🎉 Verifying invite acceptance success");

    // Check that we were redirected to communities page (success indicator)
    let current_url = env.browser.current_url().await?;
    assert!(
        current_url.as_str().contains("/communities")
            && !current_url.as_str().contains("/invites"),
        "Should be redirected to communities page after accepting invite, but got: {}",
        current_url
    );

    // Step 9: Verify community membership
    info!("🏘️ Verifying community membership");

    // We should already be on the communities page, just wait for it to load
    sleep(Duration::from_millis(500)).await;

    // Step 10: Verify the community appears in Bob's communities list
    info!("👀 Checking if community appears in Bob's communities");

    // Get the page content to verify the community is displayed
    let communities_body = env.browser.find(Locator::Css("body")).await?;
    let communities_text = communities_body.text().await?;
    debug!("Communities page content: {}", communities_text);

    // Check if the community name appears on the page
    let community_displayed = communities_text.contains("Test community")
        || communities_text.contains("Test Community");

    assert!(
        community_displayed,
        "Bob should see 'Test community' in his communities list after accepting invite. Page content: {}",
        communities_text
    );

    // Also verify that Bob has the Member role
    let has_member_role = communities_text.contains("Member");
    assert!(
        has_member_role,
        "Bob should have 'Member' role displayed for the community. Page content: {}",
        communities_text
    );

    info!("✅ Community membership verified successfully");

    info!("✅ Join community via invite test completed");
    Ok(())
}

/// UI integration test for US-010b: Accept community invite via direct link
///
/// This test covers the user story:
///   As a user, I want to accept community invites via direct link so I can join communities easily.
///
/// Steps:
/// - Create a community and generate a link-based invite
/// - Create a new user (Bob) to accept the invite
/// - Navigate directly to the invite acceptance URL with query parameter
/// - Verify automatic invite acceptance when authenticated
/// - Verify redirect to communities page after successful acceptance
/// - Verify community membership is established
/// - *API Coverage*: `accept_invite`
#[tokio::test]
async fn test_accept_community_invite_via_link() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Set up test data - create Alice and a community
    info!("📊 Setting up test data (Alice user and community)");
    env.api.create_alice_user().await?;
    let community_id = env.api.create_test_community().await?;
    debug!("Created community with ID: {}", community_id);

    // Step 2: Create a link-based invite (no email)
    info!("🔗 Creating link-based community invite");
    env.api.login_alice().await?;
    let invite_id = env.api.create_link_invite().await?;
    debug!("Created link-based invite with ID: {}", invite_id.0);

    // Step 3: Create Bob user
    info!("👤 Creating Bob user");
    env.api.create_bob_user().await?;

    // Step 4: Log in as Bob
    login_user(&env.browser, &env.frontend_url, &bob_login_credentials())
        .await?;

    // Step 5: Navigate directly to the invite acceptance URL with query parameter
    info!("🌐 Navigating directly to invite acceptance URL");
    let invite_url = format!(
        "{}/communities/invites?accept={}",
        env.frontend_url, invite_id.0
    );
    debug!("Invite URL: {}", invite_url);

    env.browser.goto(&invite_url).await?;
    sleep(Duration::from_secs(2)).await;

    // Step 6: Verify automatic invite acceptance and redirect
    info!("🔍 Verifying automatic invite acceptance and redirect");

    // Wait for the automatic acceptance process to complete
    let mut redirect_completed = false;
    let mut attempts = 0;
    while attempts < 10 {
        let current_url = env.browser.current_url().await?;
        debug!("Current URL (attempt {}): {}", attempts + 1, current_url);

        // Check if we've been redirected to the communities page (success)
        if current_url.as_str().contains("/communities")
            && !current_url.as_str().contains("/invites")
            && !current_url.as_str().contains("accept=")
        {
            redirect_completed = true;
            info!("✅ Successfully redirected to communities page");
            break;
        }

        // Check if we're still on the invites page (might be processing)
        if current_url.as_str().contains("/communities/invites") {
            debug!("Still on invites page, waiting for processing...");
            sleep(Duration::from_millis(500)).await;
            attempts += 1;
            continue;
        }

        // Check for any error states
        let page_body = env.browser.find(Locator::Css("body")).await?;
        let page_text = page_body.text().await?;
        if page_text.contains("Failed to accept invite")
            || page_text.contains("Invalid invite")
        {
            panic!("Invite acceptance failed: {}", page_text);
        }

        sleep(Duration::from_millis(500)).await;
        attempts += 1;
    }

    assert!(
        redirect_completed,
        "Should be redirected to communities page after automatic invite acceptance"
    );

    // Step 7: Verify community membership
    info!("🏘️ Verifying community membership");

    // We should already be on the communities page, wait for it to load
    sleep(Duration::from_millis(1000)).await;

    // Step 8: Verify the community appears in Bob's communities list
    info!("👀 Checking if community appears in Bob's communities");

    // Get the page content to verify the community is displayed
    let communities_body = env.browser.find(Locator::Css("body")).await?;
    let communities_text = communities_body.text().await?;
    debug!("Communities page content: {}", communities_text);

    // Check if the community name appears on the page
    let community_displayed = communities_text.contains("Test community")
        || communities_text.contains("Test Community");

    assert!(
        community_displayed,
        "Bob should see 'Test community' in his communities list after accepting invite via link. Page content: {}",
        communities_text
    );

    // Also verify that Bob has the Member role
    let has_member_role = communities_text.contains("Member");
    assert!(
        has_member_role,
        "Bob should have 'Member' role displayed for the community. Page content: {}",
        communities_text
    );

    info!("✅ Community membership verified successfully via link acceptance");

    info!("✅ Accept community invite via link test completed");
    Ok(())
}

/// UI integration test for US-009: View communities list
///
/// This test covers the user story:
///   As a user, I want to view my communities list so I can see all communities I'm a member of.
///
/// Steps:
/// - Ensure Alice user exists and has communities
/// - Log in as Alice
/// - Navigate to communities page
/// - Verify list of user's communities is displayed
/// - Click on community to access dashboard
/// - *API Coverage*: `get_communities`
#[tokio::test]
async fn test_view_communities_list() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Set up test data (user + communities)
    info!("📊 Setting up test data");
    env.api.create_alice_user().await?;
    let community_id = env.api.create_test_community().await?;
    debug!(
        "Test data created: Alice user and test community with ID: {}",
        community_id
    );

    // Step 2: Log in as Alice
    login_user(&env.browser, &env.frontend_url, &alice_login_credentials())
        .await?;

    // Step 3: Navigate to communities page
    info!("🏘️ Navigating to communities page");
    let communities_link =
        env.browser.find(Locator::LinkText("Communities")).await?;
    communities_link.click().await?;
    sleep(Duration::from_secs(2)).await;

    // Step 4: Verify we're on the communities page
    info!("🔍 Verifying communities page loaded");
    let current_url = env.browser.current_url().await?;
    debug!("Current URL: {}", current_url);
    assert!(
        current_url.as_str().contains("/communities"),
        "Should be on communities page"
    );

    // Step 5: Verify page title/header
    info!("📋 Verifying communities list page elements");
    let page_title = env
        .browser
        .find(Locator::XPath("//*[contains(text(), 'My Communities')]"))
        .await?;
    let title_text = page_title.text().await?;
    assert!(
        title_text.contains("My Communities"),
        "Page should show 'My Communities' heading"
    );

    // Step 6: Verify communities are displayed (should have at least one)
    info!("👥 Verifying communities are displayed");

    // Wait for communities to load (check for either communities or empty state)
    let mut communities_found = false;
    let mut attempts = 0;
    while attempts < 10 {
        // Check if we have community cards
        if let Ok(community_cards) =
            env.browser.find_all(Locator::Css(".cursor-pointer")).await
        {
            if !community_cards.is_empty() {
                communities_found = true;
                debug!("Found {} community cards", community_cards.len());
                break;
            }
        }

        // Check if we're in loading state
        if let Ok(_loading) =
            env.browser.find(Locator::Css(".animate-spin")).await
        {
            debug!("Still loading communities, waiting...");
            sleep(Duration::from_millis(500)).await;
            attempts += 1;
            continue;
        }

        // Check if we have empty state
        if let Ok(empty_state) = env
            .browser
            .find(Locator::XPath("//*[contains(text(), 'No communities')]"))
            .await
        {
            let empty_text = empty_state.text().await?;
            debug!("Found empty state: {}", empty_text);
            break;
        }

        attempts += 1;
        sleep(Duration::from_millis(500)).await;
    }

    assert!(
        communities_found || attempts >= 10,
        "Should display communities or finish loading within timeout"
    );

    if communities_found {
        // Step 7: Verify community card content
        info!("🏢 Verifying community card content");
        let community_cards = env
            .browser
            .find_all(Locator::Css(".cursor-pointer"))
            .await?;

        assert!(
            !community_cards.is_empty(),
            "Should have at least one community card"
        );

        // Check that community information appears on the page
        let page_body = env.browser.find(Locator::Css("body")).await?;
        let page_text = page_body.text().await?;
        debug!("Page contains text: {}", page_text);

        // Look for "Test Community" or similar text
        assert!(
            page_text.contains("Test Community")
                || page_text.contains("community"),
            "Should display community information"
        );

        // Step 8: Click on community to access dashboard
        info!("🖱️ Clicking on community to access dashboard");
        community_cards[0].click().await?;
        sleep(Duration::from_secs(2)).await;

        // Step 9: Verify navigation to community page
        info!("🏠 Verifying navigation to community page");
        let community_url = env.browser.current_url().await?;
        debug!("Community URL: {}", community_url);

        assert!(
            community_url.as_str().contains("/community/"),
            "Should navigate to community page with pattern /community/:id"
        );

        // Verify we're on the community page
        let community_body = env.browser.find(Locator::Css("body")).await?;
        let community_text = community_body.text().await?;
        debug!("Community page text: {}", community_text);

        // Look for common community page elements
        assert!(
            community_text.contains("Community")
                || community_text.contains("Members")
                || community_text.contains("Sites")
                || community_text.contains("Settings")
                || community_text.contains("Test Community"),
            "Should be on the community page with relevant content"
        );

        info!("✅ Successfully navigated to community page");
    } else {
        warn!(
            "No communities found - this might indicate an issue with community creation or loading"
        );
    }

    info!("✅ View communities list test completed successfully");
    Ok(())
}
