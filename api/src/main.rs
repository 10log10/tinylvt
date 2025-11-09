use std::time::Duration;

use api::{
    Config, build,
    scheduler::Scheduler,
    telemetry::{get_subscriber, init_subscriber},
    time::TimeSource,
};

/// TinyLVT API Server
///
/// Environment variables can be set directly or loaded from a .env file in the project root.
///
/// Required environment variables:
/// - DATABASE_URL: PostgreSQL connection string
/// - IP_ADDRESS: Server bind address (127.0.0.1 for local, 0.0.0.0 for public)
/// - PORT: Server port
/// - ALLOWED_ORIGINS: CORS origins ("*" for any origin in development, or comma-separated list for production)
/// - EMAIL_API_KEY: API key for email service (e.g., Resend)
/// - EMAIL_FROM_ADDRESS: From address for outgoing emails
/// - BASE_URL: Base URL for email links (optional, defaults to http://localhost:8080)
///
/// Example .env file:
/// DATABASE_URL=postgresql://user:password@localhost:5432/tinylvt
/// IP_ADDRESS=127.0.0.1
/// PORT=8000
/// ALLOWED_ORIGINS=*
/// EMAIL_API_KEY=your_api_key
/// EMAIL_FROM_ADDRESS=noreply@yourdomain.com
/// BASE_URL=http://localhost:8080
///
/// Example development command:
/// cargo run
///
/// Or with direct environment variables:
/// DATABASE_URL=postgresql://user:password@localhost:5432/tinylvt \
/// IP_ADDRESS=127.0.0.1 PORT=8000 ALLOWED_ORIGINS=* \
/// EMAIL_API_KEY=your_key EMAIL_FROM_ADDRESS=noreply@example.com \
/// cargo run
///
/// Example production command:
/// DATABASE_URL=postgresql://user:password@localhost:5432/tinylvt \
/// IP_ADDRESS=0.0.0.0 PORT=8000 ALLOWED_ORIGINS=https://app.tinylvt.com,https://tinylvt.com \
/// cargo run
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load environment variables from .env file if available
    // This will silently ignore if the file doesn't exist
    let _ = dotenvy::dotenv();

    let subscriber = get_subscriber("info".into());
    init_subscriber(subscriber);

    let mut config = Config::from_env();

    let pool = sqlx::PgPool::connect(&config.database_url).await.unwrap();

    // Run database migrations embedded in the binary
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    // Create time source
    #[cfg(not(feature = "mock-time"))]
    let time_source = TimeSource::new();
    #[cfg(feature = "mock-time")]
    let time_source = TimeSource::new(jiff::Timestamp::now());

    // Start the scheduler service
    let scheduler = Scheduler::new(
        pool.clone(),
        time_source.clone(),
        Duration::from_secs(1),
    );
    tokio::spawn(async move {
        scheduler.run().await;
    });

    let server = build(&mut config, time_source).await?;
    server.await
}
