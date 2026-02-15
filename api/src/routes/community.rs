use actix_identity::Identity;
use actix_web::{HttpResponse, get, post, web};
use payloads::{
    CommunityId,
    requests::{self, CreateCommunity},
};
use sqlx::PgPool;

use crate::store;

use super::{APIError, get_user_id, get_validated_member};

#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/create_community")]
pub async fn create_community(
    user: Identity,
    details: web::Json<CreateCommunity>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let community =
        store::create_community(&details, user_id, &pool, &time_source).await?;
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

#[tracing::instrument(
    skip(user, pool, email_service, config, time_source),
    ret
)]
#[post("/invite_member")]
pub async fn invite_community_member(
    user: Identity,
    details: web::Json<requests::InviteCommunityMember>,
    pool: web::Data<PgPool>,
    email_service: web::Data<crate::email::EmailService>,
    config: web::Data<crate::AppConfig>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.0.community_id, &pool).await?;
    let invite_id = store::invite_community_member(
        &validated_member,
        &details.0.new_member_email,
        details.0.single_use,
        &pool,
        &time_source,
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
                &invite_id.to_string(),
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
#[get("/received_invites")]
pub async fn get_received_invites(
    user: Identity,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let invites = store::get_received_invites(&user_id, &pool).await?;
    Ok(HttpResponse::Ok().json(invites))
}

/// Get the invites that have been issued for a community (moderator+ only)
#[tracing::instrument(skip(user, pool), ret)]
#[post("/issued_invites")]
pub async fn get_issued_invites(
    user: Identity,
    community_id: web::Json<CommunityId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &community_id, &pool).await?;
    let invites = store::get_issued_invites(&validated_member, &pool).await?;
    Ok(HttpResponse::Ok().json(invites))
}

/// Delete/rescind a community invite (moderator+ only)
#[tracing::instrument(skip(user, pool), ret)]
#[post("/delete_invite")]
pub async fn delete_invite(
    user: Identity,
    details: web::Json<requests::DeleteInvite>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;
    store::delete_invite(&validated_member, &details.invite_id, &pool).await?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(skip(pool), ret)]
#[get("/invite_community_name/{invite_id}")]
pub async fn get_invite_community_name(
    path: web::Path<payloads::InviteId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let community_name = store::get_invite_community_name(&path, &pool).await?;
    Ok(HttpResponse::Ok().json(community_name))
}

#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/accept_invite/{invite_id}")]
pub async fn accept_invite(
    user: Identity,
    path: web::Path<payloads::InviteId>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    store::accept_invite(&user_id, &path, &pool, &time_source).await?;
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
#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/membership_schedule")]
pub async fn set_membership_schedule(
    user: Identity,
    details: web::Json<requests::SetMembershipSchedule>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;
    store::set_membership_schedule(
        &validated_member,
        &details.schedule,
        &pool,
        &time_source,
    )
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

/// Update a member's active status (moderator+ only)
#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/update_member_active_status")]
pub async fn update_member_active_status(
    user: Identity,
    details: web::Json<requests::UpdateMemberActiveStatus>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;

    store::update_member_active_status(
        &validated_member,
        &details.member_user_id,
        details.is_active,
        &pool,
        &time_source,
    )
    .await?;

    Ok(HttpResponse::Ok().finish())
}

/// Remove a member from the community (moderator+ only)
#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/remove_member")]
pub async fn remove_member(
    user: Identity,
    details: web::Json<requests::RemoveMember>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;

    store::remove_member(
        &validated_member,
        &details.member_user_id,
        &pool,
        &time_source,
    )
    .await?;

    Ok(HttpResponse::Ok().finish())
}

/// Change a member's role (coleader+ only)
#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/change_member_role")]
pub async fn change_member_role(
    user: Identity,
    details: web::Json<requests::ChangeMemberRole>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;

    store::change_member_role(
        &validated_member,
        &details.member_user_id,
        details.new_role,
        &pool,
        &time_source,
    )
    .await?;

    Ok(HttpResponse::Ok().finish())
}

/// Leave a community voluntarily
#[tracing::instrument(skip(user, pool), ret)]
#[post("/leave_community")]
pub async fn leave_community(
    user: Identity,
    details: web::Json<requests::LeaveCommunity>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;

    store::leave_community(&member, &pool).await?;

    Ok(HttpResponse::Ok().finish())
}

/// Delete a community (leader only)
#[tracing::instrument(skip(user, pool), ret)]
#[post("/delete_community")]
pub async fn delete_community(
    user: Identity,
    community_id: web::Json<CommunityId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &community_id, &pool).await?;
    store::delete_community(&community_id, &validated_member, &pool).await?;
    Ok(HttpResponse::Ok().finish())
}
