use super::*;
use jiff_sqlx::ToSqlx;
use payloads::{
    AuctionId, AuctionRoundId, Bid, PermissionLevel, SiteId, SpaceId, UserId,
};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::time::TimeSource;

/// Calculate the total eligibility points required for a set of spaces
async fn calculate_total_eligibility_points(
    spaces: &[SpaceId],
    pool: &PgPool,
) -> Result<f64, StoreError> {
    let spaces =
        sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = ANY($1)")
            .bind(spaces)
            .fetch_all(pool)
            .await?;

    Ok(spaces.iter().map(|space| space.eligibility_points).sum())
}

/// Resolve a user's eligibility for a round into an `Eligibility`, given the
/// *prior* round's threshold (which governs this round's bids).
///
/// - Prior threshold 0.0 (or no prior round, i.e. round 0) → `Unlimited`,
///   short-circuited without querying the eligibility row.
/// - Otherwise → `Finite` of the user's eligibility row, treating a missing row
///   as 0.0 (no eligibility; can only bid zero-point spaces).
async fn user_eligibility<'e, E>(
    executor: E,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    prior_threshold: Option<f64>,
) -> Result<payloads::Eligibility, StoreError>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    match prior_threshold {
        None | Some(0.0) => Ok(payloads::Eligibility::Unlimited),
        Some(_) => {
            let row = sqlx::query_scalar::<_, f64>(
                "SELECT eligibility FROM user_eligibilities
                WHERE round_id = $1 AND user_id = $2",
            )
            .bind(round_id)
            .bind(user_id)
            .fetch_optional(executor)
            .await?;
            Ok(payloads::Eligibility::Finite(row.unwrap_or(0.0)))
        }
    }
}

/// Get a user's eligibility for a specific auction round
pub async fn get_eligibility(
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::Eligibility, StoreError> {
    // Verify the round exists and get auction info
    let round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => StoreError::Database(e),
    })?;

    // Validate user has access to this auction's community
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(round.auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    // The prior round's threshold governs this round's bids. Round 0 has no
    // prior round, so its eligibility is unconstrained.
    let prior_threshold = sqlx::query_scalar::<_, f64>(
        "SELECT eligibility_threshold FROM auction_rounds
        WHERE auction_id = $1 AND round_num = $2",
    )
    .bind(round.auction_id)
    .bind(round.round_num - 1)
    .fetch_optional(pool)
    .await?;

    user_eligibility(pool, round_id, user_id, prior_threshold).await
}

/// List a user's eligibility for every round in an auction, in round order.
/// The returned vec aligns 1:1 with the rounds: index 0 is round 0.
pub async fn list_eligibility(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::Eligibility>, StoreError> {
    // Validate user has access to this auction's community
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    // Get all rounds for this auction in order
    let rounds = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds
        WHERE auction_id = $1
        ORDER BY round_num",
    )
    .bind(auction_id)
    .fetch_all(pool)
    .await?;

    let mut eligibilities = Vec::with_capacity(rounds.len());

    // Round 0 has no prior round, so it is always unconstrained.
    if !rounds.is_empty() {
        eligibilities.push(payloads::Eligibility::Unlimited);
    }

    // Each subsequent round is interpreted against its predecessor's
    // threshold. The window iterates in pairs so `pair[0]` is the prior
    // round and `pair[1]` is the round being interpreted.
    for pair in rounds.windows(2) {
        let prior_threshold = pair[0].eligibility_threshold;
        let round = &pair[1];
        eligibilities.push(
            user_eligibility(pool, &round.id, user_id, Some(prior_threshold))
                .await?,
        );
    }

    Ok(eligibilities)
}
/// Get an auction and validate that the user has the required permission
/// level in the site's community. Returns both the auction and the
/// validated member if successful.
pub(super) async fn get_validated_auction(
    auction_id: &AuctionId,
    user_id: &UserId,
    required_permission: PermissionLevel,
    pool: &PgPool,
) -> Result<(Auction, ValidatedMember), StoreError> {
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_id)
            .fetch_one(pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => StoreError::AuctionNotFound,
                e => StoreError::Database(e),
            })?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let actor = get_validated_member(user_id, &community_id, pool).await?;

    if !required_permission.validate(actor.0.role) {
        return Err(StoreError::InsufficientPermissions {
            required: required_permission,
        });
    }

    Ok((auction, actor))
}

