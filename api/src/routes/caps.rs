//! Space category and bidder cap routes. Thin wrappers; validation and
//! application logic live in `store::caps`.

use actix_identity::Identity;
use actix_web::{HttpResponse, post, web};
use sqlx::PgPool;

use super::{RouteError, get_user_id};
use crate::store;

#[post("/create_space_category")]
pub async fn create_space_category(
    user: Identity,
    details: web::Json<payloads::SpaceCategory>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let category =
        store::create_space_category(&details, &user_id, &pool, &time_source)
            .await?;
    Ok(HttpResponse::Ok().json(category.id))
}

#[post("/space_category")]
pub async fn update_space_category(
    user: Identity,
    details: web::Json<payloads::requests::UpdateSpaceCategory>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let category =
        store::update_space_category(&details, &user_id, &pool, &time_source)
            .await?;
    Ok(HttpResponse::Ok().json(category))
}

#[post("/delete_space_category")]
pub async fn delete_space_category(
    user: Identity,
    category_id: web::Json<payloads::SpaceCategoryId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    store::delete_space_category(&category_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/space_categories")]
pub async fn list_space_categories(
    user: Identity,
    community_id: web::Json<payloads::CommunityId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let categories =
        store::list_space_categories(&community_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(categories))
}

#[post("/set_bidder_cap")]
pub async fn set_bidder_cap(
    user: Identity,
    details: web::Json<payloads::requests::SetBidderCap>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    store::set_bidder_cap(&details, &user_id, &pool, &time_source).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/bidder_caps")]
pub async fn list_bidder_caps(
    user: Identity,
    auction_id: web::Json<payloads::AuctionId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let caps = store::list_bidder_caps(&auction_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(caps))
}

#[post("/my_bidder_caps")]
pub async fn my_bidder_caps(
    user: Identity,
    auction_id: web::Json<payloads::AuctionId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let caps = store::my_bidder_caps(&auction_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(caps))
}

#[post("/seed_bidder_caps")]
pub async fn seed_bidder_caps(
    user: Identity,
    details: web::Json<payloads::requests::SeedBidderCaps>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    store::seed_bidder_caps(&details, &user_id, &pool, &time_source).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/set_bidder_cap_for_all")]
pub async fn set_bidder_cap_for_all(
    user: Identity,
    details: web::Json<payloads::requests::SetBidderCapForAll>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    store::set_bidder_cap_for_all(&details, &user_id, &pool, &time_source)
        .await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/set_cap_delegation")]
pub async fn set_cap_delegation(
    user: Identity,
    details: web::Json<payloads::requests::SetCapDelegation>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    store::set_cap_delegation(&details, &user_id, &pool, &time_source).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/cap_delegations")]
pub async fn list_cap_delegations(
    user: Identity,
    auction_id: web::Json<payloads::AuctionId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let delegations =
        store::list_cap_delegations(&auction_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(delegations))
}

#[post("/my_cap_delegations")]
pub async fn my_cap_delegations(
    user: Identity,
    auction_id: web::Json<payloads::AuctionId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let delegations =
        store::my_cap_delegations(&auction_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(delegations))
}
