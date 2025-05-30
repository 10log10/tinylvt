use std::time::Duration;

use backend::{
    Config, build,
    scheduler::Scheduler,
    telemetry::{get_subscriber, init_subscriber},
    time::TimeSource,
};

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
