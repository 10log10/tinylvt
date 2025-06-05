pub mod password;
pub mod routes;
pub mod scheduler;
pub mod store;
pub mod telemetry;
pub mod time;

use actix_identity::IdentityMiddleware;
use actix_session::{
    SessionMiddleware, config::BrowserSession, storage::CookieSessionStore,
};
use actix_web::cookie::{Key, time::Duration};
use actix_web::dev::Server;
use actix_web::{App, HttpServer, web};
use sqlx::PgPool;
use std::net::TcpListener;

use crate::time::TimeSource;

/// Build the server, but not await it.
///
/// Returns the port that the server has bound to by modifying the config.
pub async fn build(
    config: &mut Config,
    time_source: TimeSource,
) -> std::io::Result<Server> {
    let secret_key = Key::generate(); // key for signing session cookies
    let db_pool =
        web::Data::new(PgPool::connect(&config.database_url).await.unwrap());
    let time_source = web::Data::new(time_source);

    // OS assigns the port if binding to 0
    let listener = TcpListener::bind(format!("{}:{}", config.ip, config.port))?;
    config.port = listener.local_addr()?.port();
    let server = HttpServer::new(move || {
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
            .service(routes::api_services())
            // static files service
            .service(
                actix_files::Files::new("/", "../ui/dist/")
                    .index_file("index.html"),
            )
            .app_data(db_pool.clone())
            .app_data(time_source.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}

pub struct Config {
    pub database_url: String,
    /// set to "0.0.0.0" for public access, "127.0.0.1" for local dev
    pub ip: String,
    /// set to 0 to get an os-assigned port
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Self {
        use std::env::var;
        Config {
            database_url: var("DATABASE_URL").unwrap(),
            ip: var("IP_ADDRESS").unwrap(),
            port: var("PORT").unwrap().parse().unwrap(),
        }
    }
}
