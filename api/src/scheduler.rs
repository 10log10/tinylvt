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
use sqlx::{Acquire, PgPool};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time;

use crate::{pubsub, store, telemetry::log_error, time::TimeSource};

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
            schedule_tick(&self.pool, &self.time_source).await;
        }
    }
}
/// Main scheduler tick function.
/// Runs all periodic tasks and logs any errors without propagating them,
/// ensuring one task failure doesn't prevent other tasks from running.
#[tracing::instrument(skip(pool, time_source))]
pub async fn schedule_tick(pool: &PgPool, time_source: &TimeSource) {
    // Update active states from schedule
    // TODO: revisit this after MVP
    // let _ = store::update_is_active_from_schedule(pool, time_source)
    // .await
    // .map_err(log_error);

    // Process auctions without ongoing rounds
    let _ = process_auctions_without_rounds(pool, time_source)
        .await
        .map_err(log_error);

    // Process due (round, user) proxy bidding work items
    let _ = process_due_proxy_items(pool, time_source)
        .await
        .map_err(log_error);

    // Refresh storage usage for communities with stale caches
    let _ = store::billing::refresh_all_community_storage(pool, time_source)
        .await
        .context("Failed to refresh storage usage")
        .map_err(log_error);
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
                // Break on error - the scheduler's tick interval provides
                // natural backoff. Individual auction failures are recorded
                // in the database with exponential backoff.
                tracing::error!("Failed to process auction: {:#}", e);
                break;
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
    // A single transaction holds the advisory lock (taken in the selection
    // query) and carries the processing work, making results, settlement,
    // and bookkeeping atomic with lock release. The work runs inside a
    // savepoint so a failure can be recorded on this same transaction while
    // the lock is still held: rolling back to a savepoint releases locks
    // acquired after the savepoint, but the advisory lock predates it.
    let mut tx = pool.begin().await?;

    // Lock one auction atomically using advisory lock
    let auction =
        match lock_next_auction_needing_update(&mut tx, time_source).await? {
            Some(a) => a,
            None => return Ok(false), // No auctions available
        };

    let auction_id = auction.id;

    // Re-verify under the lock, since the selection's snapshot predates the
    // lock acquisition; process the fresh row, not the selection's stale one.
    let auction =
        match reverify_auction_under_lock(auction_id, &mut tx, time_source)
            .await?
        {
            Some(a) => a,
            None => {
                // A peer instance finished this auction after our selection
                // snapshot; release the lock and keep draining the queue.
                tx.commit().await?;
                return Ok(true);
            }
        };

    // Run the work inside a savepoint (sqlx nested transaction)
    let work_result = async {
        let mut work_tx = tx.begin().await?;
        match process_locked_auction(&auction, &mut work_tx, time_source).await
        {
            Ok(()) => work_tx.commit().await.map_err(Into::into),
            Err(e) => {
                // Discard the work's data changes; the advisory lock is
                // unaffected
                work_tx.rollback().await?;
                Err(e)
            }
        }
    }
    .await;

    match work_result {
        Ok(()) => {
            tx.commit().await?;
            Ok(true)
        }
        Err(e) => {
            // Record the failure while still holding the lock, so no other
            // scheduler instance can re-grab the auction before its backoff
            // is written
            let _ = handle_auction_processing_failure(
                auction_id,
                &mut tx,
                time_source,
            )
            .await
            .context("Failed to record auction failure")
            .map_err(log_error);

            // Commit to persist the failure record and release the lock
            let _ = tx.commit().await;

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
///
/// The try-lock is evaluated outside a MATERIALIZED CTE, which is documented
/// to force separate calculation (no folding into the parent, so the lock
/// call can't be cost-reordered in among the data predicates) while still
/// evaluating only as many rows as the parent fetches. The outer `LIMIT 1`
/// therefore pulls candidates lazily and acquires at most one lock: a
/// contended candidate is skipped for free and the walk stops at the first
/// win. Written flat, the planner cost-reorders the cheap lock call ahead of
/// the data predicates and try-locks every scanned row. `None` therefore
/// means no *unclaimed* work exists — every qualifying auction is either
/// absent or currently claimed by a peer instance.
#[tracing::instrument(skip(tx, time_source))]
async fn lock_next_auction_needing_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    time_source: &TimeSource,
) -> anyhow::Result<Option<store::Auction>> {
    sqlx::query_as::<_, store::Auction>(&format!(
        "WITH candidates AS MATERIALIZED (
            SELECT auctions.* FROM auctions
            JOIN sites ON auctions.site_id = sites.id
            WHERE sites.deleted_at IS NULL
                AND start_at IS NOT NULL
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
                    OR $1 > scheduler_last_failed_at + {backoff}
                )
        )
        SELECT * FROM candidates
        WHERE pg_try_advisory_xact_lock({lock_key})
        LIMIT 1",
        backoff = backoff_interval_sql("scheduler_failure_count"),
        lock_key = store::auction::auction_processing_lock_key("candidates.id")
    ))
    .bind(time_source.now().to_sqlx())
    .fetch_optional(&mut **tx)
    .await
    .map_err(Into::into)
}

