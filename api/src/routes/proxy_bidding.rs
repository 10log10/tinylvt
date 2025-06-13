use actix_identity::Identity;
use actix_web::{HttpResponse, post, web};
use payloads::{AuctionId, SpaceId};
use sqlx::PgPool;

use super::{APIError, get_user_id};
use crate::store;

#[tracing::instrument(skip(user, pool), ret)]
#[post("/create_or_update_user_value")]
pub async fn create_or_update_user_value(
    user: Identity,
    details: web::Json<payloads::requests::UserValue>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    store::create_or_update_user_value(&details, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/get_user_value")]
pub async fn get_user_value(
    user: Identity,
    space_id: web::Json<SpaceId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let value = store::get_user_value(&space_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(value))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/delete_user_value")]
pub async fn delete_user_value(
    user: Identity,
    space_id: web::Json<SpaceId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    store::delete_user_value(&space_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/user_values")]
pub async fn list_user_values(
    user: Identity,
    site_id: web::Json<payloads::SiteId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let values = store::list_user_values(&user_id, &site_id, &pool).await?;
    Ok(HttpResponse::Ok().json(values))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/create_or_update_proxy_bidding")]
pub async fn create_or_update_proxy_bidding(
    user: Identity,
    details: web::Json<payloads::requests::UseProxyBidding>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    store::create_or_update_proxy_bidding(&details, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/get_proxy_bidding")]
pub async fn get_proxy_bidding(
    user: Identity,
    auction_id: web::Json<AuctionId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let settings =
        store::get_proxy_bidding(&auction_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(settings))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/delete_proxy_bidding")]
pub async fn delete_proxy_bidding(
    user: Identity,
    auction_id: web::Json<AuctionId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    store::delete_proxy_bidding(&auction_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().finish())
}
