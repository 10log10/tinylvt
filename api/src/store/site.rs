use super::*;
use anyhow::Context;
use jiff_sqlx::ToSqlx;
use payloads::{PermissionLevel, SiteId};
use sqlx::{PgPool, Postgres, Transaction};

use crate::time::TimeSource;

pub async fn create_site(
    details: &payloads::Site,
    actor: &ValidatedMember,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<Site, StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        });
    }
    let mut tx = pool.begin().await?;

    let open_hours_id = match &details.open_hours {
        Some(hours) => Some(create_open_hours(hours, &mut tx).await?),
        None => None,
    };
    let auction_params_id = create_auction_params(
        &details.default_auction_params,
        &mut tx,
        time_source,
    )
    .await?;

    let site = sqlx::query_as::<_, Site>(
        "INSERT INTO sites (
            community_id,
            name,
            description,
            default_auction_params_id,
            possession_period,
            auction_lead_time,
            proxy_bidding_lead_time,
            open_hours_id,
            auto_schedule,
            timezone,
            site_image_id,
            created_at,
            updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12) RETURNING *",
    )
    .bind(actor.0.community_id)
    .bind(&details.name)
    .bind(&details.description)
    .bind(auction_params_id)
    .bind(span_to_interval(&details.possession_period)?)
    .bind(span_to_interval(&details.auction_lead_time)?)
    .bind(span_to_interval(&details.proxy_bidding_lead_time)?)
    .bind(open_hours_id)
    .bind(details.auto_schedule)
    .bind(&details.timezone)
    .bind(details.site_image_id)
    .bind(time_source.now().to_sqlx())
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(site)
}

async fn create_open_hours(
    open_hours: &payloads::OpenHours,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<OpenHoursId, StoreError> {
    let open_hours_id = sqlx::query_as::<_, OpenHoursId>(
        "INSERT INTO open_hours DEFAULT VALUES RETURNING id",
    )
    .fetch_one(&mut **tx)
    .await?;

    insert_open_hours_weekdays(&open_hours_id, open_hours, tx).await?;
    Ok(open_hours_id)
}

async fn insert_open_hours_weekdays(
    open_hours_id: &OpenHoursId,
    open_hours: &payloads::OpenHours,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), StoreError> {
    for day_of_week in &open_hours.days_of_week {
        sqlx::query(
            "INSERT INTO open_hours_weekday (
                open_hours_id,
                day_of_week,
                open_time,
                close_time
            ) VALUES ($1, $2, $3, $4)",
        )
        .bind(open_hours_id)
        .bind(day_of_week.day_of_week)
        .bind(day_of_week.open_time.to_sqlx())
        .bind(day_of_week.close_time.to_sqlx())
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub(super) async fn create_auction_params(
    params: &payloads::AuctionParams,
    tx: &mut Transaction<'_, Postgres>,
    time_source: &TimeSource,
) -> Result<AuctionParamsId, StoreError> {
    Ok(sqlx::query_as::<_, AuctionParamsId>(
        "INSERT INTO auction_params (
                round_duration,
                bid_increment,
                activity_rule_params,
                created_at,
                updated_at
            ) VALUES ($1, $2, $3, $4, $4) RETURNING id",
    )
    .bind(span_to_interval(&params.round_duration)?)
    .bind(params.bid_increment)
    .bind(Json(params.activity_rule_params.clone()))
    .bind(time_source.now().to_sqlx())
    .fetch_one(&mut **tx)
    .await?)
}

pub async fn get_site_community_id(
    site_id: &SiteId,
    pool: &PgPool,
) -> Result<CommunityId, StoreError> {
    sqlx::query_as::<_, CommunityId>(
        "SELECT community_id FROM sites WHERE id = $1",
    )
    .bind(site_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::SiteNotFound,
        e => StoreError::Database(e),
    })
}

pub async fn get_site(
    site_id: &SiteId,
    pool: &PgPool,
) -> Result<payloads::responses::Site, StoreError> {
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(site_id)
        .fetch_one(pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StoreError::SiteNotFound,
            e => StoreError::Database(e),
        })?;
    let open_hours = match &site.open_hours_id {
        Some(open_hours_id) => {
            let days_of_week = sqlx::query_as::<_, payloads::OpenHoursWeekday>(
                "SELECT * FROM open_hours_weekday WHERE open_hours_id = $1",
            )
            .bind(open_hours_id)
            .fetch_all(pool)
            .await?;
            Some(payloads::OpenHours { days_of_week })
        }
        None => None,
    };
    let default_auction_params = sqlx::query_as::<_, AuctionParams>(
        "SELECT * FROM auction_params WHERE id = $1",
    )
    .bind(site.default_auction_params_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionParamsNotFound,
        e => StoreError::Database(e),
    })?;
    let site_details = payloads::Site {
        community_id: site.community_id,
        name: site.name,
        description: site.description,
        default_auction_params: default_auction_params.into(),
        possession_period: site.possession_period,
        auction_lead_time: site.auction_lead_time,
        proxy_bidding_lead_time: site.proxy_bidding_lead_time,
        open_hours,
        auto_schedule: site.auto_schedule,
        timezone: site.timezone,
        site_image_id: site.site_image_id,
    };
    Ok(payloads::responses::Site {
        site_id: site.id,
        site_details,
        created_at: site.created_at,
        updated_at: site.updated_at,
        deleted_at: site.deleted_at,
    })
}

