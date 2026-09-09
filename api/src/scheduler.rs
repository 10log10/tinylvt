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
//!
//! # Proxy bidding processing
//!
//! Proxy bidding is processed as per-(round, user) work items, each
//! flowing through these functions in order:
//!
//! 1. [`process_due_proxy_items`] — the per-tick selector pass: lists due items
//!    lock-free (`list_due_proxy_items`), processes informal-mode items inline,
//!    and spawns backed-mode items as concurrent tasks (`ProxyTasks`), since
//!    those may carry a Stripe call.
//! 2. [`process_proxy_item`] — funding cycles: run the bidding claim; when it
//!    commits an authorization order, execute the order and re-enter the claim
//!    against the enlarged backing, until a claim completes without ordering.
//! 3. `run_proxy_bid_claim` — the bidding claim, always on the shared pool (it
//!    never carries a Stripe call): take the `auction_user` pair lock, clear
//!    the `use_proxy_bidding.needs_processing` dirty flag under that row's
//!    lock, re-verify the item is still due, run `run_proxy_item_work` in a
//!    savepoint (delete the member's bids for the round and re-place them,
//!    surplus-ordered `create_bid_tx` calls; a card-backed bid failing on funds
//!    sizes an authorization for that exact bid and commits the
//!    `funding_intents` pending row — the authorization order, via
//!    `funding_flow::order_proxy_bid_auth_tx` — and stops the walk), and write
//!    the outcome to the `proxy_round_processing` marker (success, or
//!    failure_count for backoff re-selection). Flag-clear, bids, order, and
//!    marker land atomically in its single commit.
//! 4. `funding_flow::execute_auth_order` (when the claim ordered) — a
//!    pair-locked claim on the worker pool, carrying only the Stripe work:
//!    replay the order's committed parameters (create+confirm+activate via
//!    `authorize_and_activate`; see `funding_flow`'s module docs). Commits
//!    immediately — the authorization is durable before any re-bidding, and
//!    needs no atomicity with it.
//!
//! A related loop, [`process_intent_work`], works off intent rows owing
//! a Stripe call: canceling the predecessor authorizations swap-reauth
//! raises leave behind (`superseded`), releasing holds settlement or
//! cancellation marked `release_pending`, capturing winners'
//! `capture_pending` authorizations, and canceling aged `pending`
//! orders whose execute never ran — the per-row claims live in
//! `funding_flow` (`cancel_worker_intent`, `capture_intent`,
//! `cancel_aged_pending_order`); this loop only selects and dispatches.

use anyhow::Context;
use jiff::tz::TimeZone;
use jiff_sqlx::ToSqlx;
use payloads::{ApiError, SpaceId};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

use crate::{
    WorkerPool,
    email::EmailService,
    pubsub, store,
    store::funding_flow::{
        AuthOrderOutcome, FlowDeps, Presence, WorkerIntent,
        cancel_aged_pending_order, cancel_worker_intent, capture_intent,
    },
    stripe_service::StripeService,
    telemetry::log_error,
    time::TimeSource,
};
use store::locks::{LockWait, TrackedTx};

pub struct Scheduler {
    pool: PgPool,
    worker_pool: WorkerPool,
    time_source: TimeSource,
    stripe_service: Arc<StripeService>,
    email_service: Arc<EmailService>,
    tick_interval: Duration,
}

impl Scheduler {
    pub fn new(
        pool: PgPool,
        worker_pool: WorkerPool,
        time_source: TimeSource,
        stripe_service: Arc<StripeService>,
        email_service: Arc<EmailService>,
        tick_interval: Duration,
    ) -> Self {
        Self {
            pool,
            worker_pool,
            time_source,
            stripe_service,
            email_service,
            tick_interval,
        }
    }

    /// Run the scheduler's loops forever (production entry point;
    /// spawned once from main). Tests drive everything synchronously
    /// through `schedule_tick` instead.
    ///
    /// Round processing is pure-DB and must tick reliably; the
    /// proxy-funding and intent workers carry Stripe latency, so they
    /// run as separate loops that can never delay it. The proxy loop
    /// keeps one `ProxyTasks` across iterations and never awaits its
    /// spawned items, so a slow card task delays nothing but itself.
    /// Reconciliation runs in its own loop for the same reason one
    /// level up: its hourly pass makes unbounded serial Stripe
    /// retrieves, and captures, cancels, and scheduled pre-auths must
    /// not queue behind it.
    ///
    /// A panic in any loop unwinds through the `join!` and out of this
    /// future; main observes the task's exit and aborts the process, so
    /// a bug can't leave a live API with scheduling silently dead.
    pub async fn run(&self) {
        let rounds = async {
            let mut interval = time::interval(self.tick_interval);
            loop {
                interval.tick().await;
                round_tick(&self.pool, &self.time_source).await;
            }
        };
        let proxy = async {
            let mut tasks = ProxyTasks::new();
            let mut interval = time::interval(self.tick_interval);
            loop {
                interval.tick().await;
                let _ = process_due_proxy_items(
                    &self.pool,
                    &self.worker_pool,
                    &self.time_source,
                    &self.stripe_service,
                    &mut tasks,
                )
                .await
                .map_err(log_error);
            }
        };
        let intents = async {
            let mut interval = time::interval(self.tick_interval);
            loop {
                interval.tick().await;
                let _ = process_scheduled_preauths(
                    &self.pool,
                    &self.worker_pool,
                    &self.time_source,
                    &self.stripe_service,
                )
                .await
                .map_err(log_error);
                let _ = process_intent_work(
                    &self.pool,
                    &self.worker_pool,
                    &self.time_source,
                    &self.stripe_service,
                )
                .await
                .map_err(log_error);
                let _ = process_notification_outbox(
                    &self.pool,
                    &self.worker_pool,
                    &self.time_source,
                    &self.email_service,
                )
                .await
                .map_err(log_error);
            }
        };
        let reconciliation = async {
            let mut interval = time::interval(self.tick_interval);
            loop {
                interval.tick().await;
                let _ = run_reconciliation_if_due(
                    &self.pool,
                    &self.worker_pool,
                    &self.time_source,
                    &self.stripe_service,
                )
                .await
                .map_err(log_error);
            }
        };
        tokio::join!(rounds, proxy, intents, reconciliation);
    }
}

