//! Development server for TinyLVT UI development
//!
//! This binary creates a persistent API server with comprehensive test data
//! for frontend development. It uses mocked time initially to create realistic
//! auction progressions, then syncs with real time for browser compatibility.
//!
//! Usage: cargo run -p dev-server

use anyhow::Result;
use api::scheduler::Scheduler;
use jiff::Timestamp;
use std::time::Duration;
use test_helpers::mock::DeskAllocationScreenshot;
use tokio::time::interval;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file if available
    let _ = dotenvy::dotenv();

    // Initialize logging
    let subscriber = api::telemetry::get_subscriber("info".into());
    api::telemetry::init_subscriber(subscriber);

    info!("🚀 Starting TinyLVT development server");
    info!("⏰ Using MOCKED time initially, then syncing with real time");

    // Spawn the test app with mocked time on port 8000 for development
    let app = test_helpers::spawn_app_on_port(8000).await;

    info!("✅ API server running on http://127.0.0.1:{}", app.port);

    // Set up desk allocation screenshot data
    info!("📊 Setting up desk allocation screenshot data...");
    let dataset = DeskAllocationScreenshot::create(&app).await?;
    // dataset.activate_subscription(&app).await?;

    // Start scheduler to process auction rounds and proxy bidding
    info!("⏲️  Starting auction scheduler...");
    start_scheduler(&app);
    info!("✅ Scheduler active - auctions will progress automatically");

    // Start background task to sync mocked time with real time every second
    info!("🕐 Starting real-time synchronization...");
    start_time_sync_task(&app);
    info!("✅ Time sync active - mock time will follow real time");

    info!("🎯 Development server ready!");
    info!("   API: http://127.0.0.1:{}", app.port);
    info!(
        "   UI:  cd ui && BACKEND_URL=http://127.0.0.1:{} TRUNK_WATCH_ENABLE_COOLDOWN=true trunk serve",
        app.port
    );
    info!("");
    dataset.print_summary();
    info!("");
    info!("👋 Press Ctrl+C to shutdown");

    // Keep server running until Ctrl+C
    tokio::signal::ctrl_c().await?;
    info!("🛑 Shutting down development server");
    Ok(())
}

/// Starts the auction scheduler in the background to process auction
/// rounds and proxy bidding.
fn start_scheduler(app: &test_helpers::TestApp) {
    let scheduler = Scheduler::new(
        app.db_pool.clone(),
        app.time_source.clone(),
        Duration::from_secs(1), // Tick every second for development
    );

    tokio::spawn(async move {
        info!("⏲️  Scheduler task started - processing auctions");
        scheduler.run().await;
    });
}

/// Starts a background task that continuously syncs the mocked time source
/// with real time, ensuring browser compatibility while maintaining the rich
/// test data created with mocked time progressions.
fn start_time_sync_task(app: &test_helpers::TestApp) {
    let time_source = app.time_source.clone();

    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(1));
        info!("⏱️ Time sync task started - updating every second");

        loop {
            interval.tick().await;
            let real_now = Timestamp::now();
            time_source.set(real_now);

            // Log occasionally to show sync is working (every 30 seconds)
            if real_now.as_second() % 30 == 0 {
                tracing::debug!(
                    "🕐 Synced mock time to real time: {}",
                    real_now
                );
            }
        }
    });
}
