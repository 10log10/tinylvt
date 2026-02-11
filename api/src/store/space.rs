use super::*;
use jiff_sqlx::ToSqlx;
use payloads::{PermissionLevel, SiteId, SpaceId, UserId};
use sqlx::PgPool;

use crate::time::TimeSource;

/// Get a space and validate that the user has the required permission
/// level in the site's community. Returns both the space and the
/// validated member if successful.
pub(super) async fn get_validated_space(
    space_id: &SpaceId,
    user_id: &UserId,
    required_permission: PermissionLevel,
    pool: &PgPool,
) -> Result<(Space, ValidatedMember), StoreError> {
    let space =
        sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = $1")
            .bind(space_id)
            .fetch_one(pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => StoreError::SpaceNotFound,
                e => StoreError::Database(e),
            })?;

    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(space.site_id)
        .fetch_one(pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StoreError::SiteNotFound,
            e => StoreError::Database(e),
        })?;

    let actor = get_validated_member(user_id, &site.community_id, pool).await?;

    if !required_permission.validate(actor.0.role) {
        return Err(StoreError::InsufficientPermissions {
            required: required_permission,
        });
    }

    Ok((space, actor))
}

/// Internal transaction-aware space creation function.
/// Caller is responsible for managing the transaction and validating
/// permissions.
async fn create_space_tx(
    details: &payloads::Space,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    time_source: &TimeSource,
) -> Result<Space, StoreError> {
    let space = sqlx::query_as::<_, Space>(
        "INSERT INTO spaces (
            site_id,
            name,
            description,
            eligibility_points,
            is_available,
            site_image_id,
            created_at,
            updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $7) RETURNING *",
    )
    .bind(details.site_id)
    .bind(&details.name)
    .bind(&details.description)
    .bind(details.eligibility_points)
    .bind(details.is_available)
    .bind(details.site_image_id)
    .bind(time_source.now().to_sqlx())
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| map_space_name_unique_error(e, &details.name))?;

    Ok(space)
}

pub async fn create_space(
    details: &payloads::Space,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<Space, StoreError> {
    // Get the site and validate user permissions
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(details.site_id)
        .fetch_one(pool)
        .await?;

    let actor = get_validated_member(user_id, &site.community_id, pool).await?;

    if !PermissionLevel::Coleader.validate(actor.0.role) {
        return Err(StoreError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        });
    }

    let mut tx = pool.begin().await?;
    let space = create_space_tx(details, &mut tx, time_source).await?;
    tx.commit().await?;

    Ok(space)
}

