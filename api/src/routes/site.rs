use actix_identity::Identity;
use actix_web::{HttpResponse, post, web};
use sqlx::PgPool;

use crate::store;

use super::{APIError, get_user_id, get_validated_member};

#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/create_site")]
pub async fn create_site(
    user: Identity,
    details: web::Json<payloads::Site>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.0.community_id, &pool).await?;
    let site =
        store::create_site(&details, &validated_member, &pool, &time_source)
            .await?;
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

#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/site")]
pub async fn update_site(
    user: Identity,
    details: web::Json<payloads::requests::UpdateSite>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let community_id =
        store::get_site_community_id(&details.site_id, &pool).await?;
    let actor = get_validated_member(&user_id, &community_id, &pool).await?;
    let site =
        store::update_site(&details, &actor, &pool, &time_source).await?;
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

#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/soft_delete_site")]
pub async fn soft_delete_site(
    user: Identity,
    site_id: web::Json<payloads::SiteId>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let community_id = store::get_site_community_id(&site_id, &pool).await?;
    let actor = get_validated_member(&user_id, &community_id, &pool).await?;
    store::soft_delete_site(&site_id, &actor, &pool, &time_source).await?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/sites")]
pub async fn list_sites(
    user: Identity,
    community_id: web::Json<payloads::CommunityId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let sites = store::list_sites(&community_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(sites))
}

// Site Image Routes

#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/create_site_image")]
pub async fn create_site_image(
    user: Identity,
    details: web::Json<payloads::requests::CreateSiteImage>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let site_image_id =
        store::create_site_image(&details, &user_id, &pool, &time_source)
            .await?;
    Ok(HttpResponse::Ok().json(site_image_id))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/get_site_image")]
pub async fn get_site_image(
    user: Identity,
    site_image_id: web::Json<payloads::SiteImageId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let site_image =
        store::get_site_image(&site_image_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(site_image))
}

#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/update_site_image")]
pub async fn update_site_image(
    user: Identity,
    details: web::Json<payloads::requests::UpdateSiteImage>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let site_image =
        store::update_site_image(&details, &user_id, &pool, &time_source)
            .await?;
    Ok(HttpResponse::Ok().json(site_image))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/delete_site_image")]
pub async fn delete_site_image(
    user: Identity,
    site_image_id: web::Json<payloads::SiteImageId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    store::delete_site_image(&site_image_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/list_site_images")]
pub async fn list_site_images(
    user: Identity,
    community_id: web::Json<payloads::CommunityId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let site_images =
        store::list_site_images(&community_id, &user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(site_images))
}

// Space Routes

#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/create_space")]
pub async fn create_space(
    user: Identity,
    details: web::Json<payloads::Space>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let space =
        store::create_space(&details, &user_id, &pool, &time_source).await?;
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

#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/space")]
pub async fn update_space(
    user: Identity,
    details: web::Json<payloads::requests::UpdateSpace>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let space = store::update_space(
        &details.space_id,
        &details.space_details,
        &user_id,
        &pool,
        &time_source,
    )
    .await?;
    Ok(HttpResponse::Ok().json(space))
}

#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/spaces_batch")]
pub async fn update_spaces(
    user: Identity,
    details: web::Json<payloads::requests::UpdateSpaces>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let spaces =
        store::update_spaces(&details.spaces, &user_id, &pool, &time_source)
            .await?;
    Ok(HttpResponse::Ok().json(spaces))
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