pub async fn create_auction(
    details: &payloads::Auction,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<payloads::AuctionId, StoreError> {
    // Get the site and validate user permissions
    let community_id = get_site_community_id(&details.site_id, pool).await?;
    let actor = get_validated_member(user_id, &community_id, pool).await?;

    if !PermissionLevel::Coleader.validate(actor.0.role) {
        return Err(StoreError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        });
    }

    // Check if the site has been deleted
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(details.site_id)
        .fetch_one(pool)
        .await?;

    if site.deleted_at.is_some() {
        return Err(StoreError::SiteDeleted);
    }

    if details.possession_start_at >= details.possession_end_at {
        return Err(StoreError::InvalidPossessionPeriod);
    }

    // A start time more than one round in the past would create round 0 already
    // ended, so nobody (human or proxy) could ever bid and the auction would
    // immediately self-conclude with no allocations. Starting exactly at now is
    // allowed: that's the immediate-start pattern used in tests.
    if details.start_at.is_some_and(|s| s < time_source.now()) {
        return Err(StoreError::AuctionStartInPast);
    }

    // Check storage limit before creating auction
    super::billing::check_storage_limit(
        pool,
        time_source,
        community_id,
        super::billing::row_estimates::AUCTION,
    )
    .await?;

    let mut tx = pool.begin().await?;

    // Create auction params first
    let auction_params_id =
        create_auction_params(&details.auction_params, &mut tx, time_source)
            .await?;

    let auction_id = sqlx::query_as::<_, Auction>(
        "INSERT INTO auctions (
            site_id,
            possession_start_at,
            possession_end_at,
            start_at,
            auction_params_id,
            created_at,
            updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $6) RETURNING *",
    )
    .bind(details.site_id)
    .bind(details.possession_start_at.to_sqlx())
    .bind(details.possession_end_at.to_sqlx())
    .bind(details.start_at.map(|t| t.to_sqlx()))
    .bind(auction_params_id)
    .bind(time_source.now().to_sqlx())
    .fetch_one(&mut *tx)
    .await?
    .id;

    tx.commit().await?;

    Ok(auction_id)
}

pub async fn read_auction(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::Auction, StoreError> {
    let (auction, _) = get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Member,
        pool,
    )
    .await?;

    let auction_params = sqlx::query_as::<_, AuctionParams>(
        "SELECT * FROM auction_params WHERE id = $1",
    )
    .bind(&auction.auction_params_id)
    .fetch_one(pool)
    .await?;

    Ok(auction.with_params(auction_params))
}

pub async fn delete_auction(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let (auction, _) = get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Coleader,
        pool,
    )
    .await?;

    // Hard deletion is only allowed after cancellation, so auctions stay
    // visible to bidders by default and settled auctions (whose journal
    // entries reference them with ON DELETE RESTRICT) are never deletable.
    if !auction.was_canceled {
        return Err(StoreError::AuctionNotCanceled);
    }

    sqlx::query("DELETE FROM auctions WHERE id = $1")
        .bind(auction_id)
        .execute(pool)
        .await?;

    tracing::info!(%auction_id, "permanently deleted canceled auction");

    Ok(())
}

/// SQL expression computing the advisory lock key that coordinates auction
/// processing between the scheduler and lifecycle mutations. `id_expr` is a
/// SQL expression yielding the auction id.
pub(crate) fn auction_processing_lock_key(id_expr: &str) -> String {
    format!("hashtextextended('auction_processing:' || {id_expr}::text, 0)")
}