/// Run one synchronous pass of every scheduler task, returning once all
/// work — including spawned proxy card tasks — has completed: the
/// deterministic entry point tests drive (via `TestApp::tick`). Errors
/// are logged without propagating so one task's failure doesn't prevent
/// the others from running.
#[tracing::instrument(skip_all)]
pub async fn schedule_tick(
    pool: &PgPool,
    worker_pool: &WorkerPool,
    time_source: &TimeSource,
    stripe_service: &Arc<StripeService>,
    email_service: &Arc<EmailService>,
) {
    round_tick(pool, time_source).await;

    // Mint scheduled pre-authorizations for proxy participants of
    // auctions starting within the advance window.
    let _ = process_scheduled_preauths(
        pool,
        worker_pool,
        time_source,
        stripe_service,
    )
    .await
    .map_err(log_error);

    // Process due (round, user) proxy bidding work items, awaiting the
    // spawned card tasks so callers observe completed work.
    let mut tasks = ProxyTasks::new();
    let _ = process_due_proxy_items(
        pool,
        worker_pool,
        time_source,
        stripe_service,
        &mut tasks,
    )
    .await
    .map_err(log_error);
    tasks.drain().await;

    // Work off intent rows owing a Stripe call: cancels (superseded,
    // released, and age-scanned holds) and settlement captures.
    let _ = process_intent_work(pool, worker_pool, time_source, stripe_service)
        .await
        .map_err(log_error);

    // Deliver enqueued notification emails.
    let _ = process_notification_outbox(
        pool,
        worker_pool,
        time_source,
        email_service,
    )
    .await
    .map_err(log_error);

    // Hourly reconciliation: the gate skips unless a community's
    // watermark is stale, so calling every tick is cheap.
    let _ = run_reconciliation_if_due(
        pool,
        worker_pool,
        time_source,
        stripe_service,
    )
    .await
    .map_err(log_error);
}