/// SQL expression for the retry backoff after `count_col` failures: 1
/// second after the first failure, doubling to a ~2.3-hour cap. The base
/// must stay well under the minimum round duration (5 seconds) — a failed
/// item backing off past the round would sit out the retry that could
/// still matter, and both auction processing and proxy items are pure-DB
/// work where transient failures resolve quickly.
fn backoff_interval_sql(count_col: &str) -> String {
    format!("INTERVAL '1 second' * POW(2, LEAST({count_col}, 14) - 1)")
}

/// Re-read the auction under the advisory lock, confirming it still needs
/// processing. The selection query evaluates its predicates against a
/// snapshot taken at statement start, so a peer instance can finish this
/// auction (and release its lock) between that snapshot and our lock
/// acquisition. A fresh statement under the lock is guaranteed to see
/// whatever prior lock holders committed. Returns None if the auction no
/// longer needs processing. (Backoff fields are deliberately not re-checked:
/// staleness there costs one immediate retry of a just-failed auction, which
/// re-records its backoff.)
async fn reverify_auction_under_lock(
    auction_id: payloads::AuctionId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    time_source: &TimeSource,
) -> anyhow::Result<Option<store::Auction>> {
    sqlx::query_as::<_, store::Auction>(
        "SELECT auctions.* FROM auctions
        WHERE id = $2
            AND end_at IS NULL
            AND NOT EXISTS (
                SELECT 1 FROM auction_rounds
                WHERE auction_id = auctions.id
                AND $1 < end_at
            )",
    )
    .bind(time_source.now().to_sqlx())
    .bind(auction_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(Into::into)
}

/// Record a failure to process an auction.
#[tracing::instrument(skip(tx, time_source))]
async fn handle_auction_processing_failure(
    auction_id: payloads::AuctionId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
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
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Process a locked auction on the caller's transaction (a savepoint under
/// the lock-holding transaction). This function handles updating results for
/// previous rounds and creating new rounds.
#[tracing::instrument(skip(tx, auction, time_source))]
async fn process_locked_auction(
    auction: &store::Auction,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    let previous_round = sqlx::query_as::<_, store::AuctionRound>(
        "SELECT * FROM auction_rounds
        WHERE auction_id = $1
        ORDER BY round_num DESC
        LIMIT 1",
    )
    .bind(auction.id)
    .fetch_optional(&mut **tx)
    .await
    .context("failed to query for concluded round")?;

    // If there's a previous round, update its results and check if auction
    // concluded
    if let Some(ref previous_round) = previous_round {
        let auction_continues = update_round_space_results_within_tx(
            auction,
            previous_round,
            tx,
            time_source,
        )
        .await?;

        if !auction_continues {
            // Auction has concluded, no more rounds to create
            return Ok(());
        }

        // The next round would be `round_num + 1`. If creating it would reach
        // the hard ceiling, the auction can't terminate in a reasonable time
        // (e.g. a bid increment too small relative to bidders' values). Cancel
        // rather than settle: the allocation isn't valid since bidding never
        // naturally ended, and users should retry with a larger increment.
        if previous_round.round_num + 1 >= payloads::MAX_AUCTION_ROUNDS {
            cancel_runaway_auction(auction, tx, time_source).await?;
            return Ok(());
        }
    }

    // Create next round
    let new_round_id = add_subsequent_rounds_for_auction(
        auction,
        &previous_round,
        tx,
        time_source,
    )
    .await?;

    // Update eligibilities only if there was a previous round
    if let Some(ref previous_round) = previous_round {
        update_user_eligibilities(auction, previous_round, &new_round_id, tx)
            .await?;
    }

    Ok(())
}

/// Cancel an auction that has hit [`payloads::MAX_AUCTION_ROUNDS`]. Mirrors
/// `store::auction::cancel_auction`'s terminal state (`end_at` set,
/// `was_canceled = TRUE`, `AuctionEnded` emitted) but runs inside the
/// scheduler's existing transaction and creates no settlement entry, since a
/// canceled auction has no valid allocation.
async fn cancel_runaway_auction(
    auction: &store::Auction,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    let now = time_source.now();
    sqlx::query(
        "UPDATE auctions
        SET end_at = $1, was_canceled = TRUE, updated_at = $1
        WHERE id = $2",
    )
    .bind(now.to_sqlx())
    .bind(auction.id)
    .execute(&mut **tx)
    .await
    .context("failed to cancel runaway auction")?;

    pubsub::emit(
        tx,
        &payloads::AuctionEvent::AuctionEnded {
            auction_id: auction.id,
        },
    )
    .await?;

    tracing::warn!(
        auction_id = ?auction.id,
        max_rounds = payloads::MAX_AUCTION_ROUNDS,
        "auction reached the round cap and was canceled; bid increment is \
         likely too small relative to bidders' values",
    );

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
            // In mock-time mode, use deterministic ordering for reproducible
            // tests. Need to use username since ids are nondeterministic.
            #[cfg(feature = "mock-time")]
            let query = "SELECT b.user_id FROM bids b
                JOIN users u ON b.user_id = u.id
                WHERE b.space_id = $1 AND b.round_id = $2
                ORDER BY u.username
                LIMIT 1";
            #[cfg(not(feature = "mock-time"))]
            let query = "SELECT user_id FROM bids
                WHERE space_id = $1 AND round_id = $2
                ORDER BY random()
                LIMIT 1";

            let winner = sqlx::query_scalar::<_, payloads::UserId>(query)
                .bind(space.id)
                .bind(previous_round.id)
                .fetch_one(&mut **tx)
                .await
                .with_context(|| {
                    format!(
                        "failed to select winning bid for space {}",
                        space.id
                    )
                })?;

            let new_value = payloads::next_bid_amount(
                prev_result.as_ref().map(|p| p.value),
                auction_params.bid_increment,
                space.reserve_price,
            );

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

    // The previous round is now fully concluded — its round_space_results have
    // been written. Subscribers that care about results subscribe to
    // RoundEnded.
    pubsub::emit(
        tx,
        &payloads::AuctionEvent::RoundEnded {
            auction_id: auction.id,
            round_id: previous_round.id,
        },
    )
    .await?;

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

        pubsub::emit(
            tx,
            &payloads::AuctionEvent::AuctionEnded {
                auction_id: auction.id,
            },
        )
        .await?;

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

    // Only started auctions are processed by the scheduler, so a missing
    // start time here is a bug, not an expected state.
    let auction_start = auction
        .start_at
        .context("auction has no start time; cannot create rounds")?;

    let start_time_ts = previous_round
        .as_ref()
        .map(|r| r.end_at)
        .unwrap_or(auction_start);

    // use DST-aware datetime math in case the round duration is days or
    // larger
    let zoned_start_time = match start_time_ts
        .in_tz(&timezone.unwrap_or("UTC".into()))
        .context("converting to timezone; falling back to DST-naive")
    {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("{e:#}");
            auction_start.to_zoned(TimeZone::UTC)
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

    pubsub::emit(
        tx,
        &payloads::AuctionEvent::RoundCreated {
            auction_id: auction.id,
            round_id: new_round.id,
        },
    )
    .await?;

    Ok(new_round.id)
}

/// Update user eligibilities after an auction round concludes.
///
/// In each round, eligibility is based on two factors:
/// 1. New bids placed in the just-concluded round
/// 2. Standing high bids from the round before that
///
/// This accounts for the natural alternating pattern of bidding where bidders
/// don't need to rebid on spaces they're already winning. For example:
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
/// This ensures bidders maintain eligibility even in rounds where they don't
/// need to place new bids because they're already winning from the previous
/// round.
///
/// The eligibility calculation takes the total eligibility points from these
/// spaces and divides by the eligibility threshold. For example, if the
/// threshold is 0.5 (50%), and a user has activity on spaces worth 10 points,
/// their eligibility is set to 20 points (10 / 0.5).
///
/// After the first round, eligibility cannot increase. For example, if a user
/// has 20 points of eligibility after round 1:
/// - If they bid on 15 points of spaces in round 2, eligibility stays at 20
/// - If they bid on 5 points of spaces in round 2, eligibility drops to 10 (5 /
///   0.5)
#[tracing::instrument(skip(tx))]
pub async fn update_user_eligibilities(
    auction: &store::Auction,
    previous_round: &store::AuctionRound,
    new_round_id: &payloads::AuctionRoundId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> anyhow::Result<()> {
    // A 0.0 threshold means the next round is unconstrained: bids in it are
    // checked against the prior round's threshold (see create_bid), which finds
    // 0.0 and skips the eligibility row entirely. So there's nothing to derive
    // or store here, and dividing by the threshold would produce a non-finite
    // value (+inf, or NaN with no activity).
    if previous_round.eligibility_threshold == 0.0 {
        return Ok(());
    }

    // Get all spaces for this auction's site to calculate eligibility points
    let spaces = sqlx::query_as::<_, store::Space>(
        "SELECT * FROM spaces WHERE site_id = $1 AND is_available = true AND deleted_at IS NULL",
    )
    .bind(auction.site_id)
    .fetch_all(&mut **tx)
    .await
    .context("failed to get available spaces for site")?;

    // Get all users who either bid in the previous round or had a winning bid
    // in the round before that
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
        // Get all spaces this user bid on in the previous round OR was winning
        // from two rounds ago
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

        // If not first round (round_num > 0), get previous eligibility and
        // ensure no increase
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
    // binary_search_by returns either the index of an exact match, or the
    // insert location where round_num would go. The eligibility progression
    // defines the threshold for a breakpoint's round onwards, so on a miss we
    // want the breakpoint just before the insert location (idx - 1).
    match progression.binary_search_by(|(round, _)| round.cmp(&round_num)) {
        Ok(idx) => progression[idx].1,
        // Before the first breakpoint (insert location 0): no breakpoint
        // applies yet, so eligibility is unconstrained (0.0). This also
        // covers an empty progression, whose only insert location is 0.
        Err(0) => 0.0,
        Err(idx) => progression[idx - 1].1,
    }
}

/// A due (round, user) proxy work item, as listed by the lock-free selector.
#[derive(Debug, sqlx::FromRow)]
struct ProxyWorkItem {
    round_id: payloads::AuctionRoundId,
    auction_id: payloads::AuctionId,
    user_id: payloads::UserId,
}

/// Process all due (round, user) proxy work items. The selector lists
/// candidates lock-free; each item is then claimed individually via a
/// try-lock on its `auction_user` pair key, so a stale or duplicate
/// candidate list is harmless — losers skip. One user's failure is
/// recorded on that user's marker alone and never affects other items.
#[tracing::instrument(skip(pool, time_source))]
async fn process_due_proxy_items(
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    let items = list_due_proxy_items(pool, time_source).await?;
    if items.is_empty() {
        return Ok(());
    }
    tracing::debug!("Found {} due proxy work items", items.len());

    for item in &items {
        // Per-item failures are recorded on the item's marker (backoff) and
        // must not stop the pass — that isolation is the point.
        if let Err(e) = process_proxy_item(item, pool, time_source).await {
            tracing::error!(
                "Failed to process proxy item (round {:?}, user {:?}): {:#}",
                item.round_id,
                item.user_id,
                e
            );
        }
    }

    Ok(())
}

/// List due (round, user) proxy work items, without claiming them. An item
/// is due when its active round has no marker row (per-round baseline), its
/// settings row is flagged dirty (mid-round change — this arm ignores
/// backoff, making a member change during backoff a fresh-input retry), or
/// its marker records failures and the backoff has expired.
async fn list_due_proxy_items(
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<Vec<ProxyWorkItem>> {
    // In mock-time mode, order deterministically for reproducible tests
    // (usernames, since ids are nondeterministic).
    #[cfg(feature = "mock-time")]
    let order = "ORDER BY ar.start_at, u.username";
    #[cfg(not(feature = "mock-time"))]
    let order = "";

    sqlx::query_as::<_, ProxyWorkItem>(&format!(
        "SELECT ar.id AS round_id, ar.auction_id, upb.user_id
        FROM auction_rounds ar
        -- a.end_at excludes auctions canceled mid-round (the round row
        -- still spans now, but bidding into it would be pointless)
        JOIN auctions a ON ar.auction_id = a.id AND a.end_at IS NULL
        JOIN use_proxy_bidding upb ON upb.auction_id = ar.auction_id
        JOIN users u ON upb.user_id = u.id
        LEFT JOIN proxy_round_processing prp
            ON prp.round_id = ar.id AND prp.user_id = upb.user_id
        WHERE $1 >= ar.start_at
            AND $1 < ar.end_at
            AND (
                prp.round_id IS NULL
                OR upb.needs_processing
                OR (
                    prp.failure_count > 0
                    AND prp.last_failed_at IS NOT NULL
                    AND $1 > prp.last_failed_at + {backoff}
                )
            )
        {order}",
        backoff = backoff_interval_sql("prp.failure_count"),
    ))
    .bind(time_source.now().to_sqlx())
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

/// Claim and process one (round, user) proxy work item. The claim tx
/// carries the whole operation: pair advisory try-lock first (losing
/// contenders bounce off the probe and never queue behind the row lock),
/// then the flag-clearing UPDATE under the settings row lock, dueness
/// re-verified under the claim, the bidding work inside a savepoint, and
/// the marker write — one commit makes flag-clear + bids + marker atomic.
/// A crash discards everything including the flag clear, so the item is
/// simply re-selected. Concurrent settings writers block on the row lock,
/// land strictly after the commit, and re-set the flag.
async fn process_proxy_item(
    item: &ProxyWorkItem,
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;

    let claimed: bool = sqlx::query_scalar(&format!(
        "SELECT pg_try_advisory_xact_lock({})",
        store::auction::auction_user_lock_key("$1", "$2")
    ))
    .bind(item.auction_id)
    .bind(item.user_id)
    .fetch_one(&mut *tx)
    .await?;
    if !claimed {
        // Another claimant owns this (auction, user); it will process or
        // the item stays due and is re-selected next tick.
        return Ok(());
    }

    // Lock and read the settings row (capturing the flag's pre-clear
    // value), then clear the flag. The clear stays uncommitted until the
    // final commit; other selectors are suppressed by the advisory lock,
    // not the flag's visible state.
    let settings = sqlx::query_as::<_, store::UseProxyBidding>(
        "SELECT * FROM use_proxy_bidding
        WHERE user_id = $1 AND auction_id = $2
        FOR UPDATE",
    )
    .bind(item.user_id)
    .bind(item.auction_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(settings) = settings else {
        // Proxy bidding was disabled after the candidate was listed
        return Ok(());
    };
    if settings.needs_processing {
        sqlx::query(
            "UPDATE use_proxy_bidding SET needs_processing = FALSE
            WHERE user_id = $1 AND auction_id = $2",
        )
        .bind(item.user_id)
        .bind(item.auction_id)
        .execute(&mut *tx)
        .await?;
    }

    // Re-verify under the claim: the candidate list's snapshot predates the
    // lock, and a prior claimant may have just processed this item. Fresh
    // statements here see everything prior lock holders committed.
    let now = time_source.now();
    let round = sqlx::query_as::<_, store::AuctionRound>(
        "SELECT ar.* FROM auction_rounds ar
        JOIN auctions a ON ar.auction_id = a.id AND a.end_at IS NULL
        WHERE ar.id = $1 AND $2 >= ar.start_at AND $2 < ar.end_at",
    )
    .bind(item.round_id)
    .bind(now.to_sqlx())
    .fetch_optional(&mut *tx)
    .await?;
    let Some(round) = round else {
        // Round ended, or the auction was canceled mid-round; a next
        // round's baseline arm covers any reprocessing
        tx.rollback().await?;
        return Ok(());
    };
    let due: bool = sqlx::query_scalar(&format!(
        "SELECT $3
            OR NOT EXISTS (
                SELECT 1 FROM proxy_round_processing
                WHERE round_id = $1 AND user_id = $2
            )
            OR EXISTS (
                SELECT 1 FROM proxy_round_processing
                WHERE round_id = $1 AND user_id = $2
                    AND failure_count > 0
                    AND last_failed_at IS NOT NULL
                    AND $4 > last_failed_at + {backoff}
            )",
        backoff = backoff_interval_sql("failure_count"),
    ))
    .bind(item.round_id)
    .bind(item.user_id)
    .bind(settings.needs_processing)
    .bind(now.to_sqlx())
    .fetch_one(&mut *tx)
    .await?;
    if !due {
        tx.rollback().await?;
        return Ok(());
    }

    // Run the bidding work inside a savepoint so a failure can be recorded
    // on the marker while the flag stays cleared (a writer-side signal
    // only) and the claim commits — re-selection then goes through backoff,
    // or immediately via the flag if the member changes inputs.
    let work_result = async {
        let mut work_tx = tx.begin().await?;
        match run_proxy_item_work(
            &settings,
            &round,
            &mut work_tx,
            time_source,
            pool,
        )
        .await
        {
            Ok(()) => work_tx.commit().await.map_err(Into::into),
            Err(e) => {
                work_tx.rollback().await?;
                Err(e)
            }
        }
    }
    .await;

    match work_result {
        Ok(()) => {
            sqlx::query(
                "INSERT INTO proxy_round_processing
                    (round_id, user_id, processed_at, failure_count,
                     last_failed_at)
                VALUES ($1, $2, $3, 0, NULL)
                ON CONFLICT (round_id, user_id) DO UPDATE
                SET processed_at = EXCLUDED.processed_at,
                    failure_count = 0,
                    last_failed_at = NULL",
            )
            .bind(item.round_id)
            .bind(item.user_id)
            .bind(now.to_sqlx())
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            Ok(())
        }
        Err(e) => {
            sqlx::query(
                "INSERT INTO proxy_round_processing
                    (round_id, user_id, failure_count, last_failed_at)
                VALUES ($1, $2, 1, $3)
                ON CONFLICT (round_id, user_id) DO UPDATE
                SET failure_count = proxy_round_processing.failure_count + 1,
                    last_failed_at = EXCLUDED.last_failed_at",
            )
            .bind(item.round_id)
            .bind(item.user_id)
            .bind(now.to_sqlx())
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            Err(e)
        }
    }
}

#[tracing::instrument(
    skip_all,
    fields(
        user_id = ?settings.user_id,
        max_items = settings.max_items
    )
)]
async fn run_proxy_item_work(
    settings: &store::UseProxyBidding,
    round: &store::AuctionRound,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    time_source: &TimeSource,
    pool: &PgPool, // for create_bid_tx's validation reads
) -> anyhow::Result<()> {
    // Plan reads: the auction-level inputs for this item. (The settings row
    // always denotes a current member: proxy bidding rows are deleted when
    // a member leaves a community.)
    let spaces = sqlx::query_as::<_, store::Space>(
        "SELECT s.* FROM spaces s
        JOIN sites si ON s.site_id = si.id
        JOIN auctions a ON si.id = a.site_id
        WHERE a.id = $1 AND s.is_available = true AND s.deleted_at IS NULL",
    )
    .bind(round.auction_id)
    .fetch_all(&mut **tx)
    .await
    .context("failed to get auction spaces")?;

    // Index spaces by id for cheap reserve price lookups when computing
    // surpluses.
    let spaces: HashMap<SpaceId, store::Space> =
        spaces.into_iter().map(|s| (s.id, s)).collect();

    // Prices as of the previous round; does not exist for round 0.
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
        .fetch_all(&mut **tx)
        .await
        .context("failed to get round results")?;

    // Get the auction params for the bid increment
    let auction_params = sqlx::query_as::<_, store::AuctionParams>(
        "SELECT * FROM auction_params ap
        JOIN auctions a on ap.id = a.auction_params_id
        WHERE a.id = $1",
    )
    .bind(round.auction_id)
    .fetch_one(&mut **tx)
    .await
    .context("failed to get auction params")?;
    let bid_increment = auction_params.bid_increment;

    // Clear any existing bids for this user in this round before reprocessing.
    // This ensures that if proxy bidding settings or user values were updated
    // mid-round, we start fresh with the new settings.
    tracing::debug!("Clearing existing bids for user {:?}", settings.user_id);
    sqlx::query(
        "DELETE FROM bids
        WHERE round_id = $1 AND user_id = $2",
    )
    .bind(round.id)
    .bind(settings.user_id)
    .execute(&mut **tx)
    .await
    .with_context(|| {
        format!(
            "failed to clear existing bids for user {:?}",
            settings.user_id
        )
    })?;

    // Get user values for all spaces
    // In mock-time mode, order by space name for deterministic proxy bidding
    // Need to use space name since ids are nondeterministic
    #[cfg(feature = "mock-time")]
    let user_values_query = "SELECT uv.* FROM user_values uv
        JOIN spaces s ON uv.space_id = s.id
        WHERE uv.user_id = $1 AND uv.space_id = ANY($2)
        ORDER BY s.name";
    #[cfg(not(feature = "mock-time"))]
    let user_values_query = "SELECT * FROM user_values
        WHERE user_id = $1 AND space_id = ANY($2)";

    let user_values = sqlx::query_as::<_, store::UserValue>(user_values_query)
        .bind(settings.user_id)
        .bind(spaces.keys().copied().collect::<Vec<_>>())
        .fetch_all(&mut **tx)
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
    // user_values is already ordered by space name in mock-time mode
    // Tuples: (space_id, surplus, value)
    let mut space_surpluses: Vec<(SpaceId, Decimal, Decimal)> = Vec::new();
    for user_value_entry in &user_values {
        let space_id = &user_value_entry.space_id;
        // The user_values query filters by `space_id = ANY(spaces.keys())`,
        // so every entry's space should be in the spaces map. Skip with a
        // warning if not, rather than silently falling back.
        let Some(space) = spaces.get(space_id) else {
            tracing::warn!(
                space_id = ?space_id,
                user_id = ?settings.user_id,
                "proxy bidding: user_value references a space not in the \
                 spaces map; skipping",
            );
            continue;
        };
        // Compute what the next bid on this space would cost, then
        // compare to the user's stated value. Surplus < 0 means the user
        // wouldn't bid here.
        let prev_value = prev_round_space_results
            .iter()
            .find(|r| r.space_id == *space_id)
            .map(|r| r.value);
        let next_bid = payloads::next_bid_amount(
            prev_value,
            bid_increment,
            space.reserve_price,
        );

        let surplus = user_value_entry.value - next_bid;
        tracing::info!(
            "{:?}: user_value={}, next_bid={}, surplus={}",
            user_value_entry.space_id,
            user_value_entry.value,
            next_bid,
            surplus
        );

        if surplus >= Decimal::ZERO {
            space_surpluses.push((
                user_value_entry.space_id,
                surplus,
                user_value_entry.value,
            ));
        }
    }

    tracing::info!(
        "Found {} spaces with non-negative surplus",
        space_surpluses.len()
    );

    // Sort by surplus descending, then value descending to break ties
    space_surpluses.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.cmp(&a.2)));

    // Try bidding on spaces in surplus order until we hit max_items
    let mut successful_bids = 0;
    for (space_id, surplus, _value) in space_surpluses {
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
            &round.id,
            &settings.user_id,
            tx,
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

    pubsub::emit(
        tx,
        &payloads::AuctionEvent::BidsChanged {
            auction_id: round.auction_id,
            round_id: round.id,
            user_id: settings.user_id,
        },
    )
    .await?;

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
        // Multiple breakpoints all in the future: rounds before the first
        // breakpoint are unconstrained, not a panic from index underflow.
        assert_eq!(f(2, &[(5, 0.5), (10, 0.75)]), 0.0);
        assert_eq!(f(0, &[(5, 0.5), (10, 0.75)]), 0.0);
        assert_eq!(f(5, &[(5, 0.5), (10, 0.75)]), 0.5);
        assert_eq!(f(7, &[(5, 0.5), (10, 0.75)]), 0.5);
        assert_eq!(f(10, &[(5, 0.5), (10, 0.75)]), 0.75);
    }
}
