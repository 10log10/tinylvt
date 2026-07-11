use super::*;
use jiff_sqlx::ToSqlx;
use payloads::{AuctionId, PermissionLevel, SpaceId, UserId};
use sqlx::PgPool;

use crate::time::TimeSource;

pub async fn create_or_update_user_value(
    details: &payloads::requests::UserValue,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    // Verify the space exists and user has access to it
    let (_, _) = get_validated_space(
        &details.space_id,
        user_id,
        PermissionLevel::Member,
        pool,
    )
    .await?;

    sqlx::query(
        "INSERT INTO user_values (user_id, space_id, value, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $4)
        ON CONFLICT (user_id, space_id)
        DO UPDATE SET value = EXCLUDED.value, updated_at = EXCLUDED.updated_at",
    )
    .bind(user_id)
    .bind(details.space_id)
    .bind(details.value)
    .bind(time_source.now().to_sqlx())
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_user_value(
    space_id: &SpaceId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::UserValue, StoreError> {
    // Verify the space exists and user has access to it
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    let value = sqlx::query_as::<_, UserValue>(
        "SELECT * FROM user_values WHERE space_id = $1 AND user_id = $2",
    )
    .bind(space_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::UserValueNotFound,
        e => StoreError::Database(e),
    })?;

    Ok(value.into())
}

pub async fn delete_user_value(
    space_id: &SpaceId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    // Verify the space exists and user has access to it
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    sqlx::query("DELETE FROM user_values WHERE space_id = $1 AND user_id = $2")
        .bind(space_id)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn list_user_values(
    user_id: &UserId,
    site_id: &SiteId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::UserValue>, StoreError> {
    // Verify user has access to the site
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(site_id)
        .fetch_one(pool)
        .await?;

    let _ = get_validated_member(user_id, &site.community_id, pool).await?;

    let values = sqlx::query_as::<_, UserValue>(
        "SELECT uv.* FROM user_values uv
        JOIN spaces s ON uv.space_id = s.id
        WHERE uv.user_id = $1 AND s.site_id = $2",
    )
    .bind(user_id)
    .bind(site_id)
    .fetch_all(pool)
    .await?;

    Ok(values.into_iter().map(Into::into).collect())
}

pub async fn create_or_update_proxy_bidding(
    details: &payloads::requests::UseProxyBidding,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    // Verify user has access to the auction
    let (_, _) = get_validated_auction(
        &details.auction_id,
        user_id,
        PermissionLevel::Member,
        pool,
    )
    .await?;

    sqlx::query(
        "INSERT INTO use_proxy_bidding (user_id, auction_id, max_items, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $4)
        ON CONFLICT (user_id, auction_id)
        DO UPDATE SET max_items = EXCLUDED.max_items, updated_at = EXCLUDED.updated_at",
    )
    .bind(user_id)
    .bind(details.auction_id)
    .bind(details.max_items)
    .bind(time_source.now().to_sqlx())
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_proxy_bidding(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Option<payloads::responses::UseProxyBidding>, StoreError> {
    // Verify user has access to the auction
    let (_, _) = get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Member,
        pool,
    )
    .await?;

    let settings = sqlx::query_as::<_, UseProxyBidding>(
        "SELECT * FROM use_proxy_bidding WHERE auction_id = $1 AND user_id = $2",
    )
    .bind(auction_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(settings.map(|s| s.into()))
}

/// Lists members who have enabled proxy bidding for an auction. Restricted
/// to coleaders+ so they can nudge interested members who haven't opted in
/// yet. Returns only members with a `use_proxy_bidding` row (i.e. enabled);
/// there is intentionally no not-enabled list and no `max_items`, keeping
/// the disclosure to the coarse "engaged with this auction" signal.
///
/// No membership filter is needed: proxy bidding rows are deleted when a
/// member leaves, so every row here belongs to a current member.
///
/// Only available before the auction starts: the list exists to nudge
/// members into opting in beforehand, and the UI hides it once bidding is
/// underway, so the endpoint refuses post-start to keep that contract.
pub async fn list_proxy_bidding_participants(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<Vec<payloads::responses::UserIdentity>, StoreError> {
    let (auction, actor) = get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Coleader,
        pool,
    )
    .await?;

    if auction.has_started(time_source.now()) {
        return Err(StoreError::AuctionAlreadyStarted);
    }

    let community_id = actor.0.community_id;

    #[derive(sqlx::FromRow)]
    struct Participant {
        user_id: UserId,
    }

    let participants = sqlx::query_as::<_, Participant>(
        "SELECT user_id FROM use_proxy_bidding WHERE auction_id = $1
        ORDER BY created_at ASC",
    )
    .bind(auction_id)
    .fetch_all(pool)
    .await?;

    with_user_identities(
        participants,
        |p| p.user_id,
        |_, user| Ok(user),
        &community_id,
        pool,
    )
    .await
}

pub async fn delete_proxy_bidding(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    // Verify user has access to the auction
    let (_, _) = get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Member,
        pool,
    )
    .await?;

    sqlx::query(
        "DELETE FROM use_proxy_bidding WHERE auction_id = $1 AND user_id = $2",
    )
    .bind(auction_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}
