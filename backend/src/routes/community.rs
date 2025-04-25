use actix_identity::Identity;
use actix_web::{HttpResponse, web};
use payloads::requests::CreateCommunity;
use sqlx::PgPool;

use crate::store;

use super::{APIError, get_user_id};

#[tracing::instrument(skip(user, pool))]
pub async fn create_community(
    user: Identity,
    details: web::Json<CreateCommunity>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    store::create_community(&details.name, user_id, &pool)
        .await
        .unwrap();
    sqlx::query_as::<_, store::Community>(
        "INSERT INTO communities (name) VALUES ($1) RETURNING *;",
    )
    .bind(&details.name)
    .fetch_one(pool.get_ref())
    .await
    .map_err(anyhow::Error::from)?;
    Ok(HttpResponse::Ok().finish())
}
