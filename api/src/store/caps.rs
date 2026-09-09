//! Space categories and per-bidder bidding caps.
//!
//! Categories are community-scoped labels on spaces. In a `capped` auction,
//! `auction_bidder_caps` rows give each bidder a per-category ceiling on
//! active eligibility points, enforced at bid time in `create_bid_tx` in
//! every round (including round 0, unlike the eligibility activity rule). A
//! missing cap row means 0, and the NULL-category row governs uncategorized
//! spaces only: every space maps to exactly one bucket keyed by its
//! `category_id`, and there is no cross-category or total cap.
//!
//! A user's active points in a bucket are their current-round bids plus
//! their standing wins from the previous round
//! ([`active_points_by_category`]). Bids are validated when placed, so
//! already-placed bids are legitimate budget consumption regardless of how
//! they were placed (manual, built-in proxy, or an external client).
//!
//! Members can delegate cap to one another (`auction_cap_delegations`) so
//! a group can bid through one bidder. The cap the bid check enforces is
//! the *effective* cap: the assigned row, minus backed points delegated
//! out, plus backed points delegated in. A delegation is backed only as far
//! as the delegator's own assigned cap covers it and every delegation they
//! created earlier in the same bucket; received cap never backs anything,
//! so delegations do not chain. [`EFFECTIVE_CAPS_CTE`] is the one
//! derivation, shared by the bid check and the bidder's own caps view.
//!
//! Cap and delegation writes mark the affected users' proxy rows for
//! reprocessing and emit `BidderCapsChanged`, since caps change what the
//! proxy would bid and what the bidding UI shows as remaining capacity. A
//! cap write also affects everyone the user delegates to, whose effective
//! caps move with it.

use std::collections::{HashMap, HashSet};

use super::*;
use jiff_sqlx::ToSqlx;
use payloads::{
    ApiError, AuctionId, CommunityId, PermissionLevel, SpaceCategoryId, UserId,
};
use sqlx::PgPool;

use crate::time::TimeSource;

pub async fn create_space_category(
    details: &payloads::SpaceCategory,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<payloads::responses::SpaceCategory, StoreError> {
    get_validated_member_with_permission(
        user_id,
        &details.community_id,
        PermissionLevel::Coleader,
        pool,
    )
    .await?;

    let name = validate_category_name(&details.name)?;

    super::billing::check_storage_limit(
        pool,
        time_source,
        details.community_id,
        super::billing::row_estimates::SPACE_CATEGORY,
    )
    .await?;

    let category = sqlx::query_as::<_, payloads::responses::SpaceCategory>(
        "INSERT INTO space_categories (
            community_id, name, created_at, updated_at
        ) VALUES ($1, $2, $3, $3) RETURNING *",
    )
    .bind(details.community_id)
    .bind(name)
    .bind(time_source.now().to_sqlx())
    .fetch_one(pool)
    .await
    .map_err(|e| map_category_name_unique_error(e, name))?;

    Ok(category)
}

pub async fn update_space_category(
    details: &payloads::requests::UpdateSpaceCategory,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<payloads::responses::SpaceCategory, StoreError> {
    let community_id =
        get_category_community_id(&details.category_id, pool).await?;
    get_validated_member_with_permission(
        user_id,
        &community_id,
        PermissionLevel::Coleader,
        pool,
    )
    .await?;

    let name = validate_category_name(&details.name)?;

    let category = sqlx::query_as::<_, payloads::responses::SpaceCategory>(
        "UPDATE space_categories SET name = $1, updated_at = $2
        WHERE id = $3 RETURNING *",
    )
    .bind(name)
    .bind(time_source.now().to_sqlx())
    .bind(details.category_id)
    .fetch_one(pool)
    .await
    .map_err(|e| map_category_name_unique_error(e, name))?;

    Ok(category)
}

/// Delete a category. Refused while any space or cap row references it: a
/// category vanishing under a capped auction would silently change what
/// bidders may bid on. The referencing FKs carry no ON DELETE action, so
/// the DELETE itself is the in-use check; its FK violation becomes the
/// client-facing error, with no pre-check that a concurrent reference
/// could slip past.
pub async fn delete_space_category(
    category_id: &SpaceCategoryId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let community_id = get_category_community_id(category_id, pool).await?;
    get_validated_member_with_permission(
        user_id,
        &community_id,
        PermissionLevel::Coleader,
        pool,
    )
    .await?;

    sqlx::query("DELETE FROM space_categories WHERE id = $1")
        .bind(category_id)
        .execute(pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err)
                if db_err.is_foreign_key_violation() =>
            {
                ApiError::SpaceCategoryInUse.into()
            }
            e => StoreError::Database(e),
        })?;

    Ok(())
}