pub async fn update_site(
    update_site: &payloads::requests::UpdateSite,
    actor: &ValidatedMember,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<responses::Site, StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    let details = &update_site.site_details;

    let existing_site =
        sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
            .bind(update_site.site_id)
            .fetch_one(pool)
            .await?;

    let mut tx = pool.begin().await?;

    let new_open_hours_id = update_open_hours(
        &existing_site.open_hours_id,
        &details.open_hours,
        &mut tx,
    )
    .await?;

    let new_auction_params_id = create_auction_params(
        &details.default_auction_params,
        &mut tx,
        time_source,
    )
    .await?;

    sqlx::query(
        "UPDATE sites SET
            name = $1,
            description = $2,
            default_auction_params_id = $3,
            possession_period = $4,
            auction_lead_time = $5,
            proxy_bidding_lead_time = $6,
            open_hours_id = $7,
            auto_schedule = $8,
            timezone = $9,
            site_image_id = $10,
            updated_at = $12
        WHERE id = $11",
    )
    .bind(&details.name)
    .bind(&details.description)
    .bind(new_auction_params_id)
    .bind(span_to_interval(&details.possession_period)?)
    .bind(span_to_interval(&details.auction_lead_time)?)
    .bind(span_to_interval(&details.proxy_bidding_lead_time)?)
    .bind(new_open_hours_id)
    .bind(details.auto_schedule)
    .bind(&details.timezone)
    .bind(details.site_image_id)
    .bind(existing_site.id)
    .bind(time_source.now().to_sqlx())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let site = get_site(&existing_site.id, pool).await?;

    cleanup_unused_auction_params(pool).await;
    Ok(site)
}

pub(super) async fn cleanup_unused_auction_params(pool: &PgPool) {
    if let Err(e) = sqlx::query(
        "DELETE FROM auction_params p
        WHERE NOT EXISTS (
            SELECT FROM sites
            WHERE default_auction_params_id = p.id
        ) AND NOT EXISTS (
            SELECT FROM auctions
            WHERE auction_params_id = p.id
        );",
    )
    .execute(pool)
    .await
    .context("cleanup unused auction params")
    {
        tracing::error!("{e:#}");
    }
}

