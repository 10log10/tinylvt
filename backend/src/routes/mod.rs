pub mod community;
pub mod login;
pub mod site;

use actix_identity::Identity;
use actix_web::{
    HttpResponse, Responder, ResponseError, body::BoxBody,
    dev::HttpServiceFactory, web,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::store::{self, StoreError};

pub fn api_services() -> impl HttpServiceFactory {
    web::scope("/api")
        .route("/health_check", web::get().to(health_check))
        .route("/login", web::post().to(login::login))
        .route("/login_check", web::post().to(login::login_check))
        .route("/logout", web::post().to(login::logout))
        .route("/create_account", web::post().to(login::create_account))
        .service(community::create_community)
        .service(community::get_communities)
        .service(community::invite_community_member)
        .service(community::get_invites)
        .service(community::accept_invite)
        .service(community::get_members)
        .service(community::set_membership_schedule)
        .service(community::get_membership_schedule)
        .service(site::create_site)
        .service(site::get_site)
}

pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("healthy")
}

#[derive(Debug, thiserror::Error)]
pub enum APIError {
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error("Bad request")]
    BadRequest(#[source] anyhow::Error),
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

impl ResponseError for APIError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        match self {
            Self::AuthError(e) => {
                HttpResponse::Unauthorized().body(format!("{self}: {e}"))
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

impl From<StoreError> for APIError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::Database(_) => APIError::UnexpectedError(e.into()),
            _ => APIError::BadRequest(e.into()),
        }
    }
}

fn get_user_id(user: &Identity) -> Result<payloads::UserId, APIError> {
    let id_str = user.id().map_err(|e| {
        APIError::AuthError(
            anyhow::Error::from(e).context("Invalid login session"),
        )
    })?;
    // special case: since this is used in so many routes, the user_id is
    // recorded here, but attaches to the span for the api route itself
    tracing::Span::current()
        .record("user_id", tracing::field::display(&id_str));
    Ok(payloads::UserId(
        Uuid::parse_str(&id_str).map_err(anyhow::Error::from)?,
    ))
}

async fn get_validated_member(
    user_id: &payloads::UserId,
    community_id: &payloads::CommunityId,
    pool: &PgPool,
) -> Result<store::ValidatedMember, APIError> {
    let result = store::get_validated_member(user_id, community_id, pool).await;
    match result {
        Ok(validated_member) => Ok(validated_member),
        Err(e) => Err(match e {
            // assume any errors from the database mean that the member couldn't
            // have their membership validated
            StoreError::MemberNotFound => APIError::AuthError(
                anyhow::Error::from(e)
                    .context("Couldn't validate community membership"),
            ),
            _ => APIError::UnexpectedError(e.into()),
        }),
    }
}