/// Take the same transaction-scoped advisory lock the scheduler holds while
/// processing an auction (see `scheduler::lock_next_auction_needing_update`),
/// blocking until it's available, then re-read the auction so state checks
/// can't race round creation or settlement.
async fn lock_auction_for_update(
    auction_id: &AuctionId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Auction, StoreError> {
    sqlx::query(&format!(
        "SELECT pg_advisory_xact_lock({})",
        auction_processing_lock_key("$1")
    ))
    .bind(auction_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
        .bind(auction_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StoreError::AuctionNotFound,
            e => e.into(),
        })
}

/// Set, change, or clear the auction's scheduled start time. Only valid
/// before the auction has started.
pub async fn schedule_auction(
    details: &payloads::requests::ScheduleAuction,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let (_, _) = get_validated_auction(
        &details.auction_id,
        user_id,
        PermissionLevel::Coleader,
        pool,
    )
    .await?;

    let now = time_source.now();
    if details.start_at.is_some_and(|s| s <= now) {
        return Err(StoreError::AuctionStartNotInFuture);
    }

    let mut tx = pool.begin().await?;
    let auction = lock_auction_for_update(&details.auction_id, &mut tx).await?;

    if auction.end_at.is_some() {
        return Err(StoreError::AuctionAlreadyEnded);
    }
    // Once the start time has passed the auction is started (round 0 is
    // created within a scheduler tick), so rescheduling is refused even if
    // the round doesn't exist quite yet.
    if auction.start_at.is_some_and(|s| s <= now) {
        return Err(StoreError::AuctionAlreadyStarted);
    }

    sqlx::query(
        "UPDATE auctions SET start_at = $1, updated_at = $2 WHERE id = $3",
    )
    .bind(details.start_at.map(|t| t.to_sqlx()))
    .bind(now.to_sqlx())
    .bind(details.auction_id)
    .execute(&mut *tx)
    .await?;

    crate::pubsub::emit(
        &mut tx,
        &payloads::AuctionEvent::AuctionScheduleChanged {
            auction_id: details.auction_id,
        },
    )
    .await?;

    tx.commit().await?;

    tracing::info!(
        auction_id = %details.auction_id,
        start_at = ?details.start_at,
        "auction start time rescheduled",
    );

    Ok(())
}

/// Cancel an auction that hasn't ended yet. Sets end_at so the scheduler
/// stops processing it (no further rounds, and no settlement journal entry
/// is ever created) and was_canceled so the cancellation is visible to
/// bidders. The auction row is kept for transparency; a canceled auction
/// can be hard-deleted afterwards via `delete_auction`.
pub async fn cancel_auction(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let (_, _) = get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Coleader,
        pool,
    )
    .await?;

    let now = time_source.now();
    let mut tx = pool.begin().await?;
    // Holding the scheduler's advisory lock means we can't race a
    // concluding round's settlement: either we commit first and the
    // scheduler's `end_at IS NULL` predicate excludes the auction forever,
    // or the scheduler settles first and the re-read sees end_at set.
    let auction = lock_auction_for_update(auction_id, &mut tx).await?;

    if auction.end_at.is_some() {
        return Err(StoreError::AuctionAlreadyEnded);
    }

    sqlx::query(
        "UPDATE auctions
        SET end_at = $1, was_canceled = TRUE, updated_at = $1
        WHERE id = $2",
    )
    .bind(now.to_sqlx())
    .bind(auction_id)
    .execute(&mut *tx)
    .await?;

    crate::pubsub::emit(
        &mut tx,
        &payloads::AuctionEvent::AuctionEnded {
            auction_id: *auction_id,
        },
    )
    .await?;

    tx.commit().await?;

    tracing::info!(%auction_id, "auction canceled");

    Ok(())
}