/// The pure-DB tick: auction round processing and storage refresh.
#[tracing::instrument(skip(pool, time_source))]
async fn round_tick(pool: &PgPool, time_source: &TimeSource) {
    // Update active states from schedule
    // TODO: revisit this after MVP
    // let _ = store::update_is_active_from_schedule(pool, time_source)
    // .await
    // .map_err(log_error);

    // Process auctions without ongoing rounds
    let _ = process_auctions_without_rounds(pool, time_source)
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
    let mut locks = TrackedTx::begin(pool).await?;

    // Lock one auction atomically using advisory lock
    let auction = match lock_next_auction_needing_update(
        locks.tx(),
        time_source,
    )
    .await?
    {
        Some(a) => a,
        None => return Ok(false), // No auctions available
    };

    let auction_id = auction.id;
    // The selection SQL's embedded try-lock succeeded for this auction;
    // record the claim in the lock record.
    locks.assume_processing_locked(&auction_id);

    // Re-verify under the lock, since the selection's snapshot predates the
    // lock acquisition; process the fresh row, not the selection's stale one.
    let auction =
        match reverify_auction_under_lock(auction_id, locks.tx(), time_source)
            .await?
        {
            Some(a) => a,
            None => {
                // A peer instance finished this auction after our selection
                // snapshot; release the lock and keep draining the queue.
                locks.commit().await?;
                return Ok(true);
            }
        };

    // Run the work inside a savepoint (sqlx nested transaction)
    let work_result = async {
        let mut work = locks.savepoint().await?;
        match process_locked_auction(&auction, &mut work, time_source).await {
            Ok(()) => work.commit().await.map_err(Into::into),
            Err(e) => {
                // Discard the work's data changes (and its recorded row
                // locks); the advisory lock predates the savepoint and is
                // unaffected
                work.rollback().await?;
                Err(e)
            }
        }
    }
    .await;

    match work_result {
        Ok(()) => {
            locks.commit().await?;
            Ok(true)
        }
        Err(e) => {
            // Record the failure while still holding the lock, so no other
            // scheduler instance can re-grab the auction before its backoff
            // is written
            let _ = handle_auction_processing_failure(
                auction_id,
                locks.tx(),
                time_source,
            )
            .await
            .context("Failed to record auction failure")
            .map_err(log_error);

            // Commit to persist the failure record and release the lock
            let _ = locks.commit().await;

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
        backoff = store::backoff_interval_sql("scheduler_failure_count"),
        lock_key = store::locks::auction_processing_lock_key("candidates.id")
    ))
    .bind(time_source.now().to_sqlx())
    .fetch_optional(&mut **tx)
    .await
    .map_err(Into::into)
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
#[tracing::instrument(skip(locks, auction, time_source))]
async fn process_locked_auction(
    auction: &store::Auction,
    locks: &mut TrackedTx<'_, '_>,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    let previous_round = sqlx::query_as::<_, store::AuctionRound>(
        "SELECT * FROM auction_rounds
        WHERE auction_id = $1
        ORDER BY round_num DESC
        LIMIT 1",
    )
    .bind(auction.id)
    .fetch_optional(&mut **locks.tx())
    .await
    .context("failed to query for concluded round")?;

    // If there's a previous round, update its results and check if auction
    // concluded
    if let Some(ref previous_round) = previous_round {
        let auction_continues = update_round_space_results_within_tx(
            auction,
            previous_round,
            locks,
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
            tracing::warn!(
                auction_id = ?auction.id,
                max_rounds = payloads::MAX_AUCTION_ROUNDS,
                "auction reached the round cap and was canceled; bid \
                 increment is likely too small relative to bidders' values",
            );
            cancel_runaway_auction(auction, locks, time_source).await?;
            return Ok(());
        }
    }

    let auction_params = sqlx::query_as::<_, store::AuctionParams>(
        "SELECT * FROM auction_params WHERE id = $1",
    )
    .bind(&auction.auction_params_id)
    .fetch_one(&mut **locks.tx())
    .await
    .context("getting auction params; skipping")?;

    let timezone =
        sqlx::query_as::<_, store::Site>("SELECT * FROM sites where id = $1")
            .bind(auction.site_id)
            .fetch_one(&mut **locks.tx())
            .await
            .context("getting site; skipping")?
            .timezone;

    // Only started auctions are processed by the scheduler, so a missing
    // start time here is a bug, not an expected state.
    let auction_start = auction
        .start_at
        .context("auction has no start time; cannot create rounds")?;
    let next_round_start = previous_round
        .as_ref()
        .map(|r| r.end_at)
        .unwrap_or(auction_start);
    let next_round_end = round_end_time(
        next_round_start,
        &timezone,
        auction_params.round_duration,
    )?;

    // Time-based runaway cancel: a backed-mode auction whose next round would
    // end past the fixed deadline (start + runtime) cancels with every hold
    // released — nobody pays. Bounding round end times, not just `now`, keeps
    // conclusion within the deadline by construction: the mint-site window
    // predicate and the age scan size against this same deadline, so no
    // mid-auction expiry handling exists. The `now` clause is a backstop for
    // scheduler outages longer than a round, where the prospective end can
    // still fit under the deadline while `now` is already past it; a backdated
    // round would conclude with no activity and settle winners past the window.
    let deadline = auction_start
        .checked_add(store::funding::runaway_deadline())
        .map_err(anyhow::Error::from)?;
    if next_round_end > deadline || time_source.now() >= deadline {
        let community_id =
            store::get_site_community_id(&auction.site_id, &mut **locks.tx())
                .await?;
        if store::funding::is_backed_mode(&community_id, &mut **locks.tx())
            .await?
        {
            tracing::warn!(
                auction_id = ?auction.id,
                %deadline,
                %next_round_end,
                "backed-mode auction cannot fit another round before its \
                 runaway deadline; canceling with all holds released",
            );
            cancel_runaway_auction(auction, locks, time_source).await?;
            return Ok(());
        }
    }

    // Create next round
    let new_round_id = add_subsequent_rounds_for_auction(
        auction,
        &previous_round,
        next_round_start,
        next_round_end,
        &auction_params,
        locks.tx(),
        time_source,
    )
    .await?;

    // Update eligibilities only if there was a previous round
    if let Some(ref previous_round) = previous_round {
        update_user_eligibilities(
            auction,
            previous_round,
            &new_round_id,
            locks.tx(),
        )
        .await?;
    }

    Ok(())
}

/// Cancel a runaway auction — one that hit [`payloads::MAX_AUCTION_ROUNDS`]
/// or whose next round would breach its time deadline
/// (`store::funding::runaway_deadline`, backed mode only; call sites log
/// which) — via the shared terminalization
/// (`store::auction::cancel_auction_tx`), on the scheduler's existing
/// transaction.
///
/// Lock contract: runs under the auction-processing lock (held from
/// selection by `lock_next_auction_needing_update`).
async fn cancel_runaway_auction(
    auction: &store::Auction,
    locks: &mut TrackedTx<'_, '_>,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    store::auction::cancel_auction_tx(&auction.id, time_source, locks)
        .await
        .context("failed to cancel runaway auction")?;
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
///
/// Lock contract: runs under the auction-processing lock (held from
/// selection, and excluding concurrent intent activations); on conclusion
/// it acquires the settlement entry's account locks, then marks intent rows,
/// then releases the auction's funding rows.
#[tracing::instrument(skip(locks, time_source))]
async fn update_round_space_results_within_tx(
    auction: &store::Auction,
    previous_round: &store::AuctionRound,
    locks: &mut TrackedTx<'_, '_>,
    time_source: &TimeSource,
) -> anyhow::Result<bool> {
    let tx = locks.tx();
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
        let community_id =
            store::get_site_community_id(&auction.site_id, &mut **tx)
                .await
                .context("failed to get community_id for auction settlement")?;

        // Create auction settlement journal entry
        store::currency::create_auction_settlement_entry(
            &community_id,
            &auction.id,
            winner_payments.clone(),
            time_source,
            locks,
        )
        .await
        .context("failed to create auction settlement journal entry")?;

        // Resolve the auction's card authorizations against the debits
        // just written: winners' holds are marked for capture (sized to
        // what balance couldn't cover), the rest for release.
        store::funding::settle_auction_funding_tx(
            &community_id,
            &auction.id,
            &winner_payments,
            time_source,
            locks,
        )
        .await
        .context("failed to settle auction card authorizations")?;
    }

    Ok(any_bids)
}

/// For an in-progress auction, create the next auction round as needed,
/// spanning `start_at` (the previous round's end, or the auction start for
/// round 0) to `end_at` (computed by the caller via [`round_end_time`]).
#[tracing::instrument(skip(tx))]
pub async fn add_subsequent_rounds_for_auction(
    auction: &store::Auction,
    previous_round: &Option<store::AuctionRound>,
    start_at: jiff::Timestamp,
    end_at: jiff::Timestamp,
    auction_params: &store::AuctionParams,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    time_source: &TimeSource,
) -> anyhow::Result<payloads::AuctionRoundId> {
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
    .bind(start_at.to_sqlx())
    .bind(end_at.to_sqlx())
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

/// Compute a round's end from its start, using DST-aware datetime math in the
/// site's timezone in case the round duration is days or larger.
fn round_end_time(
    start: jiff::Timestamp,
    timezone: &Option<String>,
    round_duration: jiff::Span,
) -> anyhow::Result<jiff::Timestamp> {
    let zoned_start = match start
        .in_tz(timezone.as_deref().unwrap_or("UTC"))
        .context("converting to timezone; falling back to DST-naive")
    {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("{e:#}");
            start.to_zoned(TimeZone::UTC)
        }
    };
    Ok(zoned_start
        .checked_add(round_duration)
        .context("computing round end time; skipping")?
        .timestamp())
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
#[derive(Debug, Clone, sqlx::FromRow)]
struct ProxyWorkItem {
    round_id: payloads::AuctionRoundId,
    auction_id: payloads::AuctionId,
    user_id: payloads::UserId,
    community_id: payloads::CommunityId,
    /// Stripe-backed backed_credits mode: the item may carry a card
    /// authorization leg, so its claim runs on the worker pool.
    backed: bool,
}

type ProxyItemKey = (payloads::AuctionRoundId, payloads::UserId);
type InflightSet = Arc<std::sync::Mutex<HashSet<ProxyItemKey>>>;

/// Removes its item key from the in-flight set when the owning task
/// finishes (including on panic — the guard is dropped either way).
struct InflightGuard {
    set: InflightSet,
    key: ProxyItemKey,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.set.lock().unwrap().remove(&self.key);
    }
}

/// The spawned backed-item tasks of the proxy worker: a `JoinSet` plus
/// the in-flight key set that stops overlapping selector passes from
/// re-spawning an item whose task is still running. The set is local to
/// this process and purely an optimization — a duplicate claim (from
/// this instance or another) just bounces off the `auction_user` pair
/// lock, but not before consuming a worker-pool slot to probe it;
/// correctness is entirely the lock plus state-driven re-selection.
/// `Scheduler::run` keeps one instance across loop iterations so a slow
/// task never delays the next pass; `schedule_tick` drains a fresh one so
/// tests observe completed work.
struct ProxyTasks {
    tasks: tokio::task::JoinSet<()>,
    inflight: InflightSet,
}

impl ProxyTasks {
    fn new() -> Self {
        Self {
            tasks: tokio::task::JoinSet::new(),
            inflight: Arc::new(std::sync::Mutex::new(HashSet::new())),
        }
    }

    /// Discard finished task handles without blocking, resuming any
    /// task's panic on the caller (see [`resume_task_panic`]).
    fn reap(&mut self) {
        while let Some(result) = self.tasks.try_join_next() {
            resume_task_panic(result);
        }
    }

    /// Await every spawned task — the deterministic completion barrier
    /// for `schedule_tick` (tests). Production loops never call this.
    /// Resumes any task's panic on the caller (see [`resume_task_panic`]).
    async fn drain(&mut self) {
        while let Some(result) = self.tasks.join_next().await {
            resume_task_panic(result);
        }
    }
}

/// Resume a joined proxy task's panic on the caller. A panicked claim
/// rolls back its transaction — including the failure marker — so the
/// item stays due and would panic again every tick; swallowing the
/// `JoinError` would make that loop invisible. Propagating panics the
/// proxy loop, which main treats as fatal (process exit for orchestrator
/// restart); in tests it fails the driving test. Cancellation errors
/// can't occur: nothing aborts these tasks.
fn resume_task_panic(result: Result<(), tokio::task::JoinError>) {
    if let Err(e) = result {
        std::panic::resume_unwind(e.into_panic());
    }
}

/// Run one selector pass over due (round, user) proxy work items:
/// process informal-mode items inline (ms-scale pure-DB claims) and
/// spawn backed-mode items as detached tasks in `tasks`, since their
/// claims may carry a ~1s Stripe call (`process_proxy_item` routes each
/// claim to the worker pool exactly when it carries one; the pool's
/// connection count is the concurrency bound). Returns once inline
/// items are done and backed tasks are spawned — the caller decides
/// whether to await them (see `ProxyTasks`).
///
/// Called from `Scheduler::run`'s proxy loop and `schedule_tick`. The
/// selection is lock-free, so a stale or duplicate candidate list is
/// harmless: each item's claim (`process_proxy_item`) try-locks its
/// `auction_user` pair key and losers skip. One user's failure lands on
/// that user's marker alone and never affects other items.
#[tracing::instrument(skip_all)]
async fn process_due_proxy_items(
    pool: &PgPool,
    worker_pool: &WorkerPool,
    time_source: &TimeSource,
    stripe_service: &Arc<StripeService>,
    tasks: &mut ProxyTasks,
) -> anyhow::Result<()> {
    tasks.reap();
    let items = list_due_proxy_items(pool, time_source).await?;
    if items.is_empty() {
        return Ok(());
    }
    tracing::debug!("Found {} due proxy work items", items.len());

    for item in &items {
        if item.backed {
            let key = (item.round_id, item.user_id);
            if !tasks.inflight.lock().unwrap().insert(key) {
                // A task from a previous pass still owns this item.
                continue;
            }
            let guard = InflightGuard {
                set: tasks.inflight.clone(),
                key,
            };
            let item = item.clone();
            let pool = pool.clone();
            let worker_pool = worker_pool.clone();
            let time_source = time_source.clone();
            let stripe_service = stripe_service.clone();
            tasks.tasks.spawn(async move {
                let _guard = guard;
                if let Err(e) = process_proxy_item(
                    &item,
                    &pool,
                    &worker_pool,
                    &time_source,
                    Some(&stripe_service),
                )
                .await
                {
                    tracing::error!(
                        "Failed to process proxy item (round {:?}, user \
                         {:?}): {:#}",
                        item.round_id,
                        item.user_id,
                        e
                    );
                }
            });
            continue;
        }
        // Per-item failures are recorded on the item's marker (backoff) and
        // must not stop the pass — that isolation is the point.
        if let Err(e) =
            process_proxy_item(item, pool, worker_pool, time_source, None).await
        {
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
        "SELECT ar.id AS round_id, ar.auction_id, upb.user_id,
            si.community_id,
            (c.currency_mode = 'backed_credits') AS backed
        FROM auction_rounds ar
        -- a.end_at excludes auctions canceled mid-round (the round row
        -- still spans now, but bidding into it would be pointless)
        JOIN auctions a ON ar.auction_id = a.id AND a.end_at IS NULL
        JOIN sites si ON a.site_id = si.id
        JOIN communities c ON si.community_id = c.id
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
        backoff = store::backoff_interval_sql("prp.failure_count"),
    ))
    .bind(time_source.now().to_sqlx())
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

/// Process one (round, user) proxy work item through funding cycles: each
/// bidding claim either completes the item's bids or commits an authorization
/// order sized to the first funding-short bid; execute the order
/// (`funding_flow::execute_auth_order`, the ~1s Stripe call on the worker pool)
/// and re-enter the claim against the enlarged backing. `stripe_service` is
/// Some exactly for backed-mode items.
///
/// The claim and the execute step are separate transactions by design: the
/// claim is ms-scale pure-DB work on the shared pool, and an authorization
/// without bids is a legal state (the pre-authorize flow produces it), so they
/// need no shared atomicity. The order-emitting claim records a failure marker
/// in its own commit, so a crash (or an execute error, which propagates from
/// here) re-selects the item after backoff, where the committed order is
/// re-found (get-or-create) and replayed.
///
/// The cycle bound is a defensive backstop, not the working limit: raise sizing
/// catches up or doubles (`raise_target`), so convergence is logarithmic in the
/// needed hold, not linear in funded bids.
async fn process_proxy_item(
    item: &ProxyWorkItem,
    pool: &PgPool,
    worker_pool: &WorkerPool,
    time_source: &TimeSource,
    stripe_service: Option<&StripeService>,
) -> anyhow::Result<()> {
    let card_path = if stripe_service.is_some()
        && store::funding::card_availability(
            &item.community_id,
            &item.user_id,
            pool,
        )
        .await?
            == payloads::responses::CardAvailability::Available
    {
        CardPath::Live
    } else {
        CardPath::Off
    };

    const MAX_FUNDING_CYCLES: usize = 10;
    for cycle in 0..MAX_FUNDING_CYCLES {
        let claim_cycle = if cycle == 0 {
            ClaimCycle::First
        } else {
            ClaimCycle::Continuation
        };
        match run_proxy_bid_claim(
            item,
            card_path,
            claim_cycle,
            pool,
            time_source,
        )
        .await?
        {
            ProxyClaimOutcome::Done => return Ok(()),
            ProxyClaimOutcome::OrderPlaced => {
                let ctx = crate::store::funding_flow::load_card_context(
                    &item.community_id,
                    &item.user_id,
                    pool,
                )
                .await?;
                match crate::store::funding_flow::execute_auth_order(
                    &ctx,
                    &item.community_id,
                    &item.auction_id,
                    &item.user_id,
                    Presence::Automatic,
                    FlowDeps {
                        worker_pool,
                        time_source,
                        stripe_service: stripe_service
                            .expect("order implies stripe service"),
                    },
                )
                .await?
                {
                    AuthOrderOutcome::LockMiss => {
                        // Another claimant owns this (auction, user);
                        // the failure marker re-selects the item.
                        return Ok(());
                    }
                    AuthOrderOutcome::Declined(decline) => {
                        // The decline pauses further orders; the next
                        // cycle places what existing backing allows.
                        tracing::info!(
                            user_id = ?item.user_id,
                            auction_id = ?item.auction_id,
                            code = ?decline.code,
                            decline_code = ?decline.decline_code,
                            "proxy authorization declined; bidding with \
                             existing backing"
                        );
                    }
                    _ => {}
                }
            }
        }
    }
    tracing::warn!(
        user_id = ?item.user_id,
        auction_id = ?item.auction_id,
        "proxy funding cycles exhausted without completing; backoff \
         re-selection continues from the failure marker"
    );
    Ok(())
}

/// The bidding claim's verdict for `process_proxy_item`'s cycle loop.
enum ProxyClaimOutcome {
    /// The item is processed (or no longer applicable): bids placed up
    /// to balance/backing limits, marker written; stop.
    Done,
    /// The claim committed an authorization order sized to the first
    /// funding-short bid (with a failure marker for crash-safe
    /// re-selection); execute it and re-enter.
    OrderPlaced,
}

/// Whether an item's card-order leg is live: backed mode with a
/// charges-enabled community, a saved card, and the member's charge
/// grant. Checked once per item, like the member bid flow's peek.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardPath {
    Live,
    Off,
}

/// Which cycle of `process_proxy_item`'s loop a claim runs.
/// Continuations follow an executed order and skip the dueness probe:
/// the order-emitting cycle just recorded a failure marker, and backoff
/// would report not-due mid-pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClaimCycle {
    First,
    Continuation,
}

