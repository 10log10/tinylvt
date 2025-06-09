pub mod auction;
pub mod community;
pub mod login;
pub mod proxy_bidding;
pub mod site;

use actix_identity::Identity;
use actix_web::{
    HttpResponse, Responder, ResponseError, body::BoxBody,
    dev::HttpServiceFactory, get, web,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::store::{self, StoreError};

pub fn api_services() -> impl HttpServiceFactory {
    web::scope("/api")
        .service(health_check)
        .service(login::login)
        .service(login::login_check)
        .service(login::user_profile)
        .service(login::update_profile)
        .service(login::logout)
        .service(login::create_account)
        .service(login::verify_email)
        .service(login::forgot_password)
        .service(login::reset_password)
        .service(login::resend_verification_email)
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
        .service(site::update_site)
        .service(site::delete_site)
        .service(site::create_space)
        .service(site::get_space)
        .service(site::update_space)
        .service(site::delete_space)
        .service(site::list_spaces)
        .service(auction::create_auction)
        .service(auction::get_auction)
        .service(auction::delete_auction)
        .service(auction::list_auctions)
        .service(auction::get_auction_round)
        .service(auction::list_auction_rounds)
        .service(auction::get_round_space_result)
        .service(auction::list_round_space_results_for_round)
        .service(auction::get_eligibility)
        .service(auction::list_eligibility)
        .service(auction::create_bid)
        .service(auction::get_bid)
        .service(auction::list_bids)
        .service(auction::delete_bid)
        .service(proxy_bidding::create_or_update_user_value)
        .service(proxy_bidding::get_user_value)
        .service(proxy_bidding::delete_user_value)
        .service(proxy_bidding::list_user_values)
        .service(proxy_bidding::create_or_update_proxy_bidding)
        .service(proxy_bidding::get_proxy_bidding)
        .service(proxy_bidding::delete_proxy_bidding)
}

#[get("/health_check")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("healthy")
}

#[derive(Debug, thiserror::Error)]
pub enum APIError {
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error("Bad request")]
    BadRequest(#[source] anyhow::Error),
    #[error("Not found")]
    NotFound(#[source] anyhow::Error),
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
            Self::NotFound(e) => {
                HttpResponse::NotFound().body(format!("{self}: {e}"))
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
            StoreError::MemberNotFound => APIError::AuthError(e.into()),
            StoreError::TokenNotFound => APIError::NotFound(e.into()),
            StoreError::UserNotFound => APIError::NotFound(e.into()),
            StoreError::CommunityNotFound => APIError::NotFound(e.into()),
            StoreError::SiteNotFound => APIError::NotFound(e.into()),
            StoreError::SpaceNotFound => APIError::NotFound(e.into()),
            StoreError::AuctionNotFound => APIError::NotFound(e.into()),
            StoreError::AuctionRoundNotFound => APIError::NotFound(e.into()),
            StoreError::RoundSpaceResultNotFound => {
                APIError::NotFound(e.into())
            }
            StoreError::BidNotFound => APIError::NotFound(e.into()),
            StoreError::UserValueNotFound => APIError::NotFound(e.into()),
            StoreError::ProxyBiddingNotFound => APIError::NotFound(e.into()),
            StoreError::CommunityInviteNotFound => APIError::NotFound(e.into()),
            StoreError::OpenHoursNotFound => APIError::NotFound(e.into()),
            StoreError::AuctionParamsNotFound => APIError::NotFound(e.into()),
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
