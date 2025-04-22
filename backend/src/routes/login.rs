use actix_identity::Identity;
use actix_web::{
    HttpMessage, HttpRequest, HttpResponse, Responder, ResponseError,
    body::BoxBody, dev::HttpServiceFactory, http::header::LOCATION, post, web,
};
use sqlx::PgPool;

use crate::password::{AuthError, Credentials, validate_credentials};

pub fn login_routes() -> impl HttpServiceFactory {
    // (login, logout, relogin)
    (login,)
}

/// User-visible error login errors. Only the top-level message is sent.
#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

impl ResponseError for LoginError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        match self {
            Self::AuthError(e) => {
                HttpResponse::Unauthorized().body(e.to_string())
            }
            Self::UnexpectedError(e) => {
                HttpResponse::InternalServerError().body(e.to_string())
            }
        }
    }
}

#[tracing::instrument(
    skip(credentials, pool),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
#[post("/login")]
async fn login(
    request: HttpRequest,
    credentials: web::Form<Credentials>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, LoginError> {
    tracing::Span::current()
        .record("username", tracing::field::display(&credentials.username));
    match validate_credentials(credentials.0, &pool).await {
        Ok(user_id) => {
            tracing::Span::current()
                .record("user_id", tracing::field::display(&user_id.0));
            Identity::login(&request.extensions(), user_id.0.to_string())
                .map_err(|e| LoginError::UnexpectedError(e.into()))?;
            Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/"))
                .finish())
        }
        Err(e) => {
            let e = match e {
                AuthError::InvalidCredentials(_) => {
                    LoginError::AuthError(e.into())
                }
                AuthError::UnexpectedError(_) => {
                    LoginError::UnexpectedError(e.into())
                }
            };
            Err(e)
        }
    }
}

#[post("/logout")]
async fn logout(user: Identity) -> impl Responder {
    user.logout();
    HttpResponse::SeeOther()
        .insert_header((LOCATION, "/login"))
        .finish()
}
