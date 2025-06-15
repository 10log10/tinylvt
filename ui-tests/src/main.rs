//! This crate uses standard Rust tests with #[tokio::test]
//! Run with: cargo test
//!
//! For human-in-the-loop debugging, we have a main() function
//! that sets up comprehensive test data and opens a headed browser for manual inspection.

#![allow(unused)]

use anyhow::Result;
use fantoccini::Locator;
use payloads::requests;
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

    info!("🚀 Starting comprehensive UI test environment");

    // Set up the test environment (API server, frontend, browser) in headed mode
    info!("🔧 Setting up test environment with headed browser");
    let env = TestEnvironment::setup_headed().await?;

    // === SET UP COMPREHENSIVE TEST DATA ===
    info!("📊 Setting up comprehensive test data");

    // Step 1: Create Alice user and her community
    info!("👤 Creating Alice user");
    env.api.create_alice_user().await?;
    info!("🏘️ Creating Alice's community (Alice as leader)");
    let alice_community_id = env.api.create_test_community().await?;
    info!("✅ Alice's community created with ID: {}", alice_community_id.0);

    // Step 2: Create Bob user and his community
    info!("👤 Creating Bob user");
    env.api.create_bob_user().await?;
    info!("🔑 Logging in as Bob");
    env.api.login_bob().await?;
    
    info!("🏘️ Creating Bob's community (Bob as leader)");
    let bob_community_body = requests::CreateCommunity {
        name: "Bob's Community".into(),
        new_members_default_active: true,
    };
    let bob_community_id = env.api.client.create_community(&bob_community_body).await?;
    info!("✅ Bob's community created with ID: {}", bob_community_id.0);

    // Step 3: Create invite from Bob to Alice
    info!("📧 Creating invite from Bob to Alice");
    let alice_credentials = test_helpers::alice_credentials();
    let invite_details = requests::InviteCommunityMember {
        community_id: bob_community_id,
        new_member_email: Some(alice_credentials.email.clone()),
    };
    let invite_id = env.api.client.invite_member(&invite_details).await?;
    info!("✅ Invite created with ID: {} for Alice to join Bob's community", invite_id.0);

    // Step 4: Log back in as Alice for the UI session
    info!("🔑 Logging back in as Alice for UI session");
    env.api.login_alice().await?;

    // === START UI SESSION AS ALICE ===
    info!("🌐 Starting UI session as Alice");

    // Use the login helper function
    let alice_login_creds = test_helpers::alice_login_credentials();
    framework::login_user(&env.browser, &env.frontend_url, &alice_login_creds).await?;

    // === DISPLAY TEST DATA SUMMARY ===
    info!("📋 Test Environment Summary:");
    info!("   🏘️ Alice's Community (ID: {}) - Alice is LEADER", alice_community_id.0);
    info!("   🏘️ Bob's Community (ID: {}) - Bob is LEADER", bob_community_id.0);
    info!("   📧 Pending invite (ID: {}) - Alice invited to Bob's community", invite_id.0);
    info!("   👤 Logged in as: Alice (can see her community + pending invite)");
    info!("");
    info!("🎯 You can now test:");
    info!("   • Communities list (Alice should see her own community)");
    info!("   • Community invites (Alice should see Bob's invite)");
    info!("   • Community creation, joining, etc.");
    info!("");
    let current_url = env.browser.current_url().await?;
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
