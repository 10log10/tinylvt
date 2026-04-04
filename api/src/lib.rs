pub mod email;
pub mod password;
pub mod routes;
pub mod scheduler;
pub mod store;
pub mod stripe_service;
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
    db_pool: PgPool,
    time_source: TimeSource,
    stripe_service: std::sync::Arc<stripe_service::StripeService>,
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
    let db_pool = web::Data::new(db_pool);
    let time_source = web::Data::new(time_source);

    let email_service = web::Data::new(email::EmailService::new(
        secrecy::SecretBox::new(Box::new(
            config.email_api_key.expose_secret().clone(),
        )),
        config.email_from_address.clone(),
    ));

    let stripe_service = web::Data::from(stripe_service);

    // Clone config for use in closure
    let allowed_origins = config.allowed_origins.clone();
    let app_config = web::Data::new(AppConfig {
        base_url: config.base_url.clone(),
        stripe_monthly_price_id: config.stripe_monthly_price_id.clone(),
        stripe_annual_price_id: config.stripe_annual_price_id.clone(),
    });

    // OS assigns the port if binding to 0
    let listener = TcpListener::bind(format!("{}:{}", config.ip, config.port))?;
    config.port = listener.local_addr()?.port();
    let server = HttpServer::new(move || {
        // Configure CORS with explicitly allowed origins
        let mut cors = Cors::default()
            .allow_any_method()
            .allow_any_header()
            .supports_credentials();

        for origin in &allowed_origins {
            cors = cors.allowed_origin(origin);
        }

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
            .app_data(
                web::JsonConfig::default()
                    // 1 MB image as JSON-serialized Vec<u8>
                    // expands ~4-5x (each byte becomes 1-3
                    // digits + comma)
                    .limit(6 * 1024 * 1024),
            )
            .app_data(time_source.clone())
            .app_data(email_service.clone())
            .app_data(stripe_service.clone())
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
    /// Stripe secret API key
    pub stripe_api_key: SecretBox<String>,
    /// Stripe webhook endpoint secret
    pub stripe_webhook_secret: SecretBox<String>,
    /// Stripe Price ID for the monthly plan
    pub stripe_monthly_price_id: String,
    /// Stripe Price ID for the annual plan
    pub stripe_annual_price_id: String,
}

/// Runtime configuration shared across the application as app_data.
/// Contains only the fields needed by route handlers at runtime.
pub struct AppConfig {
    /// Base URL for links (e.g., "https://yourdomain.com")
    pub base_url: String,
    /// Stripe Price ID for the monthly plan
    pub stripe_monthly_price_id: String,
    /// Stripe Price ID for the annual plan
    pub stripe_annual_price_id: String,
}

impl Config {
    pub fn create_stripe_service(
        &self,
    ) -> std::sync::Arc<stripe_service::StripeService> {
        std::sync::Arc::new(stripe_service::StripeService::new(
            SecretBox::new(Box::new(
                self.stripe_api_key.expose_secret().clone(),
            )),
            SecretBox::new(Box::new(
                self.stripe_webhook_secret.expose_secret().clone(),
            )),
        ))
    }

    pub fn from_env() -> Self {
        use std::env::var;

        let allowed_origins = var("ALLOWED_ORIGINS")
            .expect("ALLOWED_ORIGINS must be specified")
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
            stripe_api_key: SecretBox::new(Box::new(
                var("STRIPE_API_KEY").expect("STRIPE_API_KEY must be set"),
            )),
            stripe_webhook_secret: SecretBox::new(Box::new(
                var("STRIPE_WEBHOOK_SECRET")
                    .expect("STRIPE_WEBHOOK_SECRET must be set"),
            )),
            stripe_monthly_price_id: var("STRIPE_MONTHLY_PRICE_ID")
                .expect("STRIPE_MONTHLY_PRICE_ID must be set"),
            stripe_annual_price_id: var("STRIPE_ANNUAL_PRICE_ID")
                .expect("STRIPE_ANNUAL_PRICE_ID must be set"),
        }
    }
}

/// Middleware to add security headers to API responses
use actix_web::{
    Error,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header::{
        CACHE_CONTROL, EXPIRES, HeaderValue, PRAGMA, X_CONTENT_TYPE_OPTIONS,
    },
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
                && req.path() != "/api/health_check"
                && req.path() != "/api/platform_stats";

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
                res.headers_mut().insert(
                    X_CONTENT_TYPE_OPTIONS,
                    HeaderValue::from_static("nosniff"),
                );

                Ok(ServiceResponse::new(req, res))
            } else {
                Ok(res)
            }
        })
    }
}