pub async fn list_space_categories(
    community_id: &CommunityId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::SpaceCategory>, StoreError> {
    get_validated_member(user_id, community_id, pool).await?;

    let categories = sqlx::query_as::<_, payloads::responses::SpaceCategory>(
        "SELECT * FROM space_categories WHERE community_id = $1
            ORDER BY name",
    )
    .bind(community_id)
    .fetch_all(pool)
    .await?;

    Ok(categories)
}

pub(crate) async fn get_category_community_id(
    category_id: &SpaceCategoryId,
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<CommunityId, StoreError> {
    sqlx::query_as::<_, CommunityId>(
        "SELECT community_id FROM space_categories WHERE id = $1",
    )
    .bind(category_id)
    .fetch_one(executor)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => ApiError::SpaceCategoryNotFound.into(),
        e => StoreError::Database(e),
    })
}

/// Normalize a category name for storage: trimmed, non-empty, and within
/// the length limit. A blank name would render indistinguishably from the
/// uncategorized bucket.
fn validate_category_name(name: &str) -> Result<&str, StoreError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ApiError::SpaceCategoryNameEmpty.into());
    }
    if name.len() > payloads::requests::SPACE_CATEGORY_NAME_MAX_LEN {
        return Err(ApiError::SpaceCategoryNameTooLong {
            size: name.len(),
            max: payloads::requests::SPACE_CATEGORY_NAME_MAX_LEN,
        }
        .into());
    }
    Ok(name)
}

fn map_category_name_unique_error(e: sqlx::Error, name: &str) -> StoreError {
    if let sqlx::Error::Database(db_err) = &e
        && db_err.is_unique_violation()
        && db_err.constraint() == Some("space_categories_community_id_name_key")
    {
        return ApiError::SpaceCategoryNameNotUnique {
            name: name.to_string(),
        }
        .into();
    }
    e.into()
}

/// Query prefix defining the `backed` CTE: every delegation row of
/// auction `$1` plus the points of it the delegator's assigned cap backs,
/// resolved in creation order per (delegator, bucket). Queries append
/// their SELECT and bind the auction id as `$1`.
const BACKED_DELEGATIONS_CTE: &str = "WITH backed AS (
    SELECT d.auction_id, d.from_user_id, d.to_user_id,
        d.category_id, d.points, d.created_at, d.updated_at,
        GREATEST(0::float8, LEAST(d.points,
            COALESCE(c.points, 0::float8)
            - (SUM(d.points) OVER w - d.points))) AS backed
    FROM auction_cap_delegations d
    LEFT JOIN auction_bidder_caps c
        ON c.auction_id = d.auction_id
        AND c.user_id = d.from_user_id
        AND c.category_id IS NOT DISTINCT FROM d.category_id
    WHERE d.auction_id = $1
    WINDOW w AS (
        PARTITION BY d.from_user_id, d.category_id
        ORDER BY d.created_at, d.to_user_id
        ROWS UNBOUNDED PRECEDING
    )
) ";

/// Continues [`BACKED_DELEGATIONS_CTE`] with the `effective` CTE: each
/// user's positive effective cap per bucket in auction `$1`. A missing row
/// means 0, matching the assigned-cap convention.
const EFFECTIVE_CAPS_CTE: &str = ", effective AS (
    SELECT user_id, category_id, SUM(points) AS points FROM (
        SELECT user_id, category_id, points
        FROM auction_bidder_caps WHERE auction_id = $1
        UNION ALL
        SELECT to_user_id, category_id, backed FROM backed
        UNION ALL
        SELECT from_user_id, category_id, -backed FROM backed
    ) parts
    GROUP BY user_id, category_id
    HAVING SUM(points) > 0
) ";

