//! Top-level orchestration of time-based triggers and scheduling.
//!
//! E.g. on the proxy bidding lead time is reached and auto scheduling is
//! enabled, the scheduler creates the auction row so proxy bids can start to be
//! associated with it. Other scheduling tasks include, starting the auction,
//! computing auction rounds, and updating members' is_active state based on the
//! membership schedule.
//!
//! ```text
//!          round_duration
//!                v
//! |------------|---|---|---|---| < auction concluded by setting end_at
//!       ^      ^   ^
//!       |      |   round concludes, round_space_results updates with results,
//!       |      |   new rounds are created if there is still activity
//!       |      |
//!       | auction start
//!       |
//! proxy_bidding_lead_time
//!
//!
//! auction start
//! update round results (skipped)
//! update user eligibilities (skipped)
//! create next round
//!
//! round concludes
//! update round results
//! update user eligibilities
//! create next round
//!
//! round concludes
//! update round results; auction concluded
//! ```

use anyhow::Context;
use jiff::tz::TimeZone;
use jiff_sqlx::ToSqlx;
use payloads::SpaceId;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time;

use crate::{store, telemetry::log_error, time::TimeSource};

pub struct Scheduler {
    pool: PgPool,
    time_source: TimeSource,
    tick_interval: Duration,
}

impl Scheduler {
    pub fn new(
        pool: PgPool,
        time_source: TimeSource,
        tick_interval: Duration,
    ) -> Self {
        Self {
            pool,
            time_source,
            tick_interval,
        }
    }

    pub async fn run(&self) {
        let mut interval = time::interval(self.tick_interval);
        loop {
            interval.tick().await;
            let _ = schedule_tick(&self.pool, &self.time_source)
                .await
                .map_err(log_error);
        }
    }
}

/// Update state once right now.
#[tracing::instrument(skip(pool, time_source))]
pub async fn schedule_tick(
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    // Update active states from schedule
    // TODO: revisit this after MVP
    // let _ = store::update_is_active_from_schedule(pool, time_source)
    // .await
    // .map_err(log_error);

    // Process auctions without ongoing rounds
    process_auctions_without_rounds(pool, time_source).await?;

    // Process proxy bidding for active rounds
    process_proxy_bidding_for_active_rounds(pool, time_source).await?;

    Ok(())
}

/// Process all auctions that don't have ongoing rounds sequentially.
/// Uses row-level locking to prevent concurrent processing by multiple
/// scheduler instances.
#[tracing::instrument(skip(pool, time_source))]
async fn process_auctions_without_rounds(
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    loop {
        match process_next_auction(pool, time_source).await {
            Ok(true) => continue, // Processed one, try for more
            Ok(false) => break,   // No more auctions to process
            Err(e) => {
                // Log error but continue to next auction
                tracing::error!("Failed to process auction: {:#}", e);
                continue;
            }
        }
    }
    Ok(())
}

/// Lock and process the next auction that needs updating.
/// Returns Ok(true) if an auction was processed, Ok(false) if no auctions
/// available.
#[tracing::instrument(skip(pool, time_source))]
async fn process_next_auction(
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<bool> {
    // This transaction is ONLY used to hold the advisory lock for coordination.
    // It prevents re-entry by other scheduler instances.
    // No other database operations should be attached to this transaction.
    let mut coordination_tx = pool.begin().await?;

    // Lock one auction atomically using advisory lock
    let auction = match lock_next_auction_needing_update(
        &mut coordination_tx,
        time_source,
    )
    .await?
    {
        Some(a) => a,
        None => return Ok(false), // No auctions available
    };

    let auction_id = auction.id;

    // Process the auction in its own transaction (not the coordination_tx)
    match process_locked_auction(&auction, pool, time_source).await {
        Ok(()) => {
            // Success - commit the coordination transaction to release the lock
            coordination_tx.commit().await?;
            Ok(true)
        }
        Err(e) => {
            // Record the failure in a separate transaction before releasing lock
            let _ = handle_auction_processing_failure(
                auction_id,
                pool,
                time_source,
            )
            .await
            .context("Failed to record auction failure")
            .map_err(log_error);

            // Commit the coordination transaction to release the lock
            // (failure has been recorded in separate transaction above)
            let _ = coordination_tx.commit().await;

            Err(e)
        }
    }
}

/// Lock the next auction that needs updating, using advisory locks to prevent
/// blocking and enable concurrent scheduler instances.
/// Uses exponential backoff to avoid repeatedly processing failing auctions.
///
/// Auctions that need processing are those that are still ongoing (the
/// start_at is past and end_at is NULL) and which do not have an ongoing round
/// (now < end_at for any round).
#[tracing::instrument(skip(tx, time_source))]
async fn lock_next_auction_needing_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    time_source: &TimeSource,
) -> anyhow::Result<Option<store::Auction>> {
    // Exponential backoff: 5 minutes * 2^failure_count, capped at 5 failures
    // (which gives max backoff of ~2.5 hours)
    sqlx::query_as::<_, store::Auction>(
        "SELECT auctions.* FROM auctions
        JOIN sites ON auctions.site_id = sites.id
        WHERE sites.deleted_at IS NULL
            AND $1 >= start_at
            AND end_at IS NULL
            AND NOT EXISTS (
                SELECT 1 FROM auction_rounds
                WHERE auction_id = auctions.id
                AND $1 < end_at
            )
            AND (
                scheduler_failure_count = 0
                OR scheduler_last_failed_at IS NULL
                OR $1 > scheduler_last_failed_at +
                    INTERVAL '5 minutes' * POW(2, LEAST(scheduler_failure_count, 5))
            )
            -- Try to take a transaction-scoped advisory lock for this auction
            AND pg_try_advisory_xact_lock(
                hashtextextended('auction_processing:' || auctions.id::text, 0)
            )
        ORDER BY random()
        LIMIT 1",
    )
    .bind(time_source.now().to_sqlx())
    .fetch_optional(&mut **tx)
    .await
    .map_err(Into::into)
}

