use backend::{
    Config, build,
    telemetry::{get_subscriber, init_subscriber},
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("info".into());
    init_subscriber(subscriber);

    let mut config = Config::from_env();
    let server = build(&mut config).await?;
    server.await
}
