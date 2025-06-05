pub mod password;
pub mod routes;
pub mod scheduler;
pub mod store;
pub mod telemetry;
pub mod time;

use actix_cors::Cors;
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

    // Clone config values for use in closure
    let allowed_origins = config.allowed_origins.clone();

    // OS assigns the port if binding to 0
    let listener = TcpListener::bind(format!("{}:{}", config.ip, config.port))?;
    config.port = listener.local_addr()?.port();
    let server = HttpServer::new(move || {
        // Configure CORS based on allowed origins
        let cors = if allowed_origins.contains(&"*".to_string()) {
            // Allow any origin (for development)
            Cors::default()
                .allow_any_origin()
                .allow_any_method()
                .allow_any_header()
                .supports_credentials()
        } else {
            // Production: Only allow specified origins
            let mut cors = Cors::default()
                .allow_any_method()
                .allow_any_header()
                .supports_credentials();
            
            for origin in &allowed_origins {
                cors = cors.allowed_origin(origin);
            }
            cors
        };

        App::new()
            .wrap(cors)
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
    /// List of allowed CORS origins. Use "*" to allow any origin (development only)
    pub allowed_origins: Vec<String>,
}

impl Config {
    pub fn from_env() -> Self {
        use std::env::var;
        
        let allowed_origins = var("ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "*".to_string()) // Default to allow any origin for development
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        
        Config {
            database_url: var("DATABASE_URL").unwrap(),
            ip: var("IP_ADDRESS").unwrap(),
            port: var("PORT").unwrap().parse().unwrap(),
            allowed_origins,
        }
    }
}
