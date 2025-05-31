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
//! |------------|---|---|---|---|
//!       ^      ^   ^
//!       |      |   round concludes, space_rounds updates with results,
//!       |      |   new rounds are created if there is still activity
//!       |      |
//!       | auction start
//!       |
//! proxy_bidding_lead_time
//! ```

use anyhow::Context;
use jiff::tz::TimeZone;
use jiff_sqlx::ToSqlx;
use sqlx::PgPool;
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
            schedule_tick(&self.pool, &self.time_source).await;
        }
    }
}

/// Update state once right now.
#[tracing::instrument(skip(pool, time_source))]
pub async fn schedule_tick(pool: &PgPool, time_source: &TimeSource) {
    // tracing::instrument will log the errors if they occur
    let _ = store::update_is_active_from_schedule(pool, time_source)
        .await
        .map_err(log_error);
    let _ = update_space_rounds(pool, time_source)
        .await
        .map_err(log_error);
    // TODO: update user eligibilities after a round
    let _ = add_subsequent_rounds(pool, time_source)
        .await
        .map_err(log_error);
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
/// This function only fails early if it cannot read the auctions that need
/// updating. If updating a specific auction, errors are logged, but do not
/// prevent updates to other auctions in the queue.
#[tracing::instrument(skip(pool, time_source))]
pub async fn update_space_rounds(
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    // Get all auctions where the start time is in the past, the auction hasn't
    // yet concluded, and there is not an ongoing auction round (the end time in
    // the future).
    let auctions = sqlx::query_as::<_, store::Auction>(
        "SELECT * FROM auctions
        WHERE $1 >= start_at
            AND end_at IS NULL
            AND NOT EXISTS (
                SELECT 1 FROM auction_rounds
                WHERE auction_id = auctions.id
                AND $1 < end_at
            )",
    )
    .bind(time_source.now().to_sqlx())
    .fetch_all(pool)
    .await?;

    for auction in &auctions {
        let _ = update_space_rounds_for_auction(auction, pool, time_source)
            .await
            .map_err(log_error);
    }

    Ok(())
}

#[tracing::instrument(skip(pool, time_source))]
pub async fn update_space_rounds_for_auction(
    auction: &store::Auction,
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    // Get the auction params to know the bid increment
    let auction_params = sqlx::query_as::<_, store::AuctionParams>(
        "SELECT * FROM auction_params WHERE id = $1",
    )
    .bind(&auction.auction_params_id)
    .fetch_one(pool)
    .await
    .context("failed to get auction params")?;

    // Get all spaces for this auction's site
    let spaces = sqlx::query_as::<_, store::Space>(
        "SELECT * FROM spaces WHERE site_id = $1 AND is_available = true",
    )
    .bind(auction.site_id)
    .fetch_all(pool)
    .await
    .context("failed to get available spaces for site")?;

    // Get the most recently concluded round
    let concluded_round = sqlx::query_as::<_, store::AuctionRound>(
        "SELECT * FROM auction_rounds 
        WHERE auction_id = $1 
        AND end_at <= $2
        ORDER BY round_num DESC
        LIMIT 1",
    )
    .bind(auction.id)
    .bind(time_source.now().to_sqlx())
    .fetch_optional(pool)
    .await
    .context("failed to query for concluded round")?;

    // If there's no concluded round yet, nothing to update
    let Some(concluded_round) = concluded_round else {
        return Ok(());
    };

    let mut tx = pool.begin().await.context("failed to begin transaction")?;
    let mut any_bids = false;

    for space in &spaces {
        // Check how many bids exist for this space in the concluded round
        let bid_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM bids 
            WHERE space_id = $1 AND round_id = $2",
        )
        .bind(space.id)
        .bind(concluded_round.id)
        .fetch_one(&mut *tx)
        .await
        .with_context(|| {
            format!("failed to get bid count for space {}", space.id)
        })?;

        // Track if there are any bids
        any_bids = any_bids || bid_count > 0;

        // Get previous value if it exists
        let prev_value = sqlx::query_scalar::<_, rust_decimal::Decimal>(
            "SELECT value FROM space_rounds 
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
        .bind(concluded_round.round_num)
        .fetch_optional(&mut *tx)
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
            .bind(concluded_round.id)
            .fetch_one(&mut *tx)
            .await
            .with_context(|| {
                format!("failed to select winning bid for space {}", space.id)
            })?;

            let new_value = match prev_value {
                Some(prev) => prev + auction_params.bid_increment,
                None => rust_decimal::Decimal::ZERO, // Start at zero in first round
            };

            (new_value, Some(winner))
        } else {
            // No bids, value stays at zero
            (rust_decimal::Decimal::ZERO, None)
        };

        // Create space round entry
        sqlx::query(
            "INSERT INTO space_rounds (
                space_id, 
                round_id,
                winning_user_id,
                value
            ) VALUES ($1, $2, $3, $4)",
        )
        .bind(space.id)
        .bind(concluded_round.id)
        .bind(winning_user_id)
        .bind(new_value)
        .execute(&mut *tx)
        .await
        .with_context(|| {
            format!("failed to create space round entry for space {}", space.id)
        })?;
    }

    // Conclude the auction if there are no more bids
    if !any_bids {
        sqlx::query(
            "UPDATE auctions 
            SET end_at = $1 
            WHERE id = $2",
        )
        .bind(concluded_round.end_at.to_sqlx())
        .bind(auction.id)
        .execute(&mut *tx)
        .await
        .with_context(|| {
            format!("failed to conclude auction {}", auction.id)
        })?;
    }

    tx.commit().await.context("failed to commit transaction")?;

    Ok(())
}

