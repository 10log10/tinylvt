pub mod auction;
pub mod billing;
pub mod community;
pub mod currency;
pub mod login;
pub mod proxy_bidding;
pub mod site;
pub mod sse;

use actix_identity::Identity;
use actix_web::{
    HttpResponse, Responder, ResponseError, body::BoxBody,
    dev::HttpServiceFactory, get, http::StatusCode, web,
};
use payloads::ApiError;
use sqlx::PgPool;
use uuid::Uuid;

use crate::store::{self, StoreError};

pub fn api_services() -> impl HttpServiceFactory {
    web::scope("/api")
        .service(health_check)
        .service(platform_stats)
        .service(login::login)
        .service(login::login_check)
        .service(login::user_profile)
        .service(login::update_profile)
        .service(login::delete_user)
        .service(login::logout)
        .service(login::create_account)
        .service(login::verify_email)
        .service(login::forgot_password)
        .service(login::reset_password)
        .service(login::resend_verification_email)
        .service(community::create_community)
        .service(community::get_communities)
        .service(community::invite_community_member)
        .service(community::get_received_invites)
        .service(community::get_issued_invites)
        .service(community::delete_invite)
        .service(community::get_invite_community_name)
        .service(community::accept_invite)
        .service(community::get_members)
        .service(community::set_membership_schedule)
        .service(community::get_membership_schedule)
        .service(community::update_member_active_status)
        .service(community::bulk_activate_members)
        .service(community::remove_member)
        .service(community::change_member_role)
        .service(community::leave_community)
        .service(currency::get_orphaned_accounts)
        .service(currency::resolve_orphaned_balance)
        .service(community::delete_community)
        .service(community::update_community_details)
        .service(site::create_site)
        .service(site::get_site)
        .service(site::update_site)
        .service(site::delete_site)
        .service(site::soft_delete_site)
        .service(site::restore_site)
        .service(site::list_sites)
        .service(site::create_site_image)
        .service(site::get_site_image)
        .service(site::get_site_image_bytes)
        .service(site::update_site_image)
        .service(site::delete_site_image)
        .service(site::list_site_images)
        .service(site::create_space)
        .service(site::get_space)
        .service(site::update_space)
        .service(site::update_spaces)
        .service(site::delete_space)
        .service(site::soft_delete_space)
        .service(site::restore_space)
        .service(site::list_spaces)
        .service(auction::create_auction)
        .service(auction::get_auction)
        .service(auction::delete_auction)
        .service(auction::schedule_auction)
        .service(auction::cancel_auction)
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
        .service(proxy_bidding::list_proxy_bidding_participants)
        .service(proxy_bidding::delete_proxy_bidding)
        .service(currency::update_credit_limit_override)
        .service(currency::get_member_credit_limit_override)
        .service(currency::get_member_currency_info)
        .service(currency::get_member_transactions)
        .service(currency::create_transfer)
        .service(currency::get_treasury_account)
        .service(currency::get_treasury_transactions)
        .service(currency::treasury_credit_operation)
        .service(currency::reset_all_balances)
        .service(currency::update_currency_config)
        .service(billing::get_community_storage_usage)
        .service(billing::get_subscription_info)
        .service(billing::create_checkout_session)
        .service(billing::create_portal_session)
        .service(billing::stripe_webhook)
        .service(sse::sse_auction)
}

#[get("/health_check")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("healthy")
}

#[get("/platform_stats")]
pub async fn platform_stats(
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let stats = store::get_platform_stats(&pool).await?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "public, max-age=3600"))
        .json(stats))
}

#[derive(Debug, thiserror::Error)]
pub enum RouteError {
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error("Bad request")]
    BadRequest(#[source] anyhow::Error),
    #[error("Not found")]
    NotFound(#[source] anyhow::Error),
    /// A typed client-facing error, serialized as JSON in the response body
    /// so clients can match on the exact variant.
    #[error(transparent)]
    Api(payloads::ApiError),
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

/// Status code for a typed API error. `MemberNotFound` is an auth failure
/// since membership is what authorizes access to community resources.
/// Not-found variants map to 404; everything else is a client error.
fn api_error_status(e: &ApiError) -> StatusCode {
    match e {
        ApiError::MemberNotFound => StatusCode::UNAUTHORIZED,
        ApiError::TokenNotFound
        | ApiError::UserNotFound
        | ApiError::CommunityNotFound
        | ApiError::SiteNotFound
        | ApiError::SpaceNotFound
        | ApiError::SiteImageNotFound
        | ApiError::AuctionNotFound
        | ApiError::AuctionRoundNotFound
        | ApiError::RoundSpaceResultNotFound
        | ApiError::BidNotFound
        | ApiError::UserValueNotFound
        | ApiError::ProxyBiddingNotFound
        | ApiError::CommunityInviteNotFound
        | ApiError::OpenHoursNotFound
        | ApiError::AuctionParamsNotFound
        | ApiError::AccountNotFound => StatusCode::NOT_FOUND,
        _ => StatusCode::BAD_REQUEST,
    }
}

impl ResponseError for RouteError {
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
            Self::Api(e) => HttpResponse::build(api_error_status(e)).json(e),
            Self::UnexpectedError(e) => {
                tracing::error!(error = ?e, "Internal server error");
                HttpResponse::InternalServerError().body(self.to_string())
            }
        }
    }
}

impl From<StoreError> for RouteError {
    fn from(e: StoreError) -> Self {
        match e {
            // Client-facing errors cross the HTTP boundary as typed JSON.
            StoreError::Api(api) => RouteError::Api(api),

            // Unique violations are client errors, but the sqlx detail is
            // internal; send only the generic message.
            StoreError::NotUnique(_) => RouteError::BadRequest(e.into()),

            // Database errors, external service errors, and invariant
            // violations are internal server errors.
            StoreError::Database(_)
            | StoreError::StripeError(_)
            | StoreError::UnexpectedError(_)
            | StoreError::InvalidAccountOwnership
            | StoreError::InvalidCurrencyConfiguration
            | StoreError::AccountNotLocked
            | StoreError::UnquantizedJournalLine { .. } => {
                RouteError::UnexpectedError(e.into())
            }
        }
    }
}

fn get_user_id(user: &Identity) -> Result<payloads::UserId, RouteError> {
    let id_str = user.id().map_err(|e| {
        RouteError::AuthError(
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
) -> Result<store::ValidatedMember, RouteError> {
    let result = store::get_validated_member(user_id, community_id, pool).await;
    match result {
        Ok(validated_member) => Ok(validated_member),
        Err(e) => Err(match e {
            // assume any errors from the database mean that the member couldn't
            // have their membership validated
            StoreError::Api(ApiError::MemberNotFound) => {
                RouteError::Api(ApiError::MemberNotFound)
            }
            _ => RouteError::UnexpectedError(e.into()),
        }),
    }
}
