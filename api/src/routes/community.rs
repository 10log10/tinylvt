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
    let community = store::create_community(&details, user_id, &pool).await?;
    // return the community id so we can start using for other things
    Ok(HttpResponse::Ok().json(community.id))
}

/// Get the communities the user is a part of, including their role in each community.
/// This provides the frontend with role information to show/hide controls based on permissions.
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

#[tracing::instrument(skip(user, pool, email_service, config), ret)]
#[post("/invite_member")]
pub async fn invite_community_member(
    user: Identity,
    details: web::Json<requests::InviteCommunityMember>,
    pool: web::Data<PgPool>,
    email_service: web::Data<crate::email::EmailService>,
    config: web::Data<crate::Config>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.0.community_id, &pool).await?;
    let invite_id = store::invite_community_member(
        &validated_member,
        &details.0.new_member_email,
        details.0.single_use,
        &pool,
    )
    .await?;

    // Send email invitation if email address is provided
    if let Some(ref email) = details.0.new_member_email {
        // Get community information for the email
        let community =
            store::get_community_by_id(&details.0.community_id, &pool).await?;

        if let Err(e) = email_service
            .send_community_invite_email(
                email,
                &community.name,
                &config.base_url,
            )
            .await
        {
            tracing::error!("Failed to send community invite email: {}", e);
            // Don't fail the invitation creation, but log the error
        }
    }

    Ok(HttpResponse::Ok().json(invite_id))
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
#[post("/members")]
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

/// Set the community schedule all at once.
#[tracing::instrument(skip(user, pool), ret)]
#[post("/membership_schedule")]
pub async fn set_membership_schedule(
    user: Identity,
    details: web::Json<requests::SetMembershipSchedule>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;
    store::set_membership_schedule(&validated_member, &details.schedule, &pool)
        .await?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/get_membership_schedule")]
pub async fn get_membership_schedule(
    user: Identity,
    community_id: web::Json<CommunityId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &community_id, &pool).await?;
    let schedule =
        store::get_membership_schedule(&validated_member, &pool).await?;
    Ok(HttpResponse::Ok().json(schedule))
}