/// Claim and run one cycle of an item's proxy bidding — flag-clear,
/// dueness re-verify, bids (and, on a card-backed funding shortfall,
/// the sized authorization order), and outcome marker in one
/// transaction whose single commit makes them atomic; a crash discards
/// all of it (including the flag clear), so the item is simply
/// re-selected.
///
/// "Claim" because its advisory try-lock on the `auction_user` pair
/// key is what claims the item (losing contenders bounce off the probe
/// and never queue behind the row lock). Always on the shared pool:
/// this claim never carries a Stripe call.
///
/// Lock contract: acquires the pair try-lock, then the member's
/// `use_proxy_bidding` row (concurrent settings writers block on it,
/// land strictly after the commit, and re-set the flag), then per bid
/// the member's account and funding rows inside the work savepoint. The
/// pair lock doubles as the order-transaction authority: on a funding
/// shortfall the savepoint writes the pending intent row
/// (`funding_flow::order_proxy_bid_auth_tx`) — the one card-state
/// write this claim makes.
async fn run_proxy_bid_claim(
    item: &ProxyWorkItem,
    card_path: CardPath,
    cycle: ClaimCycle,
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<ProxyClaimOutcome> {
    let mut locks = TrackedTx::begin(pool).await?;
    if !locks
        .acquire_pair(&item.auction_id, &item.user_id, LockWait::Try)
        .await?
    {
        // Another claimant owns this (auction, user); it will process or
        // the item stays due and is re-selected next tick.
        return Ok(ProxyClaimOutcome::Done);
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
    .fetch_optional(&mut **locks.tx())
    .await?;
    let Some(settings) = settings else {
        // Proxy bidding was disabled after the candidate was listed
        return Ok(ProxyClaimOutcome::Done);
    };
    if settings.needs_processing {
        sqlx::query(
            "UPDATE use_proxy_bidding SET needs_processing = FALSE
            WHERE user_id = $1 AND auction_id = $2",
        )
        .bind(item.user_id)
        .bind(item.auction_id)
        .execute(&mut **locks.tx())
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
    .fetch_optional(&mut **locks.tx())
    .await?;
    let Some(round) = round else {
        // Round ended, or the auction was canceled mid-round; a next
        // round's baseline arm covers any reprocessing
        locks.rollback().await?;
        return Ok(ProxyClaimOutcome::Done);
    };
    if cycle == ClaimCycle::First {
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
            backoff = store::backoff_interval_sql("failure_count"),
        ))
        .bind(item.round_id)
        .bind(item.user_id)
        .bind(settings.needs_processing)
        .bind(now.to_sqlx())
        .fetch_one(&mut **locks.tx())
        .await?;
        if !due {
            locks.rollback().await?;
            return Ok(ProxyClaimOutcome::Done);
        }
    }

    // Run the bidding work inside a savepoint so a failure can be recorded
    // on the marker while the flag stays cleared (a writer-side signal
    // only) and the claim commits — re-selection then goes through backoff,
    // or immediately via the flag if the member changes inputs.
    let outcome = async {
        let mut work = locks.savepoint().await?;
        match run_proxy_item_work(
            &settings,
            &round,
            &item.community_id,
            card_path,
            &mut work,
            time_source,
        )
        .await
        {
            Ok(outcome) => {
                work.commit().await?;
                Ok(outcome)
            }
            Err(e) => {
                work.rollback().await?;
                Err(e)
            }
        }
    }
    .await;

    // Marker: a completed cycle resets it; an order-emitting cycle or a
    // failed one records a failure, so a crash (or an execute error)
    // re-selects the item after backoff. The order-emitting cycle's
    // marker lands in the same commit as its order, so re-selection
    // always finds the committed pending row; the completing cycle's
    // success marker resets the count.
    if matches!(
        &outcome,
        Ok(ProxyWorkOutcome {
            order_placed: false
        })
    ) {
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
        .execute(&mut **locks.tx())
        .await?;
    } else {
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
        .execute(&mut **locks.tx())
        .await?;
    }
    locks.commit().await?;
    match outcome {
        Ok(ProxyWorkOutcome {
            order_placed: false,
        }) => Ok(ProxyClaimOutcome::Done),
        Ok(ProxyWorkOutcome { order_placed: true }) => {
            Ok(ProxyClaimOutcome::OrderPlaced)
        }
        Err(e) => Err(e),
    }
}

/// Work off intent rows owing a Stripe call, one claim per row. Runs
/// each tick from `Scheduler::run`'s intent loop and from
/// `schedule_tick`.
///
/// The arms: cancels for `superseded` (swap-reauth predecessors) and
/// `release_pending` (settled or canceled with nothing owed) rows,
/// captures for `capture_pending` rows, plus a catchall cancel for
/// authorizations still active after their auction ended — a claim
/// racing conclusion can activate one after the settlement pass read
/// its snapshot, and state re-selection here releases it without that
/// race needing to be prevented. One arm carries no Stripe call: aged
/// `pending` orders whose execute never ran are canceled locally so
/// their stale amount stops being reusable
/// (`cancel_aged_pending_order`).
///
/// Rows keep their status until the Stripe call succeeds, so skipped or
/// failed items are re-found from state alone; failures back off via
/// the worker columns. Each item is claimed via try-lock on its
/// `auction_user` pair key in a claim tx on the worker pool (every arm
/// carries a Stripe call).
#[tracing::instrument(skip_all)]
async fn process_intent_work(
    pool: &PgPool,
    worker_pool: &WorkerPool,
    time_source: &TimeSource,
    stripe_service: &StripeService,
) -> anyhow::Result<()> {
    let now = time_source.now();
    let candidates: Vec<WorkerIntent> = sqlx::query_as(&format!(
        "SELECT fi.id, fi.status, fi.payment_intent_id, fi.capture_amount,
            fi.capture_before, fi.origin, fi.authorized_amount,
            fi.auction_id, s.community_id, fi.user_id,
            c.stripe_account_id, c.currency_name
        FROM funding_intents fi
        JOIN auctions a ON fi.auction_id = a.id
        JOIN sites s ON a.site_id = s.id
        JOIN communities c ON s.community_id = c.id
        WHERE (
                fi.status IN ('superseded', 'release_pending',
                              'capture_pending')
                OR (fi.status = 'authorized' AND fi.is_active
                    AND a.end_at IS NOT NULL)
                -- Age scan: pre-start holds whose window can't cover
                -- the auction deadline (known start + runtime +
                -- margin; now-based while the start is unknown — an
                -- immediate start must stay coverable). The same
                -- comparison as the mint-site predicate, so a
                -- postponement's invalidation is discovered at the
                -- next tick, not just-in-time before the start.
                OR (fi.status = 'authorized' AND fi.is_active
                    AND a.end_at IS NULL
                    AND (a.start_at IS NULL OR a.start_at > $1)
                    AND fi.capture_before IS NOT NULL
                    AND fi.capture_before < COALESCE(a.start_at, $1)
                        + $2 * INTERVAL '1 hour')
                -- Stranded orders: a pending row whose execute never
                -- ran ages out and is canceled locally, so the next
                -- genuine need orders fresh instead of reusing the
                -- stale amount.
                OR (fi.status = 'pending'
                    AND fi.created_at + $3 * INTERVAL '1 hour' <= $1)
            )
            AND (
                fi.worker_failure_count = 0
                OR fi.worker_last_failed_at IS NULL
                OR $1 > fi.worker_last_failed_at + {backoff}
            )",
        backoff = store::backoff_interval_sql("fi.worker_failure_count"),
    ))
    .bind(now.to_sqlx())
    .bind(store::funding::min_viable_window_hours() as f64)
    .bind(store::funding::stale_order_age_hours() as f64)
    .fetch_all(pool)
    .await?;

    let deps = FlowDeps {
        worker_pool,
        time_source,
        stripe_service,
    };
    for intent in &candidates {
        let result = match intent.status {
            payloads::FundingIntentStatus::CapturePending => {
                capture_intent(intent, deps).await
            }
            payloads::FundingIntentStatus::Pending => {
                cancel_aged_pending_order(intent, worker_pool, time_source)
                    .await
            }
            _ => cancel_worker_intent(intent, deps).await,
        };
        if let Err(e) = result {
            tracing::error!(
                "intent worker failed for {:?} ({}): {:#}",
                intent.id,
                intent.status,
                e
            );
        }
    }
    Ok(())
}

/// Deliver enqueued notification emails (the outbox drain; see
/// `store::notifications` for the outbox design). List-then-claim like
/// the proxy worker: list due rows lock-free on the shared pool, then
/// claim each with `FOR UPDATE SKIP LOCKED` for the send. A failed
/// send records failure backoff on the row and delivery retries next
/// tick.
async fn process_notification_outbox(
    pool: &PgPool,
    worker_pool: &WorkerPool,
    time_source: &TimeSource,
    email_service: &EmailService,
) -> anyhow::Result<()> {
    let due =
        store::notifications::list_due_notification_ids(pool, time_source)
            .await?;
    for id in due {
        if let Err(e) =
            deliver_notification(&id, worker_pool, time_source, email_service)
                .await
        {
            tracing::error!("notification delivery failed for {id}: {e:#}");
        }
    }
    Ok(())
}

/// Mint scheduled pre-authorizations for proxy participants of
/// backed-mode auctions whose known start falls within the advance
/// window (T−24h): each candidate gets a strategy-sized hold via the
/// shared preauth core (origin `scheduled_preauth`), so their card is
/// warmed before rounds begin. Members without a saved card or the
/// community grant never select — their proxies bid balance-only. A
/// budget member with no `user_values` sizes a zero target and no-ops
/// (they authorize reactively when a bid first needs it).
///
/// Try-and-skip: the need is re-derived each pass, so contended or
/// skipped candidates simply stay selectable. Selection excludes
/// members with any live active authorization (fresh mints only —
/// undersized holds grow reactively at bid time), pending orders
/// under worker backoff (a transient Stripe failure records backoff on
/// the order row rather than retrying at tick rate), and members whose
/// latest intent carries decline metadata — the ordinary
/// automatic-attempt pause; a member-present success naturally lifts
/// it. The decline itself notifies the member once (enqueued in the
/// execute claim).
async fn process_scheduled_preauths(
    pool: &PgPool,
    worker_pool: &WorkerPool,
    time_source: &TimeSource,
    stripe_service: &StripeService,
) -> anyhow::Result<()> {
    let now = time_source.now();
    let horizon = now
        .checked_add(store::funding::preauth_advance())
        .map_err(anyhow::Error::from)?;
    let candidates: Vec<(
        payloads::AuctionId,
        payloads::CommunityId,
        payloads::UserId,
    )> = sqlx::query_as(&format!(
        "SELECT a.id, si.community_id, upb.user_id
         FROM auctions a
         JOIN sites si ON a.site_id = si.id AND si.deleted_at IS NULL
         JOIN communities c ON si.community_id = c.id
         JOIN use_proxy_bidding upb ON upb.auction_id = a.id
         JOIN user_payment_profiles upp ON upp.user_id = upb.user_id
             AND upp.payment_method_id IS NOT NULL
         JOIN community_members cm ON cm.community_id = si.community_id
             AND cm.user_id = upb.user_id
             AND cm.card_charges_granted_at IS NOT NULL
         WHERE c.currency_mode = 'backed_credits'
             AND {account_operational}
             AND a.end_at IS NULL
             AND a.start_at IS NOT NULL
             AND a.start_at > $1
             AND a.start_at <= $2
             AND NOT EXISTS (
                 SELECT 1 FROM funding_intents fi
                 WHERE fi.auction_id = a.id AND fi.user_id = upb.user_id
                     AND ((fi.is_active AND fi.status = 'authorized')
                         OR (fi.status = 'pending'
                             AND fi.worker_failure_count > 0
                             AND fi.worker_last_failed_at IS NOT NULL
                             AND $1 <= fi.worker_last_failed_at
                                 + {backoff})
                         -- Decline pause: the latest intent (a declined
                         -- confirm leaves a canceled row with decline
                         -- metadata) blocks automatic attempts.
                         OR (fi.last_decline_at IS NOT NULL
                             AND fi.id = (
                                 SELECT id FROM funding_intents
                                 WHERE auction_id = a.id
                                   AND user_id = upb.user_id
                                 ORDER BY created_at DESC, id DESC
                                 LIMIT 1)))
             )",
        backoff = store::backoff_interval_sql("fi.worker_failure_count"),
        account_operational = store::connect::account_operational_sql("c"),
    ))
    .bind(now.to_sqlx())
    .bind(horizon.to_sqlx())
    .fetch_all(pool)
    .await?;

    for (auction_id, community_id, user_id) in candidates {
        if let Err(e) = process_scheduled_preauth_candidate(
            &auction_id,
            &community_id,
            &user_id,
            pool,
            worker_pool,
            time_source,
            stripe_service,
        )
        .await
        {
            tracing::error!(
                ?auction_id,
                ?user_id,
                "scheduled pre-authorization failed: {e:#}"
            );
        }
    }
    Ok(())
}

/// Run one scheduled pre-auth candidate through the shared preauth
/// core. An operational Stripe error records worker backoff on the
/// pending order row so selection paces the retries.
async fn process_scheduled_preauth_candidate(
    auction_id: &payloads::AuctionId,
    community_id: &payloads::CommunityId,
    user_id: &payloads::UserId,
    pool: &PgPool,
    worker_pool: &WorkerPool,
    time_source: &TimeSource,
    stripe_service: &StripeService,
) -> anyhow::Result<()> {
    // Prerequisites re-validate here (the selection's join is a cheap
    // approximation); a context that fails to load — e.g. the community
    // deauthorized since selection — just skips the candidate.
    let ctx = match crate::store::funding_flow::load_card_context(
        community_id,
        user_id,
        pool,
    )
    .await
    {
        Ok(ctx) => ctx,
        Err(e) => {
            tracing::debug!(
                ?auction_id,
                ?user_id,
                "scheduled pre-auth skipped: card context unavailable \
                 ({e:#?})"
            );
            return Ok(());
        }
    };

    let outcome = crate::store::funding_flow::run_preauth(
        &ctx,
        community_id,
        auction_id,
        user_id,
        None,
        payloads::FundingIntentOrigin::ScheduledPreauth,
        Presence::Automatic,
        FlowDeps {
            worker_pool,
            time_source,
            stripe_service,
        },
    )
    .await;
    match outcome {
        Ok(AuthOrderOutcome::Authorized) => {
            tracing::info!(
                ?auction_id,
                ?user_id,
                "scheduled pre-authorization minted"
            );
            Ok(())
        }
        Ok(AuthOrderOutcome::Declined(decline)) => {
            tracing::warn!(
                ?auction_id,
                ?user_id,
                code = ?decline.code,
                decline_code = ?decline.decline_code,
                "scheduled pre-authorization declined; automatic \
                 attempts paused"
            );
            Ok(())
        }
        Ok(_) => Ok(()),
        Err(e) => {
            // Pace retries via the pending order's worker backoff (the
            // selection's NOT EXISTS arm); without this a persistent
            // Stripe error would retry at tick rate for the whole
            // advance window.
            let pending: Option<payloads::FundingIntentId> =
                sqlx::query_scalar(
                    "SELECT id FROM funding_intents \
                     WHERE auction_id = $1 AND user_id = $2 \
                       AND status = 'pending'",
                )
                .bind(auction_id)
                .bind(user_id)
                .fetch_optional(pool)
                .await?;
            if let Some(intent_id) = pending {
                let mut tx = pool.begin().await?;
                store::funding::record_worker_failure_tx(
                    &intent_id,
                    time_source,
                    &mut tx,
                )
                .await?;
                tx.commit().await?;
            }
            Err(e.into())
        }
    }
}

/// Claim and deliver one outbox row. The claim transaction holds only
/// the outbox row lock across the send — permissible because the
/// drainer is the row's only writer — and runs on the worker pool,
/// which exists precisely so transactions spanning network calls never
/// pin shared API-pool connections (and its session-timeout backstop
/// covers a hung send).
async fn deliver_notification(
    id: &uuid::Uuid,
    worker_pool: &WorkerPool,
    time_source: &TimeSource,
    email_service: &EmailService,
) -> anyhow::Result<()> {
    let mut tx = worker_pool.0.begin().await?;
    let Some(due) =
        store::notifications::claim_due_notification_tx(id, &mut tx).await?
    else {
        // Sent meanwhile, or another drainer owns it.
        return Ok(());
    };
    let template = due.params.0.render(&due.username, email_service.base_url());
    match email_service.send_email(&due.email, template).await {
        Ok(()) => {
            store::notifications::mark_notification_sent_tx(
                id,
                time_source,
                &mut tx,
            )
            .await?;
            tx.commit().await?;
            tracing::info!(notification_id = %id, "notification delivered");
            Ok(())
        }
        Err(e) => {
            store::notifications::record_notification_failure_tx(
                id,
                time_source,
                &mut tx,
            )
            .await?;
            tx.commit().await?;
            Err(e)
        }
    }
}

/// What `run_proxy_item_work` observed, for `run_proxy_bid_claim`'s
/// marker decision and `process_proxy_item`'s cycle loop.
struct ProxyWorkOutcome {
    /// A pending authorization order was committed for the first
    /// funding-short bid; the walk stopped there. Execute the order and
    /// re-enter the claim.
    order_placed: bool,
}

/// Re-place one member's bids for the round inside the claim's
/// savepoint: delete their existing bids, then bid on their positive-
/// surplus spaces (best surplus first) until `max_items` is reached.
///
/// Skips on eligibility/already-winning are expected and per-space. A
/// bid failing on insufficient funding orders a card authorization
/// sized to that exact bid when the card path is live
/// (`funding_flow::order_proxy_bid_auth_tx`) and stops the walk —
/// continuing to cheaper spaces would consume the balance the gap was
/// just sized against; without a card leg (or during a decline pause)
/// the skip is the member's honest balance limit and the walk tries the
/// next space.
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
    community_id: &payloads::CommunityId,
    card_path: CardPath,
    ttx: &mut TrackedTx<'_, '_>,
    time_source: &TimeSource,
) -> anyhow::Result<ProxyWorkOutcome> {
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
    .fetch_all(&mut **ttx.tx())
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
        .fetch_all(&mut **ttx.tx())
        .await
        .context("failed to get round results")?;

    // Get the auction params for the bid increment
    let auction_params = sqlx::query_as::<_, store::AuctionParams>(
        "SELECT * FROM auction_params ap
        JOIN auctions a on ap.id = a.auction_params_id
        WHERE a.id = $1",
    )
    .bind(round.auction_id)
    .fetch_one(&mut **ttx.tx())
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
    .execute(&mut **ttx.tx())
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
        .fetch_all(&mut **ttx.tx())
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

    // In a capped auction, precompute the user's cap budgets so the walk
    // never attempts a bid that cap validation would reject. The
    // enforcement rule is shared with `create_bid_tx`
    // (`CapBudgets::check`), so the pre-filter cannot diverge from the
    // authoritative check; the walk debits each placed bid to keep the
    // state current. Spaces whose category the user has no cap for
    // (including the NULL bucket) are skipped entirely. None when the
    // auction is uncapped.
    //
    // Caps get this precomputation while eligibility stays reactive (attempt
    // the bid, skip on rejection) for historical reasons. Pre-checking reduces
    // round-trips and could be implemented for eligibility points as well.
    // Eligibility is useful when space quantity is encoded in eligibility, and
    // bidders may set max_items high since eligibility points are what then set
    // their bidding limits, rather than the number of spaces. Defining proxy
    // bid limits by points would be more useful to bidders in that case.
    let mut cap_budgets =
        store::caps::fetch_cap_budgets(round, &settings.user_id, ttx.tx())
            .await
            .context("failed to fetch cap budgets")?;

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
    let mut order_placed = false;
    for (space_id, surplus, _value) in space_surpluses {
        if successful_bids + num_spaces_already_winning
            >= settings.max_items as usize
        {
            break;
        }

        if let Some(budgets) = &cap_budgets {
            let space = &spaces[&space_id];
            if budgets
                .check(space.category_id, space.eligibility_points)
                .is_err()
            {
                tracing::debug!(
                    "Skipping {:?}: category cap budget exhausted or missing",
                    space_id
                );
                continue;
            }
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
            ttx,
            time_source,
        )
        .await
        {
            Ok(_) => {
                successful_bids += 1;
                if let Some(budgets) = &mut cap_budgets {
                    let space = &spaces[&space_id];
                    budgets.debit(space.category_id, space.eligibility_points);
                }
                tracing::info!("Successfully placed bid on {:?}", space_id);
            }
            Err(store::StoreError::Api(
                ApiError::ExceedsEligibility { .. }
                | ApiError::ExceedsBidderCap { .. }
                | ApiError::AlreadyWinningSpace,
            )) => {
                // Expected errors - try next space. A cap rejection should
                // not occur given the pre-filter above, but is handled the
                // same way defensively.
                tracing::info!(
                    "Failed to bid on {:?}: eligibility, cap, or already \
                     winning",
                    space_id
                );
                continue;
            }
            Err(store::StoreError::Api(ApiError::InsufficientBalance)) => {
                // The failed bid passed every other gate (eligibility is
                // checked before funding), so it is exactly what the card
                // must back: order reactively and stop.
                if card_path == CardPath::Live
                    && crate::store::funding_flow::order_proxy_bid_auth_tx(
                        community_id,
                        &spaces[&space_id],
                        &round.id,
                        &settings.user_id,
                        time_source,
                        ttx,
                    )
                    .await?
                {
                    order_placed = true;
                    break;
                }
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
        ttx.tx(),
        &payloads::AuctionEvent::BidsChanged {
            auction_id: round.auction_id,
            round_id: round.id,
            user_id: settings.user_id,
        },
    )
    .await?;

    tracing::info!("Placed {} successful new bids", successful_bids,);

    Ok(ProxyWorkOutcome { order_placed })
}

/// Run the hourly reconciliation pass when due: the advisory-lock gate
/// plus any-community-stale check (`try_claim_reconciliation_pass`),
/// the full pass (`store::reconciliation::run_reconciliation_pass`),
/// and the watermark restamp committing with the gate.
///
/// Called every tick from `run()` and `schedule_tick`; the gate makes
/// the frequent calls cheap and deduplicates instances. The gate runs
/// on the worker pool: it is held across the whole Stripe-heavy pass,
/// and a pinned connection must not come out of the API pool.
async fn run_reconciliation_if_due(
    pool: &PgPool,
    worker_pool: &WorkerPool,
    time_source: &TimeSource,
    stripe_service: &StripeService,
) -> anyhow::Result<()> {
    let mut gate = worker_pool.0.begin().await?;
    if !store::reconciliation::try_claim_reconciliation_pass(
        time_source,
        &mut gate,
    )
    .await?
    {
        tracing::trace!("reconciliation not due or already running");
        return Ok(());
    }
    store::reconciliation::run_reconciliation_pass(
        pool,
        worker_pool,
        time_source,
        stripe_service,
    )
    .await?;
    store::reconciliation::record_reconciliation_pass(time_source, &mut gate)
        .await?;
    gate.commit().await?;
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
