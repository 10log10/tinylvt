use actix_identity::Identity;
use actix_web::{HttpResponse, web};
use payloads::requests::{COMMUNITY_NAME_MAX_LEN, CreateCommunity};
use sqlx::PgPool;

use crate::store;

use super::{APIError, get_user_id};

#[tracing::instrument(skip(user, pool), ret)]
pub async fn create_community(
    user: Identity,
    details: web::Json<CreateCommunity>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    tracing::info!("a message");
    if details.name.len() > COMMUNITY_NAME_MAX_LEN {
        return Err(APIError::BadRequest(anyhow::anyhow!(
            "Community name too long (>{COMMUNITY_NAME_MAX_LEN} characters)."
        )));
    }
    store::create_community(&details.name, user_id, &pool)
        .await
        .map_err(anyhow::Error::from)?;
    Ok(HttpResponse::Ok().finish())
}

// #[tracing::instrument(skip(user, pool), ret)]
// pub async fn invite_community_member(
// user: Identity,
// details: web::Json<requests::InviteCommunityMember>,
// pool: web::Data<PgPool>,
// ) -> Result<HttpResponse, APIError> {
// let user_id = get_user_id(&user)?;
// }
