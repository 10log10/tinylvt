use std::time::Duration;

use api::{
    Config, build,
    scheduler::Scheduler,
    telemetry::{get_subscriber, init_subscriber},
    time::TimeSource,
};

/// TinyLVT API Server
///
/// Required environment variables:
/// - DATABASE_URL: PostgreSQL connection string
/// - IP_ADDRESS: Server bind address (127.0.0.1 for local, 0.0.0.0 for public)
/// - PORT: Server port
/// - ALLOWED_ORIGINS: CORS origins ("*" for any origin in development, or comma-separated list for production)
///
/// Example development command:
/// DATABASE_URL=postgresql://user:password@localhost:5432/tinylvt \
/// IP_ADDRESS=127.0.0.1 PORT=8000 ALLOWED_ORIGINS=* \
/// cargo run
///
/// Example production command:
/// DATABASE_URL=postgresql://user:password@localhost:5432/tinylvt \
/// IP_ADDRESS=0.0.0.0 PORT=8000 ALLOWED_ORIGINS=https://app.tinylvt.com,https://tinylvt.com \
/// cargo run
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("info".into());
    init_subscriber(subscriber);

    let mut config = Config::from_env();

    let pool = sqlx::PgPool::connect(&config.database_url).await.unwrap();

    // Create time source
    #[cfg(not(feature = "test-utils"))]
    let time_source = TimeSource::new();
    #[cfg(feature = "test-utils")]
    let time_source = TimeSource::new(jiff::Timestamp::now());

    // Start the scheduler service
    let scheduler = Scheduler::new(
        pool.clone(),
        time_source.clone(),
        Duration::from_secs(5),
    );
    tokio::spawn(async move {
        scheduler.run().await;
    });

    let server = build(&mut config, time_source).await?;
    server.await
}
