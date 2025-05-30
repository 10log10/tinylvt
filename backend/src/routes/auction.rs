use actix_identity::Identity;
use actix_web::{HttpResponse, get, post, web};
use payloads::{AuctionId, SiteId, SpaceId, AuctionRoundId};
use sqlx::PgPool;

use crate::store;

use super::{APIError, get_user_id};

#[tracing::instrument(skip(user, pool), ret)]
#[post("/create_auction")]
pub async fn create_auction(
    user: Identity,
    details: web::Json<payloads::Auction>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let auction_id = store::create_auction(&details, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(auction_id))
}

#[tracing::instrument(skip(user, pool), ret)]
#[get("/auction")]
pub async fn get_auction(
    user: Identity,
    auction_id: web::Json<AuctionId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let auction = store::read_auction(&auction_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(auction))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/delete_auction")]
pub async fn delete_auction(
    user: Identity,
    auction_id: web::Json<AuctionId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    store::delete_auction(&auction_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(skip(user, pool), ret)]
#[get("/auctions")]
pub async fn list_auctions(
    user: Identity,
    site_id: web::Json<SiteId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let auctions = store::list_auctions(&site_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(auctions))
}

#[tracing::instrument(skip(user, pool), ret)]
#[get("/auction_round")]
pub async fn get_auction_round(
    user: Identity,
    round_id: web::Json<payloads::AuctionRoundId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let round = store::get_auction_round(&round_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(round))
}

#[tracing::instrument(skip(user, pool), ret)]
#[get("/auction_rounds")]
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

#[tracing::instrument(skip(user, pool), ret)]
#[post("/space_round")]
pub async fn get_space_round(
    user: Identity,
    params: web::Json<(SpaceId, AuctionRoundId)>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let (space_id, round_id) = params.into_inner();
    let round = store::get_space_round(&space_id, &round_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(round))
}

#[tracing::instrument(skip(user, pool), ret)]
#[get("/space_rounds")]
pub async fn list_space_rounds(
    user: Identity,
    space_id: web::Json<SpaceId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let rounds = store::list_space_rounds(&space_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(rounds))
}