/// Update an existing open hours (if it exists), returning the id.
async fn update_open_hours(
    // existing open hours
    open_hours_id: &Option<OpenHoursId>,
    new_open_hours: &Option<payloads::OpenHours>,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Option<OpenHoursId>, StoreError> {
    // delete the existing open hours
    sqlx::query("DELETE FROM open_hours WHERE id = $1;")
        .bind(open_hours_id)
        .execute(&mut **tx)
        .await?;

    // add new open hours
    match new_open_hours {
        Some(new_open_hours) => {
            Ok(Some(create_open_hours(new_open_hours, tx).await?))
        }
        None => Ok(None),
    }
}

pub async fn delete_site(
    site_id: &payloads::SiteId,
    actor: &ValidatedMember,
    pool: &PgPool,
) -> Result<(), StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    let existing_site =
        sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
            .bind(site_id)
            .fetch_one(pool)
            .await?;

    let mut tx = pool.begin().await?;

    // Remove any remaining open hours
    update_open_hours(&existing_site.open_hours_id, &None, &mut tx).await?;

    let delete_result = sqlx::query("DELETE FROM sites WHERE id = $1")
        .bind(site_id)
        .execute(&mut *tx)
        .await;

    match delete_result {
        Ok(_) => {
            tx.commit().await?;
            cleanup_unused_auction_params(pool).await;
            Ok(())
        }
        Err(sqlx::Error::Database(db_err))
            if db_err.is_foreign_key_violation() =>
        {
            // FK violation means site has auctions with financial history:
            // sites → auctions (CASCADE) → journal_entries.auction_id (RESTRICT)
            Err(StoreError::SiteHasFinancialHistory)
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn soft_delete_site(
    site_id: &payloads::SiteId,
    actor: &ValidatedMember,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    let now = time_source.now().to_sqlx();

    // Use transaction to ensure atomicity
    let mut tx = pool.begin().await?;

    // Cancel any active auctions for this site
    // (auctions where end_at is NULL or in the future)
    sqlx::query(
        "UPDATE auctions
         SET end_at = $2, updated_at = $2
         WHERE site_id = $1
         AND (end_at IS NULL OR end_at > $2)",
    )
    .bind(site_id)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    // Soft delete the site
    let result = sqlx::query(
        "UPDATE sites SET deleted_at = $2, updated_at = $2 WHERE id = $1",
    )
    .bind(site_id)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() == 0 {
        return Err(StoreError::SiteNotFound);
    }

    tx.commit().await?;

    Ok(())
}

pub async fn restore_site(
    site_id: &payloads::SiteId,
    actor: &ValidatedMember,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    let now = time_source.now().to_sqlx();

    let result = sqlx::query(
        "UPDATE sites SET deleted_at = NULL, updated_at = $2 WHERE id = $1",
    )
    .bind(site_id)
    .bind(now)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(StoreError::SiteNotFound);
    }

    Ok(())
}

pub async fn list_sites(
    community_id: &payloads::CommunityId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::Site>, StoreError> {
    // Validate user is a member of the community
    let _ = get_validated_member(user_id, community_id, pool).await?;

    let sites = sqlx::query_as::<_, Site>(
        "SELECT * FROM sites WHERE community_id = $1 ORDER BY name",
    )
    .bind(community_id)
    .fetch_all(pool)
    .await?;

    // Convert to response format
    let mut site_responses = Vec::new();
    for site in sites {
        let site_response = get_site(&site.id, pool).await?;
        site_responses.push(site_response);
    }

    Ok(site_responses)
}

pub async fn create_site_image(
    details: &payloads::requests::CreateSiteImage,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<payloads::SiteImageId, StoreError> {
    // Validate user is a member of the community
    let actor =
        get_validated_member(user_id, &details.community_id, pool).await?;

    // Check if user has at least coleader permissions
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    let site_image = sqlx::query_as::<_, payloads::responses::SiteImage>(
        "INSERT INTO site_images (community_id, name, image_data, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $4)
         RETURNING *",
    )
    .bind(details.community_id)
    .bind(&details.name)
    .bind(&details.image_data)
    .bind(time_source.now().to_sqlx())
    .fetch_one(pool)
    .await?;

    Ok(site_image.id)
}

pub async fn get_site_image(
    site_image_id: &payloads::SiteImageId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::SiteImage, StoreError> {
    let site_image = sqlx::query_as::<_, payloads::responses::SiteImage>(
        "SELECT * FROM site_images WHERE id = $1",
    )
    .bind(site_image_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::SiteImageNotFound,
        e => StoreError::Database(e),
    })?;

    // Validate user is a member of the community
    let _ =
        get_validated_member(user_id, &site_image.community_id, pool).await?;

    Ok(site_image)
}

pub async fn update_site_image(
    details: &payloads::requests::UpdateSiteImage,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<payloads::responses::SiteImage, StoreError> {
    // First, get the existing site image to check permissions
    let existing_site_image =
        sqlx::query_as::<_, payloads::responses::SiteImage>(
            "SELECT * FROM site_images WHERE id = $1",
        )
        .bind(details.id)
        .fetch_one(pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StoreError::SiteImageNotFound,
            e => StoreError::Database(e),
        })?;

    // Validate user is a member of the community with coleader permissions
    let actor =
        get_validated_member(user_id, &existing_site_image.community_id, pool)
            .await?;
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    // Update the site image
    let updated_site_image =
        sqlx::query_as::<_, payloads::responses::SiteImage>(
            "UPDATE site_images
         SET name = COALESCE($2, name),
             image_data = COALESCE($3, image_data),
             updated_at = $4
         WHERE id = $1
         RETURNING *",
        )
        .bind(details.id)
        .bind(&details.name)
        .bind(&details.image_data)
        .bind(time_source.now().to_sqlx())
        .fetch_one(pool)
        .await?;

    Ok(updated_site_image)
}

pub async fn delete_site_image(
    site_image_id: &payloads::SiteImageId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    // First, get the existing site image to check permissions
    let existing_site_image =
        sqlx::query_as::<_, payloads::responses::SiteImage>(
            "SELECT * FROM site_images WHERE id = $1",
        )
        .bind(site_image_id)
        .fetch_one(pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StoreError::SiteImageNotFound,
            e => StoreError::Database(e),
        })?;

    // Validate user is a member of the community with coleader permissions
    let actor =
        get_validated_member(user_id, &existing_site_image.community_id, pool)
            .await?;
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    // Delete the site image
    sqlx::query("DELETE FROM site_images WHERE id = $1")
        .bind(site_image_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn list_site_images(
    community_id: &payloads::CommunityId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::SiteImage>, StoreError> {
    // Validate user is a member of the community
    let _ = get_validated_member(user_id, community_id, pool).await?;

    let site_images = sqlx::query_as::<_, payloads::responses::SiteImage>(
        "SELECT * FROM site_images WHERE community_id = $1 ORDER BY name",
    )
    .bind(community_id)
    .fetch_all(pool)
    .await?;

    Ok(site_images)
}
