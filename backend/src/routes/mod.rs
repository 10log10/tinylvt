pub mod login;

use actix_identity::Identity;
use actix_web::{HttpResponse, Responder, ResponseError, body::BoxBody};
use uuid::Uuid;

use crate::store::model::UserId;

pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("healthy")
}

/// Public login errors. Only the top-level message is sent.
#[derive(Debug, thiserror::Error)]
pub enum APIError {
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error("Login expired")]
    LoginExpired(#[source] anyhow::Error),
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

impl ResponseError for APIError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        match self {
            Self::AuthError(e) => {
                HttpResponse::Unauthorized().body(e.to_string())
            }
            Self::LoginExpired(e) => {
                HttpResponse::Unauthorized().body(e.to_string())
            }
            Self::UnexpectedError(e) => {
                HttpResponse::InternalServerError().body(e.to_string())
            }
        }
    }
}

fn get_user_id(user: &Identity) -> Result<UserId, APIError> {
    let id_str = user.id().map_err(|e| APIError::LoginExpired(e.into()))?;
    // special case: since this is used in so many routes, the user_id is
    // recorded here, but attaches to the span for the api route itself
    tracing::Span::current()
        .record("user_id", tracing::field::display(&id_str));
    Ok(UserId(
        Uuid::parse_str(&id_str).map_err(anyhow::Error::from)?,
    ))
}