pub async fn list_auctions(
    site_id: &SiteId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::Auction>, StoreError> {
    // Get the site and validate user permissions
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(site_id)
        .fetch_one(pool)
        .await?;

    let _ = get_validated_member(user_id, &site.community_id, pool).await?;

    let auctions = sqlx::query_as::<_, Auction>(
        "SELECT * FROM auctions WHERE site_id = $1 ORDER BY start_at DESC",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;

    // Convert each auction with its params
    let mut responses = Vec::new();
    for auction in auctions {
        let auction_params = sqlx::query_as::<_, AuctionParams>(
            "SELECT * FROM auction_params WHERE id = $1",
        )
        .bind(&auction.auction_params_id)
        .fetch_one(pool)
        .await?;

        responses.push(auction.with_params(auction_params));
    }

    Ok(responses)
}

pub async fn get_auction_round(
    round_id: &payloads::AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::AuctionRound, StoreError> {
    let round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await?;

    // Validate user has access to this auction's community
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(round.auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    Ok(round.into_response())
}

pub async fn list_auction_rounds(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::AuctionRound>, StoreError> {
    // First validate user has access to this auction's community
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    let rounds = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE auction_id = $1 ORDER BY round_num",
    )
    .bind(auction_id)
    .fetch_all(pool)
    .await?;

    Ok(rounds.into_iter().map(|r| r.into_response()).collect())
}

pub async fn get_round_space_result(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::RoundSpaceResult, StoreError> {
    // Verify user has access to the space
    get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
        .await?;

    // Fetch the round_space_result
    let db_result = sqlx::query_as::<_, RoundSpaceResult>(
        "SELECT * FROM round_space_results WHERE space_id = $1 AND round_id = $2",
    )
    .bind(space_id)
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::RoundSpaceResultNotFound,
        e => e.into(),
    })?;

    // Get the space to find its community
    let space =
        sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = $1")
            .bind(space_id)
            .fetch_one(pool)
            .await?;
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(space.site_id)
        .fetch_one(pool)
        .await?;

    // Fetch user identity
    let user_identities = get_user_identities(
        &[db_result.winning_user_id],
        &site.community_id,
        pool,
    )
    .await?;

    let winner = user_identities
        .get(&db_result.winning_user_id)
        .cloned()
        .ok_or(StoreError::UserNotFound)?;

    Ok(payloads::RoundSpaceResult {
        space_id: db_result.space_id,
        round_id: db_result.round_id,
        winner,
        value: db_result.value,
    })
}

pub async fn list_round_space_results_for_round(
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::RoundSpaceResult>, StoreError> {
    // Verify user has access to the auction round
    let auction_round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => e.into(),
    })?;

    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_round.auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    // Fetch round space results
    let db_results = sqlx::query_as::<_, RoundSpaceResult>(
        "SELECT * FROM round_space_results WHERE round_id = $1",
    )
    .bind(round_id)
    .fetch_all(pool)
    .await?;

    with_user_identities(
        db_results,
        |r| r.winning_user_id,
        |r, winner| {
            Ok(payloads::RoundSpaceResult {
                space_id: r.space_id,
                round_id: r.round_id,
                winner,
                value: r.value,
            })
        },
        &community_id,
        pool,
    )
    .await
}

pub async fn create_bid(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let mut tx = pool.begin().await?;
    create_bid_tx(space_id, round_id, user_id, &mut tx, time_source, pool)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn create_bid_tx(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    time_source: &TimeSource,
    pool: &PgPool, // for get_validated_space
) -> Result<(), StoreError> {
    // Get the space to validate user permissions and check availability
    let (space, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    // Ensure the space is available for bidding
    if !space.is_available {
        return Err(StoreError::SpaceNotAvailable);
    }

    // Check if the space has been deleted
    if space.deleted_at.is_some() {
        return Err(StoreError::SpaceDeleted);
    }

    // Check if the site has been deleted
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(space.site_id)
        .fetch_one(pool)
        .await?;

    if site.deleted_at.is_some() {
        return Err(StoreError::SiteDeleted);
    }

    // Verify the round exists and is ongoing
    let round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => StoreError::Database(e),
    })?;

    let now = time_source.now();
    if now < round.start_at {
        return Err(StoreError::RoundNotStarted);
    }
    if now >= round.end_at {
        return Err(StoreError::RoundEnded);
    }

    if round.round_num > 0 {
        let previous_round = sqlx::query_as::<_, AuctionRound>(
            "SELECT * FROM auction_rounds
            WHERE auction_id = $1 AND round_num = $2",
        )
        .bind(round.auction_id)
        .bind(round.round_num - 1)
        .fetch_one(&mut **tx)
        .await?;

        // Check if user is already the standing high bidder from the previous
        // round
        let is_winning = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
                SELECT 1 FROM round_space_results
                WHERE round_id = $1
                AND space_id = $2
                AND winning_user_id = $3
            )",
        )
        .bind(previous_round.id)
        .bind(space_id)
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await?;

        if is_winning {
            return Err(StoreError::AlreadyWinningSpace);
        }

        // Resolve the user's eligibility for this round the same way the read
        // path does (the prior round's threshold governs this round's bids).
        // Unlimited eligibility (prior threshold 0.0) needs no check, and the
        // helper skips the row query in that case.
        let eligibility = user_eligibility(
            &mut **tx,
            round_id,
            user_id,
            Some(previous_round.eligibility_threshold),
        )
        .await?;

        if let payloads::Eligibility::Finite(budget) = eligibility {
            // Get all spaces this user is currently bidding on or winning in
            // this round
            let active_spaces = sqlx::query_scalar::<_, SpaceId>(
                "SELECT space_id FROM (
                    SELECT space_id FROM bids
                    WHERE round_id = $1 AND user_id = $2
                    UNION
                    SELECT space_id FROM round_space_results rsr
                    JOIN auction_rounds ar ON rsr.round_id = ar.id
                    WHERE ar.auction_id = $3
                    AND ar.round_num = $4
                    AND winning_user_id = $2
                ) spaces",
            )
            .bind(round_id)
            .bind(user_id)
            .bind(round.auction_id)
            .bind(round.round_num - 1)
            .fetch_all(&mut **tx)
            .await?;

            // The new bid's total activity is this space plus everything the
            // user is already bidding on or winning. A zero-point space keeps
            // the total within a zero budget; positive points do not.
            let mut total_points = space.eligibility_points;
            total_points +=
                calculate_total_eligibility_points(&active_spaces, pool)
                    .await?;

            if total_points > budget {
                return Err(StoreError::ExceedsEligibility {
                    available: budget,
                    required: total_points,
                });
            }
        }
    }

    // Check credit limit before creating bid
    // Get and lock the account for this user in the community
    let account = currency::get_account_for_update_tx(
        &site.community_id,
        payloads::AccountOwner::Member(*user_id),
        tx,
    )
    .await?;

    // Get bid increment from auction params
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(round.auction_id)
            .fetch_one(&mut **tx)
            .await?;

    let auction_params = sqlx::query_as::<_, AuctionParams>(
        "SELECT * FROM auction_params WHERE id = $1",
    )
    .bind(auction.auction_params_id)
    .fetch_one(&mut **tx)
    .await?;

    // Calculate the amount this bid will lock
    // Get previous round's value for this space (if any)
    let prev_value: Option<Decimal> = if round.round_num > 0 {
        let prev_round_id: Option<payloads::AuctionRoundId> =
            sqlx::query_scalar(
                "SELECT id FROM auction_rounds
                WHERE auction_id = $1 AND round_num = $2",
            )
            .bind(round.auction_id)
            .bind(round.round_num - 1)
            .fetch_optional(&mut **tx)
            .await?;

        if let Some(prev_id) = prev_round_id {
            sqlx::query_scalar(
                "SELECT value FROM round_space_results
                WHERE round_id = $1 AND space_id = $2",
            )
            .bind(prev_id)
            .bind(space_id)
            .fetch_optional(&mut **tx)
            .await?
        } else {
            None
        }
    } else {
        None
    };

    let bid_amount = payloads::next_bid_amount(
        prev_value,
        auction_params.bid_increment,
        space.reserve_price,
    );

    // Check if user has sufficient credit for this bid. Skip when the bid
    // amount is non-positive: a chore bid doesn't put the bidder on the
    // hook for anything (and the locked-balance computation similarly
    // clamps chore bids to zero rather than treating them as freed
    // credit).
    if bid_amount > Decimal::ZERO {
        currency::check_sufficient_credit_tx(&account.id, bid_amount, tx)
            .await?;
    }

    // Create the bid
    sqlx::query(
        "INSERT INTO bids (space_id, round_id, user_id, created_at, updated_at) VALUES ($1, $2, $3, $4, $4)",
    )
    .bind(space_id)
    .bind(round_id)
    .bind(user_id)
    .bind(time_source.now().to_sqlx())
    .execute(&mut **tx)
    .await?;

    crate::pubsub::emit(
        tx,
        &payloads::AuctionEvent::BidsChanged {
            auction_id: round.auction_id,
            round_id: *round_id,
            user_id: *user_id,
        },
    )
    .await?;

    Ok(())
}