/// A bidder's cap state in one round of a capped auction: their effective
/// caps and their active points, each per bucket. Built by
/// [`fetch_cap_budgets`]; the one enforcement rule is
/// [`payloads::cap_permits`], applied through [`CapBudgets::check`], so
/// the bid-time check, the proxy's pre-filter, and the UI's bid gating
/// cannot diverge. The proxy [`debit`](CapBudgets::debit)s each bid it
/// places to keep the state current across its walk.
pub(crate) struct CapBudgets {
    caps: HashMap<Option<SpaceCategoryId>, f64>,
    active: HashMap<Option<SpaceCategoryId>, f64>,
}

/// A failed [`CapBudgets::check`], carrying the numbers for
/// `ApiError::ExceedsBidderCap` (the caller resolves the category name).
pub(crate) struct CapExceeded {
    pub available: f64,
    pub required: f64,
}

impl CapBudgets {
    /// Check that adding `points` in `bucket` stays within the bucket's
    /// cap ([`payloads::cap_permits`]; a missing cap row means 0).
    pub(crate) fn check(
        &self,
        bucket: Option<SpaceCategoryId>,
        points: f64,
    ) -> Result<(), CapExceeded> {
        let cap = self.caps.get(&bucket).copied();
        let active = self.active.get(&bucket).copied().unwrap_or(0.0);
        if payloads::cap_permits(cap, active, points) {
            Ok(())
        } else {
            Err(CapExceeded {
                available: cap.unwrap_or(0.0),
                required: active + points,
            })
        }
    }

    /// Record a successfully placed bid against its bucket, so subsequent
    /// checks in the same pass see it as active.
    pub(crate) fn debit(
        &mut self,
        bucket: Option<SpaceCategoryId>,
        points: f64,
    ) {
        *self.active.entry(bucket).or_insert(0.0) += points;
    }

    /// The per-bucket activity map, for reuse by the eligibility check
    /// (total activity is the sum across buckets).
    pub(crate) fn into_active(self) -> HashMap<Option<SpaceCategoryId>, f64> {
        self.active
    }
}

