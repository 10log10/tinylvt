pub mod email;
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
use secrecy::{ExposeSecret, SecretBox};
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
    build_with_email_service(config, time_source, None).await
}

/// Build the server with optional email service override (for testing)
pub async fn build_with_email_service(
    config: &mut Config,
    time_source: TimeSource,
    email_service_override: Option<web::Data<email::EmailService>>,
) -> std::io::Result<Server> {
    // Initialize session key from config or generate a temporary one
    let secret_key = match &config.session_master_key {
        Some(master_key) => {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            let decoded = STANDARD
                .decode(master_key.expose_secret())
                .expect("SESSION_MASTER_KEY must be valid base64");
            if decoded.len() != 64 {
                panic!(
                    "SESSION_MASTER_KEY must decode to exactly 64 bytes, got {} bytes",
                    decoded.len()
                );
            }
            Key::from(&decoded[..])
        }
        None => {
            tracing::warn!(
                "No SESSION_MASTER_KEY provided; using temporary key. \
                Sessions will not persist across restarts or between multiple instances. \
                Generate a key with: openssl rand -base64 64 | tr -d '\\n'"
            );
            Key::generate()
        }
    };
    let db_pool =
        web::Data::new(PgPool::connect(&config.database_url).await.unwrap());
    let time_source = web::Data::new(time_source);

    // Use override email service for testing, or create real one
    let email_service = match email_service_override {
        Some(service) => service,
        None => web::Data::new(email::EmailService::new(
            secrecy::SecretBox::new(Box::new(
                config.email_api_key.expose_secret().clone(),
            )),
            config.email_from_address.clone(),
        )),
    };

    // Clone config for use in closure
    let allowed_origins = config.allowed_origins.clone();
    let app_config = web::Data::new(AppConfig {
        base_url: config.base_url.clone(),
    });

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
            // Add security headers middleware before authentication
            .wrap(SecurityHeadersMiddleware)
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
            .app_data(email_service.clone())
            .app_data(app_config.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}

/// Configuration loaded from environment variables at startup.
/// Used only during server initialization, not shared as app_data.
pub struct Config {
    pub database_url: String,
    /// set to "0.0.0.0" for public access, "127.0.0.1" for local dev
    pub ip: String,
    /// set to 0 to get an os-assigned port
    pub port: u16,
    /// List of allowed CORS origins. Use "*" to allow any origin (development only)
    pub allowed_origins: Vec<String>,
    /// Email service API key (e.g., Resend API key)
    pub email_api_key: SecretBox<String>,
    /// Email from address
    pub email_from_address: String,
    /// Base URL for email links (e.g., "https://yourdomain.com" or "http://localhost:8080")
    pub base_url: String,
    /// Optional master key for session cookies (base64-encoded 64-byte key)
    /// If not provided, a random key will be generated on each startup
    pub session_master_key: Option<SecretBox<String>>,
}

/// Runtime configuration shared across the application as app_data.
/// Contains only the fields needed by route handlers at runtime.
pub struct AppConfig {
    /// Base URL for email links (e.g., "https://yourdomain.com" or "http://localhost:8080")
    pub base_url: String,
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
            email_api_key: SecretBox::new(Box::new(
                var("EMAIL_API_KEY").expect("EMAIL_API_KEY must be set"),
            )),
            email_from_address: var("EMAIL_FROM_ADDRESS")
                .expect("EMAIL_FROM_ADDRESS must be set"),
            base_url: var("BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            session_master_key: var("SESSION_MASTER_KEY")
                .ok()
                .map(|k| SecretBox::new(Box::new(k))),
        }
    }
}

/// Middleware to add security headers to API responses
use actix_web::{
    Error,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header::{CACHE_CONTROL, EXPIRES, HeaderValue, PRAGMA},
};
use std::{
    future::{Ready, ready},
    pin::Pin,
    rc::Rc,
};

type LocalBoxFuture<T> = Pin<Box<dyn std::future::Future<Output = T>>>;

pub struct SecurityHeadersMiddleware;

impl<S, B> Transform<S, ServiceRequest> for SecurityHeadersMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>
        + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = SecurityHeadersMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SecurityHeadersMiddlewareService {
            service: Rc::new(service),
        }))
    }
}

pub struct SecurityHeadersMiddlewareService<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for SecurityHeadersMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>
        + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();

        Box::pin(async move {
            let is_api_endpoint = req.path().starts_with("/api")
                && req.path() != "/api/health_check";

            let res = service.call(req).await?;

            if is_api_endpoint {
                let (req, mut res) = res.into_parts();

                // Add security headers for API endpoints
                res.headers_mut().insert(
                    CACHE_CONTROL,
                    HeaderValue::from_static(
                        "no-store, no-cache, must-revalidate, private",
                    ),
                );
                res.headers_mut()
                    .insert(PRAGMA, HeaderValue::from_static("no-cache"));
                res.headers_mut()
                    .insert(EXPIRES, HeaderValue::from_static("0"));

                Ok(ServiceResponse::new(req, res))
            } else {
                Ok(res)
            }
        })
    }
}
