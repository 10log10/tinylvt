//! Development server for TinyLVT UI development
//!
//! This binary creates a persistent API server with comprehensive test data
//! for frontend development. It uses mocked time initially to create realistic
//! auction progressions, then syncs with real time for browser compatibility.
//!
//! Usage: cargo run -p dev-server

use anyhow::Result;
use jiff::Timestamp;
use std::time::Duration;
use test_helpers::mock::DevDataset;
use tokio::time::interval;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = api::telemetry::get_subscriber("info".into());
    api::telemetry::init_subscriber(subscriber);

    info!("🚀 Starting TinyLVT development server");
    info!("⏰ Using MOCKED time initially, then syncing with real time");

    // Spawn the test app with mocked time
    let app = test_helpers::spawn_app().await;

    info!("✅ API server running on http://127.0.0.1:{}", app.port);

    // Set up comprehensive test data using mocked time for realistic progressions
    info!("📊 Setting up development test data...");
    let dataset = DevDataset::create(&app).await?;

    // Start background task to sync mocked time with real time every second
    info!("🕐 Starting real-time synchronization...");
    start_time_sync_task(&app);
    info!("✅ Time sync active - mock time will follow real time");

    info!("🎯 Development server ready!");
    info!("   API: http://127.0.0.1:{}", app.port);
    info!(
        "   UI:  cd ui && BACKEND_URL=http://127.0.0.1:{} trunk serve",
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
