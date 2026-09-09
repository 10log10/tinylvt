use actix_identity::Identity;
use actix_web::{HttpResponse, post, web};
use payloads::{AuctionId, AuctionRoundId, SpaceId};
use sqlx::PgPool;

use crate::routes::{RouteError, get_user_id};
use crate::{store, time::TimeSource};

#[post("/create_auction")]
pub async fn create_auction(
    user: Identity,
    details: web::Json<payloads::Auction>,
    pool: web::Data<PgPool>,
    time_source: web::Data<TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let auction_id =
        store::create_auction(&details, &user_id, &pool, &time_source).await?;
    Ok(HttpResponse::Ok().json(auction_id))
}

#[post("/auction")]
pub async fn get_auction(
    user: Identity,
    auction_id: web::Json<payloads::AuctionId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let auction = store::read_auction(&auction_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(auction))
}

#[post("/auction_funding")]
pub async fn get_auction_funding(
    user: Identity,
    auction_id: web::Json<payloads::AuctionId>,
    pool: web::Data<PgPool>,
    time_source: web::Data<TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let funding = store::funding::get_auction_funding(
        &auction_id,
        &user_id,
        &time_source,
        &pool,
    )
    .await?;
    Ok(HttpResponse::Ok().json(funding))
}

#[post("/delete_auction")]
pub async fn delete_auction(
    user: Identity,
    auction_id: web::Json<payloads::AuctionId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    store::delete_auction(&auction_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/update_auction")]
pub async fn update_auction(
    user: Identity,
    details: web::Json<payloads::requests::UpdateAuction>,
    pool: web::Data<PgPool>,
    time_source: web::Data<TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    store::update_auction(&details, &user_id, &pool, &time_source).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/schedule_auction")]
pub async fn schedule_auction(
    user: Identity,
    details: web::Json<payloads::requests::ScheduleAuction>,
    pool: web::Data<PgPool>,
    time_source: web::Data<TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    store::schedule_auction(&details, &user_id, &pool, &time_source).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/cancel_auction")]
pub async fn cancel_auction(
    user: Identity,
    auction_id: web::Json<payloads::AuctionId>,
    pool: web::Data<PgPool>,
    time_source: web::Data<TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    store::cancel_auction(&auction_id, &user_id, &pool, &time_source).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/auctions")]
pub async fn list_auctions(
    user: Identity,
    site_id: web::Json<payloads::SiteId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let auctions = store::list_auctions(&site_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(auctions))
}

#[post("/auction_round")]
pub async fn get_auction_round(
    user: Identity,
    round_id: web::Json<payloads::AuctionRoundId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let round = store::get_auction_round(&round_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(round))
}

#[post("/auction_rounds")]
pub async fn list_auction_rounds(
    user: Identity,
    auction_id: web::Json<AuctionId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let rounds =
        store::list_auction_rounds(&auction_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(rounds))
}

#[post("/round_space_result")]
pub async fn get_round_space_result(
    user: Identity,
    params: web::Json<(SpaceId, AuctionRoundId)>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let (space_id, round_id) = params.into_inner();
    let round =
        store::get_round_space_result(&space_id, &round_id, &user_id, &pool)
            .await?;
    Ok(HttpResponse::Ok().json(round))
}

#[post("/round_space_results_for_round")]
pub async fn list_round_space_results_for_round(
    user: Identity,
    round_id: web::Json<AuctionRoundId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let rounds =
        store::list_round_space_results_for_round(&round_id, &user_id, &pool)
            .await?;
    Ok(HttpResponse::Ok().json(rounds))
}

#[post("/get_eligibility")]
pub async fn get_eligibility(
    user: Identity,
    round_id: web::Json<AuctionRoundId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let eligibility =
        store::get_eligibility(&round_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(eligibility))
}

#[post("/list_eligibility")]
pub async fn list_eligibility(
    user: Identity,
    auction_id: web::Json<AuctionId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let eligibilities =
        store::list_eligibility(&auction_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(eligibilities))
}

#[post("/create_bid")]
pub async fn create_bid(
    user: Identity,
    params: web::Json<(SpaceId, AuctionRoundId)>,
    pool: web::Data<PgPool>,
    worker_pool: web::Data<crate::WorkerPool>,
    time_source: web::Data<TimeSource>,
    stripe_service: web::Data<crate::stripe_service::StripeService>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let (space_id, round_id) = params.into_inner();
    store::funding_flow::create_bid_with_funding(
        &space_id,
        &round_id,
        &user_id,
        &pool,
        store::funding_flow::FlowDeps {
            worker_pool: &worker_pool,
            time_source: &time_source,
            stripe_service: &stripe_service,
        },
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/authorize_funding")]
pub async fn authorize_funding(
    user: Identity,
    request: web::Json<payloads::requests::AuthorizeFunding>,
    pool: web::Data<PgPool>,
    worker_pool: web::Data<crate::WorkerPool>,
    time_source: web::Data<TimeSource>,
    stripe_service: web::Data<crate::stripe_service::StripeService>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    store::funding_flow::authorize_funding(
        &request.auction_id,
        &user_id,
        request.amount,
        &pool,
        store::funding_flow::FlowDeps {
            worker_pool: &worker_pool,
            time_source: &time_source,
            stripe_service: &stripe_service,
        },
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/checkout_funding")]
pub async fn checkout_funding(
    user: Identity,
    request: web::Json<payloads::requests::CheckoutFunding>,
    pool: web::Data<PgPool>,
    time_source: web::Data<TimeSource>,
    stripe_service: web::Data<crate::stripe_service::StripeService>,
    app_config: web::Data<crate::AppConfig>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let url = store::funding_checkout::create_funding_checkout(
        &request.auction_id,
        &user_id,
        request.amount,
        &app_config,
        &stripe_service,
        &time_source,
        &pool,
    )
    .await?;
    Ok(HttpResponse::Ok()
        .json(payloads::billing::CheckoutSessionResponse { checkout_url: url }))
}

#[post("/bid")]
pub async fn get_bid(
    user: Identity,
    params: web::Json<(SpaceId, AuctionRoundId)>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let (space_id, round_id) = params.into_inner();
    let bid = store::get_bid(&space_id, &round_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(bid))
}

#[post("/bids")]
pub async fn list_bids(
    user: Identity,
    round_id: web::Json<AuctionRoundId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let bids = store::list_bids(&round_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(bids))
}

#[post("/delete_bid")]
pub async fn delete_bid(
    user: Identity,
    params: web::Json<(SpaceId, AuctionRoundId)>,
    pool: web::Data<PgPool>,
    time_source: web::Data<TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let (space_id, round_id) = params.into_inner();
    store::delete_bid(&space_id, &round_id, &user_id, &pool, &time_source)
        .await?;
    Ok(HttpResponse::Ok().finish())
}