/// Record a failure to process an auction.
#[tracing::instrument(skip(pool, time_source))]
async fn handle_auction_processing_failure(
    auction_id: payloads::AuctionId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE auctions
        SET scheduler_failure_count = scheduler_failure_count + 1,
            scheduler_last_failed_at = $1
        WHERE id = $2",
    )
    .bind(time_source.now().to_sqlx())
    .bind(auction_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Process a locked auction in its own transaction.
/// This function handles updating results for previous rounds and creating new
/// rounds.
#[tracing::instrument(skip(pool, auction, time_source))]
async fn process_locked_auction(
    auction: &store::Auction,
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    // Create a new transaction for the actual processing work
    let mut tx = pool.begin().await?;

    let previous_round = sqlx::query_as::<_, store::AuctionRound>(
        "SELECT * FROM auction_rounds
        WHERE auction_id = $1
        ORDER BY round_num DESC
        LIMIT 1",
    )
    .bind(auction.id)
    .fetch_optional(&mut *tx)
    .await
    .context("failed to query for concluded round")?;

    // If there's a previous round, update its results and check if auction
    // concluded
    if let Some(ref previous_round) = previous_round {
        let auction_continues = update_round_space_results_within_tx(
            auction,
            previous_round,
            &mut tx,
            time_source,
        )
        .await?;

        if !auction_continues {
            // Auction has concluded, no more rounds to create
            tx.commit().await?;
            return Ok(());
        }
    }

    // Create next round
    let new_round_id = add_subsequent_rounds_for_auction(
        auction,
        &previous_round,
        &mut tx,
        time_source,
    )
    .await?;

    // Update eligibilities only if there was a previous round
    if let Some(ref previous_round) = previous_round {
        update_user_eligibilities(
            auction,
            previous_round,
            &new_round_id,
            &mut tx,
        )
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// For rounds that have concluded (now > end_time), create an entry for each
/// space for that round defining the current value of the space (0.0 by
/// default), and the user_id of the current highest bidder.
///
/// Bids are just whether someone wants the space at the previous value plus
/// the bid increment (from the auction params), and are defined in the bids
/// table. When there are multiple bids for the same space, the winner is
/// selected at random for this round.
///
/// Bids in the bids table are assumed to already have sufficient eligibility
/// and are considered valid.
///
/// If all space values remain the same in a new round, the auction is
/// concluded by defining end_at in the auction table with the current time.
///
/// Returns whether the auction is still ongoing.
#[tracing::instrument(skip(tx, time_source))]
async fn update_round_space_results_within_tx(
    auction: &store::Auction,
    previous_round: &store::AuctionRound,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    time_source: &TimeSource,
) -> anyhow::Result<bool> {
    // Get the auction params to know the bid increment
    let auction_params = sqlx::query_as::<_, store::AuctionParams>(
        "SELECT * FROM auction_params WHERE id = $1",
    )
    .bind(&auction.auction_params_id)
    .fetch_one(&mut **tx)
    .await
    .context("failed to get auction params")?;

    // Get all spaces for this auction's site
    let spaces = sqlx::query_as::<_, store::Space>(
        "SELECT * FROM spaces WHERE site_id = $1 AND is_available = true AND deleted_at IS NULL",
    )
    .bind(auction.site_id)
    .fetch_all(&mut **tx)
    .await
    .context("failed to get available spaces for site")?;

    let mut any_bids = false;
    // Collect winner payments for settlement (user_id -> total amount owed)
    let mut winner_payments: HashMap<payloads::UserId, Decimal> =
        HashMap::new();

    for space in &spaces {
        // Check how many bids exist for this space in the concluded round
        let bid_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM bids
            WHERE space_id = $1 AND round_id = $2",
        )
        .bind(space.id)
        .bind(previous_round.id)
        .fetch_one(&mut **tx)
        .await
        .with_context(|| {
            format!("failed to get bid count for space {}", space.id)
        })?;

        // Track if there are any bids
        any_bids = any_bids || bid_count > 0;

        // Get previous value if it exists
        let prev_result = sqlx::query_as::<_, store::RoundSpaceResult>(
            "SELECT * FROM round_space_results
            WHERE space_id = $1
            AND round_id IN (
                SELECT id FROM auction_rounds
                WHERE auction_id = $2
                AND round_num < $3
            )
            ORDER BY (
                SELECT round_num FROM auction_rounds
                WHERE id = round_id
            ) DESC
            LIMIT 1",
        )
        .bind(space.id)
        .bind(auction.id)
        .bind(previous_round.round_num)
        .fetch_optional(&mut **tx)
        .await
        .with_context(|| {
            format!("failed to get previous value for space {}", space.id)
        })?;

        let (new_value, winning_user_id) = if bid_count > 0 {
            // With any bids, increase the value if there was a previous value
            let winner = sqlx::query_scalar::<_, payloads::UserId>(
                "SELECT user_id FROM bids
                WHERE space_id = $1 AND round_id = $2
                ORDER BY random()
                LIMIT 1",
            )
            .bind(space.id)
            .bind(previous_round.id)
            .fetch_one(&mut **tx)
            .await
            .with_context(|| {
                format!("failed to select winning bid for space {}", space.id)
            })?;

            let new_value = match prev_result {
                Some(prev) => prev.value + auction_params.bid_increment,
                None => rust_decimal::Decimal::ZERO, // Start at zero in first round
            };

            (new_value, winner)
        } else {
            match prev_result {
                // No new bids, keep the same value and winner
                Some(result) => (result.value, result.winning_user_id),
                // No previous winner, skip creating a round_space_result entry
                // entirely (no activity yet)
                None => continue,
            }
        };

        // Create space round entry
        sqlx::query(
            "INSERT INTO round_space_results (
                space_id,
                round_id,
                winning_user_id,
                value
            ) VALUES ($1, $2, $3, $4)",
        )
        .bind(space.id)
        .bind(previous_round.id)
        .bind(winning_user_id)
        .bind(new_value)
        .execute(&mut **tx)
        .await
        .with_context(|| {
            format!("failed to create space round entry for space {}", space.id)
        })?;

        // Accumulate payment owed by this winner
        *winner_payments
            .entry(winning_user_id)
            .or_insert(Decimal::ZERO) += new_value;
    }

    // Conclude the auction if there are no more bids
    if !any_bids {
        sqlx::query(
            "UPDATE auctions
            SET end_at = $1
            WHERE id = $2",
        )
        .bind(previous_round.end_at.to_sqlx())
        .bind(auction.id)
        .execute(&mut **tx)
        .await
        .with_context(|| {
            format!("failed to conclude auction {}", auction.id)
        })?;

        // Get community_id from site for settlement
        let community_id: payloads::CommunityId =
            sqlx::query_scalar("SELECT community_id FROM sites WHERE id = $1")
                .bind(auction.site_id)
                .fetch_one(&mut **tx)
                .await
                .context("failed to get community_id for auction settlement")?;

        // Create auction settlement journal entry
        store::currency::create_auction_settlement_entry(
            &community_id,
            &auction.id,
            winner_payments,
            time_source,
            tx,
        )
        .await
        .context("failed to create auction settlement journal entry")?;
    }

    Ok(any_bids)
}

/// For an in-progress auction, create the next auction round as needed.
#[tracing::instrument(skip(tx))]
pub async fn add_subsequent_rounds_for_auction(
    auction: &store::Auction,
    previous_round: &Option<store::AuctionRound>,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    time_source: &TimeSource,
) -> anyhow::Result<payloads::AuctionRoundId> {
    let auction_params = sqlx::query_as::<_, store::AuctionParams>(
        "SELECT * FROM auction_params WHERE id = $1",
    )
    .bind(&auction.auction_params_id)
    .fetch_one(&mut **tx)
    .await
    .context("getting auction params; skipping")?;

    let timezone =
        sqlx::query_as::<_, store::Site>("SELECT * FROM sites where id = $1")
            .bind(auction.site_id)
            .fetch_one(&mut **tx)
            .await
            .context("getting site; skipping")?
            .timezone;

    let start_time_ts = previous_round
        .as_ref()
        .map(|r| r.end_at)
        .unwrap_or(auction.start_at);

    // use DST-aware datetime math in case the round duration is days or
    // larger
    let zoned_start_time = match start_time_ts
        .in_tz(&timezone.unwrap_or("UTC".into()))
        .context("converting to timezone; falling back to DST-naive")
    {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("{e:#}");
            auction.start_at.to_zoned(TimeZone::UTC)
        }
    };

    let zoned_end_time = zoned_start_time
        .checked_add(auction_params.round_duration)
        .context("computing round end time; skipping")?;

    let round_num: i32 = previous_round
        .as_ref()
        .map(|r| r.round_num + 1)
        .unwrap_or(0);

    let eligibility_threshold = get_eligibility_for_round_num(
        round_num,
        &auction_params.activity_rule_params.eligibility_progression,
    );

    let new_round = sqlx::query_as::<_, store::AuctionRound>(
        "INSERT INTO auction_rounds (
            auction_id,
            round_num,
            start_at,
            end_at,
            eligibility_threshold,
            created_at,
            updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $6)
        RETURNING *",
    )
    .bind(auction.id)
    .bind(round_num)
    .bind(start_time_ts.to_sqlx())
    .bind(zoned_end_time.timestamp().to_sqlx())
    .bind(eligibility_threshold)
    .bind(time_source.now().to_sqlx())
    .fetch_one(&mut **tx)
    .await
    .context("inserting round into database")?;

    Ok(new_round.id)
}

