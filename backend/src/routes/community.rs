use actix_identity::Identity;
use actix_web::{HttpResponse, get, post, web};
use payloads::{
    CommunityId,
    requests::{self, CreateCommunity},
};
use sqlx::PgPool;

use crate::store;

use super::{APIError, get_user_id, get_validated_member};

#[tracing::instrument(skip(user, pool), ret)]
#[post("/create_community")]
pub async fn create_community(
    user: Identity,
    details: web::Json<CreateCommunity>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    store::create_community(&details.name, user_id, &pool).await?;
    Ok(HttpResponse::Ok().finish())
}

/// Get the communities the user is a part of.
#[tracing::instrument(skip(user, pool), ret)]
#[get("/communities")]
pub async fn get_communities(
    user: Identity,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let communities = store::get_communities(&user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(communities))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/invite_member")]
pub async fn invite_community_member(
    user: Identity,
    details: web::Json<requests::InviteCommunityMember>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.0.community_id, &pool).await?;
    let invite_id = store::invite_community_member(
        &validated_member,
        &details.0.new_member_email,
        &pool,
    )
    .await?;
    Ok(HttpResponse::Ok().json(format!("/api/invite/{invite_id}")))
}

/// Get the invites the user has received
#[tracing::instrument(skip(user, pool), ret)]
#[get("/invites")]
pub async fn get_invites(
    user: Identity,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let invites = store::get_invites(&user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(invites))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/accept_invite/{invite_id}")]
pub async fn accept_invite(
    user: Identity,
    path: web::Path<payloads::InviteId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    store::accept_invite(&user_id, &path, &pool).await?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(skip(user, pool), ret)]
#[get("/members")]
pub async fn get_members(
    user: Identity,
    community_id: web::Json<CommunityId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &community_id, &pool).await?;
    let members = store::get_members(&validated_member, &pool).await?;
    Ok(HttpResponse::Ok().json(members))
}
