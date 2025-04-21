use backend::{
    Config, startup,
    telemetry::{get_subscriber, init_subscriber},
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("info".into());
    init_subscriber(subscriber);

    let config = Config::from_env();
    startup(config).await
}