/// For auctions that are in progress, create the next auction round as needed.
///
/// This function only fails early if it cannot read the auctions that need
/// updating. If updating a specific auction, errors are logged, but do not
/// prevent updates to other auctions in the queue.
#[tracing::instrument(skip(pool, time_source))]
pub async fn add_subsequent_rounds(
    pool: &PgPool,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    // Get all auctions where the start time is in the past, the auction hasn't
    // yet concluded, and there is not an ongoing auction round (the end time in
    // the future).
    let auctions = sqlx::query_as::<_, store::Auction>(
        "SELECT * FROM auctions
        WHERE $1 >= start_at
            AND end_at IS NULL
            AND NOT EXISTS (
                SELECT 1 FROM auction_rounds
                WHERE auction_id = auctions.id
                AND $1 < end_at
            )",
    )
    .bind(time_source.now().to_sqlx())
    .fetch_all(pool)
    .await?;

    for auction in &auctions {
        let _ = add_subsequent_rounds_for_auction(auction, pool)
            .await
            .map_err(log_error);
    }

    Ok(())
}

#[tracing::instrument(skip(pool))]
pub async fn add_subsequent_rounds_for_auction(
    auction: &store::Auction,
    pool: &PgPool,
) -> anyhow::Result<()> {
    let auction_params = sqlx::query_as::<_, store::AuctionParams>(
        "SELECT * FROM auction_params WHERE id = $1",
    )
    .bind(&auction.auction_params_id)
    .fetch_one(pool)
    .await
    .context("getting auction params; skipping")?;

    let timezone =
        sqlx::query_as::<_, store::Site>("SELECT * FROM sites where id = $1")
            .bind(auction.site_id)
            .fetch_one(pool)
            .await
            .context("getting site; skipping")?
            .timezone;

    let last_auction_round = sqlx::query_as::<_, store::AuctionRound>(
        "SELECT * FROM auction_rounds
            WHERE auction_id = $1
            ORDER BY round_num DESC LIMIT 1",
    )
    .bind(auction.id)
    .fetch_optional(pool)
    .await
    .context("getting last auction round; skipping")?;

    let start_time_ts = last_auction_round
        .as_ref()
        .map(|r| r.end_at)
        .unwrap_or(auction.start_at);

    // use DST-aware datetime math in case the round duration is days or
    // larger
    let zoned_start_time = match start_time_ts
        .in_tz(&timezone)
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

    let round_num: i32 =
        last_auction_round.map(|r| r.round_num + 1).unwrap_or(0);

    let eligibility_threshold = get_eligibility_for_round_num(
        round_num,
        &auction_params.activity_rule_params.eligibility_progression,
    );

    sqlx::query(
        "INSERT INTO auction_rounds (
                auction_id,
                round_num,
                start_at,
                end_at,
                eligibility_threshold
            ) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(auction.id)
    .bind(round_num)
    .bind(start_time_ts.to_sqlx())
    .bind(zoned_end_time.timestamp().to_sqlx())
    .bind(eligibility_threshold)
    .execute(pool)
    .await
    .context("inserting round into database")?;

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