/// Update user eligibilities after an auction round concludes.
///
/// In each round, eligibility is based on two factors:
/// 1. New bids placed in the just-concluded round
/// 2. Standing high bids from the round before that
///
/// This accounts for the natural alternating pattern of bidding where bidders don't
/// need to rebid on spaces they're already winning. For example:
///
/// Round X-1:
/// - Bidder A bids on a space
/// - Round concludes, A becomes high bidder
///
/// Round X:
/// - Bidder A doesn't need to bid (they're winning from X-1)
/// - Bidder B bids to take the lead
/// - Round concludes, B becomes high bidder
///
/// Round X+1:
/// - Bidder B doesn't need to bid (they're winning from X)
/// - Bidder A bids to take back the lead
/// - Round concludes, A becomes high bidder
///
/// When calculating eligibility for round X+1, we need to count:
/// - New bids placed in round X
/// - Standing high bids from round X-1
/// This ensures bidders maintain eligibility even in rounds where they don't need
/// to place new bids because they're already winning from the previous round.
///
/// The eligibility calculation takes the total eligibility points from these
/// spaces and divides by the eligibility threshold. For example, if the
/// threshold is 0.5 (50%), and a user has activity on spaces worth 10 points,
/// their eligibility is set to 20 points (10 / 0.5).
///
/// After the first round, eligibility cannot increase. For example, if a user
/// has 20 points of eligibility after round 1:
/// - If they bid on 15 points of spaces in round 2, eligibility stays at 20
/// - If they bid on 5 points of spaces in round 2, eligibility drops to 10 (5 / 0.5)
#[tracing::instrument(skip(tx))]
pub async fn update_user_eligibilities(
    auction: &store::Auction,
    previous_round: &store::AuctionRound,
    new_round_id: &payloads::AuctionRoundId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> anyhow::Result<()> {
    // Get all spaces for this auction's site to calculate eligibility points
    let spaces = sqlx::query_as::<_, store::Space>(
        "SELECT * FROM spaces WHERE site_id = $1 AND is_available = true AND deleted_at IS NULL",
    )
    .bind(auction.site_id)
    .fetch_all(&mut **tx)
    .await
    .context("failed to get available spaces for site")?;

    // Get all users who either bid in the previous round or had a winning bid in the round before that
    let bidding_users = sqlx::query_scalar::<_, payloads::UserId>(
        "SELECT DISTINCT user_id 
         FROM (
             SELECT user_id FROM bids WHERE round_id = $1
             UNION
             SELECT winning_user_id FROM round_space_results rsr
             JOIN auction_rounds ar ON rsr.round_id = ar.id
             WHERE ar.auction_id = $2 
             AND ar.round_num = $3 
             AND winning_user_id IS NOT NULL
         ) users",
    )
    .bind(previous_round.id)
    .bind(auction.id)
    .bind(previous_round.round_num - 1)
    .fetch_all(&mut **tx)
    .await
    .context("failed to get users who bid or had standing high bids")?;

    for user_id in bidding_users {
        // Get all spaces this user bid on in the previous round OR was winning from two rounds ago
        let active_spaces = sqlx::query_scalar::<_, payloads::SpaceId>(
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
        .bind(previous_round.id)
        .bind(user_id)
        .bind(auction.id)
        .bind(previous_round.round_num - 1)
        .fetch_all(&mut **tx)
        .await
        .with_context(|| {
            format!("failed to get active spaces for user {}", user_id)
        })?;

        // Calculate total eligibility points from active spaces
        let total_points: f64 = spaces
            .iter()
            .filter(|space| active_spaces.contains(&space.id))
            .map(|space| space.eligibility_points)
            .sum();

        // Calculate new eligibility by dividing by threshold
        let mut new_eligibility =
            total_points / previous_round.eligibility_threshold;

        // If not first round (round_num > 0), get previous eligibility and ensure no increase
        if previous_round.round_num > 0 {
            let prev_eligibility = sqlx::query_scalar::<_, f64>(
                "SELECT eligibility FROM user_eligibilities 
                WHERE round_id = $1 AND user_id = $2",
            )
            .bind(previous_round.id)
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await
            .with_context(|| {
                format!(
                    "failed to get previous eligibility for user {} in round {}",
                    user_id, previous_round.round_num
                )
            })?;

            if let Some(prev) = prev_eligibility {
                new_eligibility = new_eligibility.min(prev);
            }
        }

        // Store the new eligibility for the next round
        sqlx::query(
            "INSERT INTO user_eligibilities (user_id, round_id, eligibility)
            VALUES ($1, $2, $3)",
        )
        .bind(user_id)
        .bind(new_round_id)
        .bind(new_eligibility)
        .execute(&mut **tx)
        .await
        .with_context(|| {
            format!(
                "failed to store eligibility for user {} in round {}",
                user_id,
                previous_round.round_num + 1
            )
        })?;
    }

    Ok(())
}

fn get_eligibility_for_round_num(
    round_num: i32,
    progression: &[(i32, f64)],
) -> f64 {
    if progression.is_empty() {
        return 0.0;
    }
    if progression.len() == 1 && progression[0].0 > round_num {
        return 0.0;
    }
    // binary_search_by returns either the target index if found, or the insert
    // location where the seek value would go, which is one more than the place
    // we want to lookup the points value for the current seek. This is because
    // the eligibility progression defines the point values for the given
    // round number onwards.
    let idx = match progression
        .binary_search_by(|(round, _)| round.cmp(&round_num))
    {
        Ok(idx) => idx,
        Err(idx) => idx - 1,
    };
    progression[idx].1
}

/// Process proxy bidding for all active rounds sequentially.
/// Uses advisory locks to prevent concurrent processing by multiple
/// scheduler instances.
#[tracing::instrument(skip(pool, time_source))]
async fn process_proxy_bidding_for_active_rounds(
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    tracing::debug!("Starting process_proxy_bidding_for_active_rounds");

    loop {
        match process_next_active_round(pool, time_source).await {
            Ok(true) => continue, // Processed one, try for more
            Ok(false) => break,   // No more rounds to process
            Err(e) => {
                // Log error but continue to next round
                tracing::error!(
                    "Failed to process proxy bidding for round: {:#}",
                    e
                );
                continue;
            }
        }
    }

    tracing::debug!("Finished process_proxy_bidding_for_active_rounds");
    Ok(())
}

/// Lock and process the next active round that needs proxy bidding processing.
/// Returns Ok(true) if a round was processed, Ok(false) if no rounds available.
#[tracing::instrument(skip(pool, time_source))]
async fn process_next_active_round(
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<bool> {
    tracing::debug!("Starting process_next_active_round");
    // This transaction is ONLY used to hold the advisory lock for coordination.
    // It prevents re-entry by other scheduler instances.
    // No other database operations should be attached to this transaction.
    let mut coordination_tx = pool.begin().await?;

    // Find a round that needs processing and acquire advisory lock
    let round = match lock_next_active_round_needing_processing(
        &mut coordination_tx,
        time_source,
    )
    .await?
    {
        Some(r) => r,
        None => {
            tracing::debug!("No rounds available for processing");
            return Ok(false);
        }
    };

    tracing::debug!(
        "Found round {:?} for processing and acquired advisory lock: last_processed={:?}, failure_count={}",
        round.id,
        round.proxy_bidding_last_processed_at,
        round.proxy_bidding_failure_count
    );

    let round_id = round.id;

    // Process the round in its own transaction (not the coordination_tx)
    match process_proxy_bidding_for_round(&round, pool, time_source).await {
        Ok(()) => {
            tracing::debug!(
                "Successfully processed proxy bidding for round {:?}",
                round_id
            );
            // Update last processed timestamp and reset failure tracking
            sqlx::query(
                "UPDATE auction_rounds
                SET proxy_bidding_last_processed_at = $1,
                    proxy_bidding_failure_count = 0,
                    proxy_bidding_last_failed_at = NULL
                WHERE id = $2",
            )
            .bind(time_source.now().to_sqlx())
            .bind(round_id)
            .execute(pool)
            .await?;

            tracing::debug!(
                "Updated round {:?} processing timestamp",
                round_id
            );

            // Success - commit the coordination transaction to release the lock
            coordination_tx.commit().await?;
            Ok(true)
        }
        Err(e) => {
            tracing::error!(
                "Error processing proxy bidding for round {:?}: {:#}",
                round_id,
                e
            );

            // Record the failure in a separate transaction before releasing lock
            let _ = handle_proxy_bidding_failure(round_id, pool, time_source)
                .await
                .map_err(|err| {
                    tracing::error!(
                        "Failed to record proxy bidding failure: {:#}",
                        err
                    );
                });

            // Commit the coordination transaction to release the lock
            // (failure has been recorded in separate transaction above)
            let _ = coordination_tx.commit().await;

            Err(e)
        }
    }
}

/// Lock the next active round that needs proxy bidding processing.
/// Uses FOR UPDATE SKIP LOCKED to prevent blocking and enable concurrent
/// scheduler instances. Uses exponential backoff to avoid repeatedly processing
/// failing rounds.
#[tracing::instrument(skip(tx, time_source))]
async fn lock_next_active_round_needing_processing(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    time_source: &TimeSource,
) -> anyhow::Result<Option<store::AuctionRound>> {
    // Select rounds that:
    // 1. Are currently active (now >= start_at AND now < end_at)
    // 2. Have never been processed OR
    // 3. Have user settings that changed since last processing OR
    // 4. Failed but backoff period has expired

    sqlx::query_as::<_, store::AuctionRound>(
        "SELECT ar.* FROM auction_rounds ar
        WHERE $1 >= ar.start_at
            AND $1 < ar.end_at
            AND (
                -- Never processed
                (ar.proxy_bidding_last_processed_at IS NULL
                 AND ar.proxy_bidding_failure_count = 0)
                -- Or proxy bidding settings updated since last processing
                OR EXISTS (
                    SELECT 1 FROM use_proxy_bidding upb
                    WHERE upb.auction_id = ar.auction_id
                    AND ar.proxy_bidding_last_processed_at IS NOT NULL
                    AND upb.updated_at > ar.proxy_bidding_last_processed_at
                )
                -- Or any user values for this auction updated since last processing
                OR EXISTS (
                    SELECT 1 FROM user_values uv
                    JOIN spaces s ON uv.space_id = s.id
                    JOIN sites si ON s.site_id = si.id
                    JOIN auctions a ON si.id = a.site_id
                    WHERE a.id = ar.auction_id
                    AND ar.proxy_bidding_last_processed_at IS NOT NULL
                    AND uv.updated_at > ar.proxy_bidding_last_processed_at
                )
                -- Or failed but backoff period expired
                OR (
                    ar.proxy_bidding_failure_count > 0
                    AND ar.proxy_bidding_last_failed_at IS NOT NULL
                    AND $1 > ar.proxy_bidding_last_failed_at +
                        INTERVAL '5 minutes' * POW(2, LEAST(ar.proxy_bidding_failure_count, 5))
                )
            )
            -- Try to take a transaction-scoped advisory lock for this row
            AND pg_try_advisory_xact_lock(
                hashtextextended('proxy_bidding:' || ar.id::text, 0)
            )
        ORDER BY random()
        LIMIT 1",
    )
    .bind(time_source.now().to_sqlx())
    .fetch_optional(&mut **tx)
    .await
    .map_err(Into::into)
}

/// Record a failure to process proxy bidding for a round.
#[tracing::instrument(skip(pool, time_source))]
async fn handle_proxy_bidding_failure(
    round_id: payloads::AuctionRoundId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE auction_rounds
        SET proxy_bidding_failure_count = proxy_bidding_failure_count + 1,
            proxy_bidding_last_failed_at = $1
        WHERE id = $2",
    )
    .bind(time_source.now().to_sqlx())
    .bind(round_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Process proxy bidding for a single auction round.
/// Protected by an advisory lock to prevent concurrent processing.
#[tracing::instrument(skip(pool, time_source))]
async fn process_proxy_bidding_for_round(
    round: &store::AuctionRound,
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    tracing::debug!(
        "Entered process_proxy_bidding_for_round for round {:?}",
        round.id
    );
    // Get all proxy bidding settings for this auction
    tracing::debug!(
        "Fetching proxy bidding settings for auction {:?}",
        round.auction_id
    );
    let proxy_settings = sqlx::query_as::<_, store::UseProxyBidding>(
        "SELECT * FROM use_proxy_bidding
        WHERE auction_id = $1",
    )
    .bind(round.auction_id)
    .fetch_all(pool)
    .await?;

    tracing::info!("Found {} proxy bidding settings", proxy_settings.len());
    tracing::debug!("Proxy bidding settings: {:?}", proxy_settings);

    // Get all spaces for this auction
    let spaces = sqlx::query_as::<_, store::Space>(
        "SELECT s.* FROM spaces s
        JOIN sites si ON s.site_id = si.id
        JOIN auctions a ON si.id = a.site_id
        WHERE a.id = $1 AND s.is_available = true AND s.deleted_at IS NULL",
    )
    .bind(round.auction_id)
    .fetch_all(pool)
    .await
    .context("failed to get auction spaces")?;

    tracing::info!("Found {} spaces", spaces.len());

    // Get the most recent completed round results to determine current prices
    // We look at the round with the highest round_num that is less than the current round
    let prev_round_space_results =
        sqlx::query_as::<_, store::RoundSpaceResult>(
            "SELECT *
            FROM round_space_results rsr
            JOIN auction_rounds ar ON rsr.round_id = ar.id
            WHERE ar.auction_id = $1
            AND ar.round_num = $2",
        )
        .bind(round.auction_id)
        .bind(round.round_num - 1)
        .fetch_all(pool)
        .await
        .context("failed to get round results")?;

    tracing::info!(
        "Found {} round results from previous rounds",
        prev_round_space_results.len()
    );

    // Get the auction params for the bid increment
    let auction_params = sqlx::query_as::<_, store::AuctionParams>(
        "SELECT * FROM auction_params ap
        JOIN auctions a on ap.id = a.auction_params_id
        WHERE a.id = $1",
    )
    .bind(round.auction_id)
    .fetch_one(pool)
    .await
    .context("failed to get auction params")?;

    let mut user_err = None;

    // Process each user's proxy bidding settings
    for settings in proxy_settings {
        if let Err(e) = process_user_proxy_bidding(
            &settings,
            &spaces,
            &prev_round_space_results,
            &round.id,
            auction_params.bid_increment,
            pool,
            time_source,
        )
        .await
        {
            log_error(e);
            user_err =
                Some(anyhow::anyhow!("partial user proxy bidding failure"));
        };
    }

    tracing::debug!(
        "Finished processing all users' proxy bidding for round {:?}",
        round.id
    );
    // Return an error if we failed to process a user's bids, so we can try
    // again.
    user_err.map_or(Ok(()), Err)
}

#[tracing::instrument(
    skip_all,
    fields(
        user_id = ?settings.user_id,
        max_items = settings.max_items
    )
)]
async fn process_user_proxy_bidding(
    settings: &store::UseProxyBidding,
    // all spaces
    spaces: &[store::Space],
    // prices as of the previous round; does not exist for round 0
    prev_round_space_results: &[store::RoundSpaceResult],
    current_round_id: &payloads::AuctionRoundId,
    bid_increment: rust_decimal::Decimal,
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;

    // Clear any existing bids for this user in this round before reprocessing.
    // This ensures that if proxy bidding settings or user values were updated
    // mid-round, we start fresh with the new settings.
    tracing::debug!("Clearing existing bids for user {:?}", settings.user_id);
    sqlx::query(
        "DELETE FROM bids
        WHERE round_id = $1 AND user_id = $2",
    )
    .bind(current_round_id)
    .bind(settings.user_id)
    .execute(&mut *tx)
    .await
    .with_context(|| {
        format!(
            "failed to clear existing bids for user {:?}",
            settings.user_id
        )
    })?;

    // Get user values for all spaces
    let user_values = sqlx::query_as::<_, store::UserValue>(
        "SELECT * FROM user_values 
            WHERE user_id = $1 AND space_id = ANY($2)",
    )
    .bind(settings.user_id)
    .bind(spaces.iter().map(|s| s.id).collect::<Vec<_>>())
    .fetch_all(&mut *tx)
    .await
    .with_context(|| {
        format!("failed to get user values for {:?}", settings.user_id)
    })?;

    tracing::info!("Found {} space values", user_values.len(),);

    // Count the number of spaces the user is already the high bidder for
    let num_spaces_already_winning = prev_round_space_results
        .iter()
        .filter(|rsr| rsr.winning_user_id == settings.user_id)
        .count();

    // Calculate surpluses for spaces where user has set values
    let mut space_surpluses: Vec<(SpaceId, Decimal)> = Vec::new();
    for user_value_entry in &user_values {
        let space_id = &user_value_entry.space_id;
        // Get current price from the most recent completed round results, add
        // the bid increment, and calculate the potential surplus
        let current_price = prev_round_space_results
            .iter()
            .find(|r| r.space_id == *space_id)
            .map(|r| r.value + bid_increment)
            .unwrap_or_else(|| Decimal::ZERO);

        let surplus = user_value_entry.value - current_price;
        tracing::info!(
            "{:?}: user_value={}, current_price={}, surplus={}",
            user_value_entry.space_id,
            user_value_entry.value,
            current_price,
            surplus
        );

        if surplus >= Decimal::ZERO {
            space_surpluses.push((user_value_entry.space_id, surplus));
        }
    }

    tracing::info!(
        "Found {} spaces with non-negative surplus",
        space_surpluses.len()
    );

    // Sort by surplus (highest to lowest)
    space_surpluses.sort_by(|a, b| b.1.cmp(&a.1));

    // Try bidding on spaces in surplus order until we hit max_items
    let mut successful_bids = 0;
    for (space_id, surplus) in space_surpluses {
        if successful_bids + num_spaces_already_winning
            >= settings.max_items as usize
        {
            break;
        }

        tracing::info!(
            "Attempting to bid on {:?} with surplus {}",
            space_id,
            surplus
        );

        match store::create_bid_tx(
            &space_id,
            current_round_id,
            &settings.user_id,
            &mut tx,
            time_source,
            pool,
        )
        .await
        {
            Ok(_) => {
                successful_bids += 1;
                tracing::info!("Successfully placed bid on {:?}", space_id);
            }
            Err(store::StoreError::ExceedsEligibility { .. })
            | Err(store::StoreError::NoEligibility)
            | Err(store::StoreError::AlreadyWinningSpace) => {
                // Expected errors - try next space
                tracing::info!(
                    "Failed to bid on {:?}: eligibility or already winning",
                    space_id
                );
                continue;
            }
            Err(store::StoreError::InsufficientBalance) => {
                // User has run out of credit - try next space
                tracing::info!(
                    "Failed to bid on {:?}: insufficient credit, trying next space",
                    space_id
                );
                continue;
            }
            Err(e) => {
                // Log unexpected errors but continue processing
                tracing::error!(
                    "Unexpected error bidding on {:?}: {}",
                    space_id,
                    e
                );
                log_error(anyhow::Error::from(e));
            }
        }
    }

    tx.commit().await?;

    tracing::info!("Placed {} successful new bids", successful_bids,);

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_get_eligibility_for_round_num() {
        use super::get_eligibility_for_round_num;
        let progression: &[(i32, f64)] =
            &[(0, 0.5), (10, 0.75), (20, 0.9), (30, 1.0)];
        let f = get_eligibility_for_round_num;
        assert_eq!(f(0, &progression[..1]), 0.5);
        assert_eq!(f(0, progression), 0.5);
        assert_eq!(f(1, progression), 0.5);
        assert_eq!(f(10, progression), 0.75);
        assert_eq!(f(11, progression), 0.75);
        assert_eq!(f(31, progression), 1.0);
        assert_eq!(f(0, &[]), 0.0);
        assert_eq!(f(0, &[(5, 0.5)]), 0.0);
        assert_eq!(f(5, &[(5, 0.5)]), 0.5);
    }
}
