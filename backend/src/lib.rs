pub mod password;
pub mod routes;
pub mod store;
pub mod telemetry;

use actix_identity::IdentityMiddleware;
use actix_session::{
    SessionMiddleware, config::BrowserSession, storage::CookieSessionStore,
};
use actix_web::cookie::{Key, time::Duration};
use actix_web::{App, HttpServer, web};
use sqlx::PgPool;

pub async fn startup(config: Config) -> std::io::Result<()> {
    let secret_key = Key::generate(); // key for signing session cookies
    let db_pool =
        web::Data::new(PgPool::connect(&config.database_url).await.unwrap());
    HttpServer::new(move || {
        App::new()
            // Use signed cookie to track user id
            // Redis would be better (can invalidate sessions; persists between
            // deployments), but this is ok for now
            .wrap(IdentityMiddleware::default())
            .wrap(
                SessionMiddleware::builder(
                    CookieSessionStore::default(),
                    secret_key.clone(),
                )
                .cookie_name("tinylvt".into())
                .session_lifecycle(
                    BrowserSession::default().state_ttl(Duration::days(30)),
                )
                .build(),
            )
            .service(web::scope("/api").service(routes::api_routes()))
            // static files service
            .service(
                actix_files::Files::new("/", "../ui/dist/")
                    .index_file("index.html"),
            )
            .app_data(db_pool.clone())
    })
    .bind((config.ip, 8081))?
    .run()
    .await
}

pub struct Config {
    database_url: String,
    ip: String, // set to "0.0.0.0" for public access, "127.0.0.1" for local dev
}

impl Config {
    pub fn from_env() -> Self {
        Config {
            database_url: std::env::var("DATABASE_URL").unwrap(),
            ip: std::env::var("IP_ADDRESS").unwrap(),
        }
    }
}