/// Fetch the bidder's cap state for a round, or None when the auction is
/// uncapped.
pub(crate) async fn fetch_cap_budgets(
    round: &AuctionRound,
    user_id: &UserId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Option<CapBudgets>, StoreError> {
    let capped = sqlx::query_scalar::<_, bool>(
        "SELECT capped FROM auctions WHERE id = $1",
    )
    .bind(round.auction_id)
    .fetch_one(&mut **tx)
    .await?;
    if !capped {
        return Ok(None);
    }

    let caps: Vec<(Option<SpaceCategoryId>, f64)> = sqlx::query_as(&format!(
        "{BACKED_DELEGATIONS_CTE}{EFFECTIVE_CAPS_CTE}
        SELECT category_id, points FROM effective WHERE user_id = $2"
    ))
    .bind(round.auction_id)
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?;

    let active = active_points_by_category(round, user_id, tx).await?;

    Ok(Some(CapBudgets {
        caps: caps.into_iter().collect(),
        active,
    }))
}

/// The user's active eligibility points per category bucket in a round:
/// their current-round bids plus their standing wins from the previous
/// round (`round_num - 1` matches nothing in round 0). This is the full
/// activity definition regardless of how the bids were placed — manual,
/// built-in proxy, or an external client — since every placed bid passed
/// validation and is legitimate budget consumption. The eligibility
/// check's total activity is the sum across buckets, so this one query
/// serves both the cap and eligibility checks.
pub(crate) async fn active_points_by_category(
    round: &AuctionRound,
    user_id: &UserId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<HashMap<Option<SpaceCategoryId>, f64>, StoreError> {
    let active: Vec<(Option<SpaceCategoryId>, f64)> = sqlx::query_as(
        "SELECT s.category_id, SUM(s.eligibility_points) FROM (
            SELECT space_id FROM bids
            WHERE round_id = $1 AND user_id = $2
            UNION
            SELECT rsr.space_id FROM round_space_results rsr
            JOIN auction_rounds ar ON rsr.round_id = ar.id
            WHERE ar.auction_id = $3
            AND ar.round_num = $4
            AND winning_user_id = $2
        ) active
        JOIN spaces s ON s.id = active.space_id
        GROUP BY s.category_id",
    )
    .bind(round.id)
    .bind(user_id)
    .bind(round.auction_id)
    .bind(round.round_num - 1)
    .fetch_all(&mut **tx)
    .await?;

    Ok(active.into_iter().collect())
}

/// Upsert one cap row (coleader+); 0 points deletes the row instead, since
/// a 0-points row and a missing row mean the same thing at bid time. The
/// auction must be capped; the category (if any) must belong to the
/// auction's community; the target user must be a member.
pub async fn set_bidder_cap(
    details: &payloads::requests::SetBidderCap,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let (auction, community_id) =
        validate_capped_auction_coleader(&details.auction_id, user_id, pool)
            .await?;

    if !details.points.is_finite() || details.points < 0.0 {
        return Err(ApiError::InvalidCapPoints.into());
    }
    if let Some(category_id) = &details.category_id
        && get_category_community_id(category_id, pool).await? != community_id
    {
        return Err(ApiError::SpaceCategoryCommunityMismatch.into());
    }
    get_validated_member(&details.user_id, &community_id, pool).await?;

    let mut tx = pool.begin().await?;

    let changed = if details.points == 0.0 {
        sqlx::query(
            "DELETE FROM auction_bidder_caps
            WHERE auction_id = $1 AND user_id = $2
                AND category_id IS NOT DISTINCT FROM $3",
        )
        .bind(details.auction_id)
        .bind(details.user_id)
        .bind(details.category_id)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0
    } else {
        sqlx::query(
            "INSERT INTO auction_bidder_caps (
                auction_id, user_id, category_id, points,
                created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $5)
            ON CONFLICT (auction_id, user_id, category_id)
            DO UPDATE SET points = EXCLUDED.points,
                updated_at = EXCLUDED.updated_at",
        )
        .bind(details.auction_id)
        .bind(details.user_id)
        .bind(details.category_id)
        .bind(details.points)
        .bind(time_source.now().to_sqlx())
        .execute(&mut *tx)
        .await?;
        true
    };

    if changed {
        flag_proxy_rows_and_emit(&auction, &[details.user_id], &mut tx).await?;
    }

    tx.commit().await?;

    if changed {
        tracing::info!(
            auction_id = %details.auction_id,
            user_id = %details.user_id,
            category_id = ?details.category_id,
            points = details.points,
            "bidder cap set",
        );
    }

    Ok(())
}

/// Set every active member's cap in one bucket (coleader+), the bulk form
/// of [`set_bidder_cap`] for communities where everyone gets the same
/// entitlement. 0 points deletes their rows. Inactive members are left
/// untouched either way, mirroring allowance issuance.
pub async fn set_bidder_cap_for_all(
    details: &payloads::requests::SetBidderCapForAll,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let (auction, community_id) =
        validate_capped_auction_coleader(&details.auction_id, user_id, pool)
            .await?;

    if !details.points.is_finite() || details.points < 0.0 {
        return Err(ApiError::InvalidCapPoints.into());
    }
    if let Some(category_id) = &details.category_id
        && get_category_community_id(category_id, pool).await? != community_id
    {
        return Err(ApiError::SpaceCategoryCommunityMismatch.into());
    }

    let mut tx = pool.begin().await?;

    let affected_users: Vec<UserId> = if details.points == 0.0 {
        sqlx::query_scalar(
            "DELETE FROM auction_bidder_caps
            WHERE auction_id = $1
                AND category_id IS NOT DISTINCT FROM $2
                AND user_id IN (
                    SELECT user_id FROM community_members
                    WHERE community_id = $3 AND is_active
                )
            RETURNING user_id",
        )
        .bind(details.auction_id)
        .bind(details.category_id)
        .bind(community_id)
        .fetch_all(&mut *tx)
        .await?
    } else {
        sqlx::query_scalar(
            "INSERT INTO auction_bidder_caps (
                auction_id, user_id, category_id, points,
                created_at, updated_at
            )
            SELECT $1, user_id, $2, $3, $4, $4 FROM community_members
            WHERE community_id = $5 AND is_active
            ON CONFLICT (auction_id, user_id, category_id)
            DO UPDATE SET points = EXCLUDED.points,
                updated_at = EXCLUDED.updated_at
            RETURNING user_id",
        )
        .bind(details.auction_id)
        .bind(details.category_id)
        .bind(details.points)
        .bind(time_source.now().to_sqlx())
        .bind(community_id)
        .fetch_all(&mut *tx)
        .await?
    };

    flag_proxy_rows_and_emit(&auction, &affected_users, &mut tx).await?;

    tx.commit().await?;

    tracing::info!(
        auction_id = %details.auction_id,
        category_id = ?details.category_id,
        points = details.points,
        users = affected_users.len(),
        "bidder cap set for all active members",
    );

    Ok(())
}

