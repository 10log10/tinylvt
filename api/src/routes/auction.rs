use actix_identity::Identity;
use actix_web::{HttpResponse, post, web};
use payloads::{AuctionId, AuctionRoundId, SpaceId};
use sqlx::PgPool;

use crate::routes::{APIError, get_user_id};
use crate::{store, time::TimeSource};

#[post("/create_auction")]
pub async fn create_auction(
    user: Identity,
    details: web::Json<payloads::Auction>,
    pool: web::Data<PgPool>,
    time_source: web::Data<TimeSource>,
) -> Result<HttpResponse, APIError> {
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
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let auction = store::read_auction(&auction_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(auction))
}

#[post("/delete_auction")]
pub async fn delete_auction(
    user: Identity,
    auction_id: web::Json<payloads::AuctionId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    store::delete_auction(&auction_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/auctions")]
pub async fn list_auctions(
    user: Identity,
    site_id: web::Json<payloads::SiteId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let auctions = store::list_auctions(&site_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(auctions))
}

#[post("/auction_round")]
pub async fn get_auction_round(
    user: Identity,
    round_id: web::Json<payloads::AuctionRoundId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let round = store::get_auction_round(&round_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(round))
}

#[post("/auction_rounds")]
pub async fn list_auction_rounds(
    user: Identity,
    auction_id: web::Json<AuctionId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
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
) -> Result<HttpResponse, APIError> {
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
) -> Result<HttpResponse, APIError> {
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
) -> Result<HttpResponse, APIError> {
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
) -> Result<HttpResponse, APIError> {
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
    time_source: web::Data<TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let (space_id, round_id) = params.into_inner();
    store::create_bid(&space_id, &round_id, &user_id, &pool, &time_source)
        .await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/bid")]
pub async fn get_bid(
    user: Identity,
    params: web::Json<(SpaceId, AuctionRoundId)>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
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
) -> Result<HttpResponse, APIError> {
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
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let (space_id, round_id) = params.into_inner();
    store::delete_bid(&space_id, &round_id, &user_id, &pool, &time_source)
        .await?;
    Ok(HttpResponse::Ok().finish())
}