pub async fn get_bid(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Bid, StoreError> {
    // Get the space to validate user permissions
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    let bid = sqlx::query_as::<_, Bid>(
        "SELECT * FROM bids WHERE space_id = $1 AND round_id = $2 AND user_id = $3",
    )
    .bind(space_id)
    .bind(round_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::BidNotFound,
        e => StoreError::Database(e),
    })?;

    Ok(bid)
}

pub async fn list_bids(
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<Bid>, StoreError> {
    // Verify user has access to the auction round
    let auction_round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => e.into(),
    })?;

    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_round.auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    let bids = sqlx::query_as::<_, Bid>(
        "SELECT * FROM bids WHERE round_id = $1 AND user_id = $2",
    )
    .bind(round_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(bids)
}

pub async fn delete_bid(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    // Get the space to validate user permissions
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    let mut tx = pool.begin().await?;

    // Verify the round exists and is ongoing
    let round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => StoreError::Database(e),
    })?;

    let now = time_source.now();
    if now < round.start_at {
        return Err(StoreError::RoundNotStarted);
    }
    if now >= round.end_at {
        return Err(StoreError::RoundEnded);
    }

    // Delete the bid
    sqlx::query(
        "DELETE FROM bids WHERE space_id = $1 AND round_id = $2 AND user_id = $3",
    )
    .bind(space_id)
    .bind(round_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    crate::pubsub::emit(
        &mut tx,
        &payloads::AuctionEvent::BidsChanged {
            auction_id: round.auction_id,
            round_id: *round_id,
            user_id: *user_id,
        },
    )
    .await?;

    tx.commit().await?;

    Ok(())
}

pub async fn get_platform_stats(
    pool: &PgPool,
) -> Result<payloads::responses::PlatformStats, StoreError> {
    let auctions_held: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM auctions WHERE end_at IS NOT NULL",
    )
    .fetch_one(pool)
    .await?;

    let spaces_allocated: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) \
         FROM round_space_results \
         WHERE round_id IN ( \
             SELECT DISTINCT ON (ar.auction_id) ar.id \
             FROM auction_rounds ar \
             INNER JOIN auctions a ON a.id = ar.auction_id \
             WHERE a.end_at IS NOT NULL \
             ORDER BY ar.auction_id, ar.round_num DESC \
         )",
    )
    .fetch_one(pool)
    .await?;

    Ok(payloads::responses::PlatformStats {
        auctions_held,
        spaces_allocated,
    })
}
