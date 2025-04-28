pub mod community;
pub mod login;

use actix_identity::{Identity, error::GetIdentityError};
use actix_web::{
    HttpResponse, Responder, ResponseError, body::BoxBody,
    dev::HttpServiceFactory, web,
};
use uuid::Uuid;

use crate::store;

pub fn api_services() -> impl HttpServiceFactory {
    web::scope("/api")
        .route("/health_check", web::get().to(health_check))
        .route("/login", web::post().to(login::login))
        .route("/login_check", web::post().to(login::login_check))
        .route("/logout", web::post().to(login::logout))
        .route("/create_account", web::post().to(login::create_account))
        .route(
            "/create_community",
            web::post().to(community::create_community),
        )
}

pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("healthy")
}

/// Public login errors. Only the top-level message is sent.
#[derive(Debug, thiserror::Error)]
pub enum APIError {
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error("Invalid login session")]
    GetIdentityError(#[source] GetIdentityError),
    #[error("Bad request")]
    BadRequest(#[source] anyhow::Error),
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

impl ResponseError for APIError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        match self {
            Self::AuthError(_) => {
                HttpResponse::Unauthorized().body(self.to_string())
            }
            Self::GetIdentityError(_) => {
                HttpResponse::Unauthorized().body(self.to_string())
            }
            Self::BadRequest(e) => {
                HttpResponse::BadRequest().body(format!("{self}: {e}"))
            }
            Self::UnexpectedError(_) => {
                HttpResponse::InternalServerError().body(self.to_string())
            }
        }
    }
}

fn get_user_id(user: &Identity) -> Result<store::UserId, APIError> {
    let id_str = user.id().map_err(APIError::GetIdentityError)?;
    // special case: since this is used in so many routes, the user_id is
    // recorded here, but attaches to the span for the api route itself
    tracing::Span::current()
        .record("user_id", tracing::field::display(&id_str));
    Ok(store::UserId(
        Uuid::parse_str(&id_str).map_err(anyhow::Error::from)?,
    ))
}