pub async fn get_space(
    space_id: &SpaceId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::Space, StoreError> {
    let (space, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    Ok(space.into())
}

/// Check if a space has auction history (bids or round results).
/// Returns true if the space has been used in any auction.
async fn space_has_auction_history(
    space_id: &SpaceId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<bool, StoreError> {
    let has_history = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1 FROM bids WHERE space_id = $1
            UNION
            SELECT 1 FROM round_space_results WHERE space_id = $1
        )",
    )
    .bind(space_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(has_history)
}

/// Check if update contains nontrivial changes (name or eligibility_points).
/// These fields trigger copy-on-write when the space has auction history.
fn has_nontrivial_changes(
    old_space: &Space,
    new_details: &payloads::Space,
) -> bool {
    old_space.name != new_details.name
        || old_space.eligibility_points != new_details.eligibility_points
}

/// Internal transaction-aware space update function.
/// Caller is responsible for managing the transaction.
async fn update_space_tx(
    space_id: &SpaceId,
    details: &payloads::Space,
    user_id: &UserId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<payloads::responses::UpdateSpaceResult, StoreError> {
    let (old_space, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Coleader, pool)
            .await?;

    // Check for auction history and nontrivial changes
    let has_history = space_has_auction_history(space_id, tx).await?;
    let nontrivial = has_nontrivial_changes(&old_space, details);

    if has_history && nontrivial {
        // Copy-on-write: create new space and soft-delete old one
        let new_space = create_space_tx(details, tx, time_source).await?;

        // Soft-delete old space
        let now = time_source.now().to_sqlx();
        sqlx::query(
            "UPDATE spaces SET deleted_at = $2, updated_at = $2 WHERE id = $1",
        )
        .bind(old_space.id)
        .bind(now)
        .execute(&mut **tx)
        .await?;

        return Ok(payloads::responses::UpdateSpaceResult {
            space: new_space.into(),
            was_copied: true,
            old_space_id: Some(*space_id),
        });
    }

    // Update in place
    let updated_space = sqlx::query_as::<_, Space>(
        "UPDATE spaces SET
            name = $1,
            description = $2,
            eligibility_points = $3,
            is_available = $4,
            site_image_id = $5,
            updated_at = $7
        WHERE id = $6
        RETURNING *",
    )
    .bind(&details.name)
    .bind(&details.description)
    .bind(details.eligibility_points)
    .bind(details.is_available)
    .bind(details.site_image_id)
    .bind(space_id)
    .bind(time_source.now().to_sqlx())
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| map_space_name_unique_error(e, &details.name))?;

    Ok(payloads::responses::UpdateSpaceResult {
        space: updated_space.into(),
        was_copied: false,
        old_space_id: None,
    })
}

pub async fn update_space(
    space_id: &SpaceId,
    details: &payloads::Space,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<payloads::responses::UpdateSpaceResult, StoreError> {
    let mut tx = pool.begin().await?;
    let result =
        update_space_tx(space_id, details, user_id, &mut tx, pool, time_source)
            .await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn update_spaces(
    updates: &[payloads::requests::UpdateSpace],
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<Vec<payloads::responses::UpdateSpaceResult>, StoreError> {
    if updates.is_empty() {
        return Ok(Vec::new());
    }

    // Start a transaction
    let mut tx = pool.begin().await?;

    // Process each update using the transaction-based function
    // (validation happens inside update_space_tx)
    let mut results = Vec::new();
    for update in updates {
        let result = update_space_tx(
            &update.space_id,
            &update.space_details,
            user_id,
            &mut tx,
            pool,
            time_source,
        )
        .await?;
        results.push(result);
    }

    // Commit the transaction
    tx.commit().await?;

    Ok(results)
}

pub async fn delete_space(
    space_id: &SpaceId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Coleader, pool)
            .await?;

    // Only delete if no auction history references this space.
    // This preserves auction data integrity while allowing CASCADE for bulk
    // operations like community deletion.
    let result = sqlx::query(
        "DELETE FROM spaces WHERE id = $1
         AND NOT EXISTS (SELECT 1 FROM bids WHERE space_id = $1)
         AND NOT EXISTS (SELECT 1 FROM round_space_results WHERE space_id = $1)",
    )
    .bind(space_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        // Space exists but wasn't deleted due to auction history
        return Err(StoreError::SpaceHasAuctionHistory);
    }

    Ok(())
}

pub async fn soft_delete_space(
    space_id: &SpaceId,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Coleader, pool)
            .await?;

    let now = time_source.now().to_sqlx();

    let result = sqlx::query(
        "UPDATE spaces SET deleted_at = $2, updated_at = $2 WHERE id = $1",
    )
    .bind(space_id)
    .bind(now)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(StoreError::SpaceNotFound);
    }

    Ok(())
}

pub async fn restore_space(
    space_id: &SpaceId,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let (space, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Coleader, pool)
            .await?;

    let now = time_source.now().to_sqlx();

    let result = sqlx::query(
        "UPDATE spaces SET deleted_at = NULL, updated_at = $2 WHERE id = $1",
    )
    .bind(space_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| map_space_name_unique_error(e, &space.name))?;

    if result.rows_affected() == 0 {
        return Err(StoreError::SpaceNotFound);
    }

    Ok(())
}

pub async fn list_spaces(
    site_id: &SiteId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::Space>, StoreError> {
    // Get the site and validate user permissions
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(site_id)
        .fetch_one(pool)
        .await?;

    let _ = get_validated_member(user_id, &site.community_id, pool).await?;

    let spaces = sqlx::query_as::<_, Space>(
        "SELECT * FROM spaces WHERE site_id = $1 ORDER BY name",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;

    Ok(spaces.into_iter().map(Into::into).collect())
}
