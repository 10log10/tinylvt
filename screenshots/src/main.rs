//! Screenshot automation for TinyLVT documentation.
//!
//! This tool sets up mock datasets and captures screenshots in both light
//! and dark modes for use in documentation and marketing materials.
//!
//! ## Prerequisites
//!
//! - geckodriver must be installed and available in PATH
//! - Firefox must be installed
//!
//! ## Usage
//!
//! ```shell
//! # Run from project root
//! cargo run -p screenshots
//!
//! # With debug output
//! RUST_LOG=screenshots=debug,info cargo run -p screenshots
//! ```

use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

mod framework;

use framework::{ScreenshotEnvironment, login_user};

fn output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("output")
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let subscriber = api::telemetry::get_subscriber("info".into());
    api::telemetry::init_subscriber(subscriber);

    info!("Starting screenshot automation");

    // Create output directory
    let output_dir = output_dir();
    fs::create_dir_all(&output_dir)?;

    // Set up environment with a reasonable width for documentation
    // 800 px width is just above the 768 `md:` break point
    let env = ScreenshotEnvironment::setup_with_size(800, 800).await?;
    // let env = ScreenshotEnvironment::setup_headed().await?;

    // Set up the desk allocation mock dataset
    info!("Setting up desk allocation mock dataset");
    let dataset =
        test_helpers::mock::DeskAllocationScreenshot::create(&env.api).await?;

    info!("Dataset created:");
    info!("  Community: {}", dataset.community_id.0);
    info!("  Site: {}", dataset.site.site_id.0);
    info!("  Auction: {}", dataset.auction.auction_id.0);

    // Login as Bob (has interesting bids across multiple desks)
    let bob_creds = test_helpers::bob_login_credentials();
    login_user(&env, &bob_creds).await?;

    // Wait a moment for any redirects to complete
    sleep(Duration::from_millis(500)).await;

    // Navigate to the auction detail page
    let auction_path = format!("/auctions/{}", dataset.auction.auction_id.0);
    info!("Navigating to auction page: {}", auction_path);
    env.goto(&auction_path).await?;

    // Wait for the page to fully load
    sleep(Duration::from_millis(1000)).await;

    // Take screenshots
    info!("Taking auction detail screenshots");
    env.screenshot_both_modes(&output_dir, "auction-detail")
        .await?;

    info!(
        "Screenshots complete! Output saved to {}",
        output_dir.display()
    );
    info!("Files created:");
    for entry in fs::read_dir(&output_dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        info!(
            "  {} ({} bytes)",
            entry.file_name().to_string_lossy(),
            metadata.len()
        );
    }

    Ok(())
}
