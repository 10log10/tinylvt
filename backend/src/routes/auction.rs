use actix_identity::Identity;
use actix_web::{HttpResponse, get, post, web};
use payloads::{AuctionId, SiteId};
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
