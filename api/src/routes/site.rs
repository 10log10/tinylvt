use actix_identity::Identity;
use actix_web::{HttpResponse, post, web};
use sqlx::PgPool;

use crate::store;

use super::{APIError, get_user_id, get_validated_member};

#[tracing::instrument(skip(user, pool), ret)]
#[post("/create_site")]
pub async fn create_site(
    user: Identity,
    details: web::Json<payloads::Site>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.0.community_id, &pool).await?;
    let site = store::create_site(&details, &validated_member, &pool).await?;
    // return the community id so we can start using for other things
    Ok(HttpResponse::Ok().json(site.id))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/get_site")]
pub async fn get_site(
    user: Identity,
    site_id: web::Json<payloads::SiteId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let community_id = store::get_site_community_id(&site_id, &pool).await?;
    get_validated_member(&user_id, &community_id, &pool).await?;
    let site = store::get_site(&site_id, &pool).await?;
    // return the community id so we can start using for other things
    Ok(HttpResponse::Ok().json(site))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/site")]
pub async fn update_site(
    user: Identity,
    details: web::Json<payloads::requests::UpdateSite>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let community_id =
        store::get_site_community_id(&details.site_id, &pool).await?;
    let actor = get_validated_member(&user_id, &community_id, &pool).await?;
    let site = store::update_site(&details, &actor, &pool).await?;
    Ok(HttpResponse::Ok().json(site))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/delete_site")]
pub async fn delete_site(
    user: Identity,
    site_id: web::Json<payloads::SiteId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let community_id = store::get_site_community_id(&site_id, &pool).await?;
    let actor = get_validated_member(&user_id, &community_id, &pool).await?;
    store::delete_site(&site_id, &actor, &pool).await?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/create_space")]
pub async fn create_space(
    user: Identity,
    details: web::Json<payloads::Space>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let space = store::create_space(&details, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(space.id))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/get_space")]
pub async fn get_space(
    user: Identity,
    space_id: web::Json<payloads::SpaceId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let space = store::get_space(&space_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(space))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/space")]
pub async fn update_space(
    user: Identity,
    details: web::Json<payloads::requests::UpdateSpace>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let space = store::update_space(
        &details.space_id,
        &details.space_details,
        &user_id,
        &pool,
    )
    .await?;
    Ok(HttpResponse::Ok().json(space))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/delete_space")]
pub async fn delete_space(
    user: Identity,
    space_id: web::Json<payloads::SpaceId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    store::delete_space(&space_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/spaces")]
pub async fn list_spaces(
    user: Identity,
    site_id: web::Json<payloads::SiteId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let spaces = store::list_spaces(&site_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(spaces))
}