/// All cap rows of the auction (coleader+), for the cap editor. These are
/// the assigned rows, not effective caps: delegation conserves the total,
/// so the editor's view is what a coleader reasons about.
pub async fn list_bidder_caps(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::BidderCap>, StoreError> {
    get_validated_auction(auction_id, user_id, PermissionLevel::Coleader, pool)
        .await?;

    let caps = sqlx::query_as::<_, payloads::responses::BidderCap>(
        "SELECT * FROM auction_bidder_caps WHERE auction_id = $1
        ORDER BY user_id, category_id NULLS FIRST",
    )
    .bind(auction_id)
    .fetch_all(pool)
    .await?;

    Ok(caps)
}

/// The requesting user's effective caps in the auction, for the bidding
/// view's remaining-capacity display and bid gating.
pub async fn my_bidder_caps(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::EffectiveCap>, StoreError> {
    get_validated_auction(auction_id, user_id, PermissionLevel::Member, pool)
        .await?;

    let caps =
        sqlx::query_as::<_, payloads::responses::EffectiveCap>(&format!(
            "{BACKED_DELEGATIONS_CTE}{EFFECTIVE_CAPS_CTE}
        SELECT category_id, points FROM effective WHERE user_id = $2
        ORDER BY category_id NULLS FIRST"
        ))
        .bind(auction_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;

    Ok(caps)
}

/// Upsert or delete (0 points) one of the acting user's own delegations.
/// Delegations are delegator-owned: received cap obligates nothing and
/// can't be passed on, so nobody else needs write access, and coleaders
/// override one through the delegator's cap. Allowed only before the
/// auction starts, since a delegation moving cap under placed bids would
/// leave activity above the cap. Not checked against the delegator's
/// assigned cap: a delegation may be recorded before the cap is, and
/// simply stays unbacked until it is.
pub async fn set_cap_delegation(
    details: &payloads::requests::SetCapDelegation,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let (auction, _) = get_validated_auction(
        &details.auction_id,
        user_id,
        PermissionLevel::Member,
        pool,
    )
    .await?;
    if !auction.capped {
        return Err(ApiError::AuctionNotCapped.into());
    }
    if auction.end_at.is_some() {
        return Err(ApiError::CapsFrozenAfterConclusion.into());
    }
    if auction.has_started(time_source.now()) {
        return Err(ApiError::CapDelegationsFrozenAfterStart.into());
    }

    if !details.points.is_finite() || details.points < 0.0 {
        return Err(ApiError::InvalidCapPoints.into());
    }
    if *user_id == details.to_user_id {
        return Err(ApiError::CapDelegationToSelf.into());
    }

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    if let Some(category_id) = &details.category_id
        && get_category_community_id(category_id, pool).await? != community_id
    {
        return Err(ApiError::SpaceCategoryCommunityMismatch.into());
    }
    get_validated_member(&details.to_user_id, &community_id, pool).await?;

    let mut tx = pool.begin().await?;

    let changed = if details.points == 0.0 {
        sqlx::query(
            "DELETE FROM auction_cap_delegations
            WHERE auction_id = $1 AND from_user_id = $2 AND to_user_id = $3
                AND category_id IS NOT DISTINCT FROM $4",
        )
        .bind(details.auction_id)
        .bind(user_id)
        .bind(details.to_user_id)
        .bind(details.category_id)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0
    } else {
        sqlx::query(
            "INSERT INTO auction_cap_delegations (
                auction_id, from_user_id, to_user_id, category_id, points,
                created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $6)
            ON CONFLICT (auction_id, from_user_id, to_user_id, category_id)
            DO UPDATE SET points = EXCLUDED.points,
                updated_at = EXCLUDED.updated_at",
        )
        .bind(details.auction_id)
        .bind(user_id)
        .bind(details.to_user_id)
        .bind(details.category_id)
        .bind(details.points)
        .bind(time_source.now().to_sqlx())
        .execute(&mut *tx)
        .await?;
        true
    };

    if changed {
        // The delegator's other recipients are covered by the helper's
        // fan-out: changing one delegation can shift which later ones
        // are backed.
        flag_proxy_rows_and_emit(
            &auction,
            &[*user_id, details.to_user_id],
            &mut tx,
        )
        .await?;
    }

    tx.commit().await?;

    if changed {
        tracing::info!(
            auction_id = %details.auction_id,
            from_user_id = %user_id,
            to_user_id = %details.to_user_id,
            category_id = ?details.category_id,
            points = details.points,
            "cap delegation set",
        );
    }

    Ok(())
}

/// All delegations of the auction with backing (coleader+), read-only.
pub async fn list_cap_delegations(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::CapDelegation>, StoreError> {
    get_validated_auction(auction_id, user_id, PermissionLevel::Coleader, pool)
        .await?;
    list_delegations(auction_id, None, pool).await
}

/// The delegations the requesting user gave or received in the auction.
pub async fn my_cap_delegations(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::CapDelegation>, StoreError> {
    get_validated_auction(auction_id, user_id, PermissionLevel::Member, pool)
        .await?;
    list_delegations(auction_id, Some(user_id), pool).await
}

async fn list_delegations(
    auction_id: &AuctionId,
    party: Option<&UserId>,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::CapDelegation>, StoreError> {
    let delegations =
        sqlx::query_as::<_, payloads::responses::CapDelegation>(&format!(
            "{BACKED_DELEGATIONS_CTE}
            SELECT * FROM backed
            WHERE $2::uuid IS NULL OR from_user_id = $2 OR to_user_id = $2
            ORDER BY created_at, to_user_id"
        ))
        .bind(auction_id)
        .bind(party.copied())
        .fetch_all(pool)
        .await?;

    Ok(delegations)
}

/// Add each winner's final-round points per category in the concluded
/// source auction to their caps in the target auction (coleader+).
/// Additive because overlap is expected: a pre-committed cap of 1 plus a
/// won item should end at 2. There is no provenance record; at the scales
/// caps are used at, the cap list itself makes a wrong or double apply
/// evident, and the cap editor is the correction path.
pub async fn seed_bidder_caps(
    details: &payloads::requests::SeedBidderCaps,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let (target, community_id) = validate_capped_auction_coleader(
        &details.target_auction_id,
        user_id,
        pool,
    )
    .await?;

    let source =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(details.source_auction_id)
            .fetch_one(pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => {
                    StoreError::Api(ApiError::AuctionNotFound)
                }
                e => StoreError::Database(e),
            })?;

    if get_site_community_id(&source.site_id, pool).await? != community_id {
        return Err(ApiError::SeedSourceCommunityMismatch.into());
    }
    // Cancellation also sets end_at, so check both.
    if source.end_at.is_none() || source.was_canceled {
        return Err(ApiError::SeedSourceNotConcluded.into());
    }

    let mut tx = pool.begin().await?;

    // One additive upsert over the source's final-round winners, grouped
    // by winner and category. Zero-point sums are dropped so the table
    // never holds 0-points rows (a missing row already means 0). Winners
    // who have since left the community are silently skipped, matching
    // set_bidder_cap's membership requirement. RETURNING gives the
    // affected users for proxy reprocessing and cap-change events.
    let affected_users = sqlx::query_scalar::<_, UserId>(
        "INSERT INTO auction_bidder_caps (
            auction_id, user_id, category_id, points, created_at, updated_at
        )
        SELECT $1, rsr.winning_user_id, s.category_id,
            SUM(s.eligibility_points), $3, $3
        FROM round_space_results rsr
        JOIN auction_rounds ar ON rsr.round_id = ar.id
        JOIN spaces s ON rsr.space_id = s.id
        JOIN community_members cm
            ON cm.community_id = $4
            AND cm.user_id = rsr.winning_user_id
        WHERE ar.id = (
            SELECT id FROM auction_rounds
            WHERE auction_id = $2
            ORDER BY round_num DESC LIMIT 1
        )
        GROUP BY rsr.winning_user_id, s.category_id
        HAVING SUM(s.eligibility_points) > 0
        ON CONFLICT (auction_id, user_id, category_id)
        DO UPDATE SET
            points = auction_bidder_caps.points + EXCLUDED.points,
            updated_at = EXCLUDED.updated_at
        RETURNING user_id",
    )
    .bind(details.target_auction_id)
    .bind(details.source_auction_id)
    .bind(time_source.now().to_sqlx())
    .bind(community_id)
    .fetch_all(&mut *tx)
    .await?;

    flag_proxy_rows_and_emit(&target, &affected_users, &mut tx).await?;

    tx.commit().await?;

    tracing::info!(
        target_auction_id = %details.target_auction_id,
        source_auction_id = %details.source_auction_id,
        cap_rows = affected_users.len(),
        "bidder caps seeded from source auction results",
    );

    Ok(())
}

/// Shared validation for cap-writing endpoints: the actor is a coleader of
/// the auction's community, the auction uses caps, and it has not ended.
/// Returns the auction and its community id.
async fn validate_capped_auction_coleader(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(Auction, CommunityId), StoreError> {
    let (auction, _) = get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Coleader,
        pool,
    )
    .await?;
    if !auction.capped {
        return Err(ApiError::AuctionNotCapped.into());
    }
    // Cap rows are the historical record of what governed a concluded
    // auction; post-conclusion edits would silently corrupt it. end_at
    // covers both terminal states (cancellation also sets it).
    if auction.end_at.is_some() {
        return Err(ApiError::CapsFrozenAfterConclusion.into());
    }
    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    Ok((auction, community_id))
}

/// Mark the users' proxy rows in the auction dirty (open auctions only)
/// and emit a per-user `BidderCapsChanged`, in the caller's cap-writing
/// transaction. The set is widened to everyone the given users delegate
/// to, since their effective caps move with the delegator's cap; callers
/// may pass duplicates. Setting the flags in the writer's own tx makes
/// proxy re-selection immune to writes that straddle the processor's
/// read, mirroring `flag_proxy_rows_for_space`. The flags are one batched
/// UPDATE; the emits stay per-user because the event payload is.
async fn flag_proxy_rows_and_emit(
    auction: &Auction,
    user_ids: &[UserId],
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    let recipients: Vec<UserId> = sqlx::query_scalar(
        "SELECT DISTINCT to_user_id FROM auction_cap_delegations
        WHERE auction_id = $1 AND from_user_id = ANY($2)",
    )
    .bind(auction.id)
    .bind(user_ids)
    .fetch_all(&mut **tx)
    .await?;
    let user_ids: Vec<UserId> = user_ids
        .iter()
        .chain(&recipients)
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    if auction.end_at.is_none() {
        sqlx::query(
            "UPDATE use_proxy_bidding SET needs_processing = TRUE
            WHERE auction_id = $1 AND user_id = ANY($2)",
        )
        .bind(auction.id)
        .bind(&user_ids)
        .execute(&mut **tx)
        .await?;
    }

    for user_id in &user_ids {
        crate::pubsub::emit(
            tx,
            &payloads::AuctionEvent::BidderCapsChanged {
                auction_id: auction.id,
                user_id: *user_id,
            },
        )
        .await?;
    }

    Ok(())
}
