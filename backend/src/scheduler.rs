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
    // tracing::instrument will log the errors if they occur
    let _ = store::update_is_active_from_schedule(pool, time_source)
        .await
        .map_err(log_error);

    // Get all auctions where the start time is in the past, the auction hasn't
    // yet concluded, and there is not an ongoing auction round (the end time in
    // the future).
    let auctions_without_ongoing_round = sqlx::query_as::<_, store::Auction>(
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

    for auction in &auctions_without_ongoing_round {
        let previous_round = sqlx::query_as::<_, store::AuctionRound>(
            "SELECT * FROM auction_rounds 
            WHERE auction_id = $1 
            ORDER BY round_num DESC
            LIMIT 1",
        )
        .bind(auction.id)
        .fetch_optional(pool)
        .await
        .context("failed to query for concluded round")?;

        // If there's a previous round, check if it had activity
        if let Some(previous_round) = &previous_round {
            if let Ok(false) = update_round_space_results_for_auction(
                auction,
                previous_round,
                pool,
            )
            .await
            .map_err(log_error)
            {
                // auction has concluded
                continue;
            }
        }

        // Create next round and update eligibilities atomically
        let mut tx = pool.begin().await?;
        match add_subsequent_rounds_for_auction(
            auction,
            &previous_round,
            &mut tx,
        )
        .await
        .map_err(log_error)
        {
            Ok(new_round_id) => {
                // Update eligibilities only if there was a previous round
                if let Some(previous_round) = &previous_round {
                    if let Err(e) = update_user_eligibilities(
                        auction,
                        previous_round,
                        &new_round_id,
                        &mut tx,
                    )
                    .await
                    {
                        log_error(e);
                        let _ = tx.rollback().await;
                        continue;
                    }
                }

                if let Err(e) = tx.commit().await {
                    log_error(anyhow::Error::from(e));
                }
            }
            Err(_) => {
                let _ = tx.rollback().await;
            }
        }
    }
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
/// This function only fails early if it cannot read the auctions that need
/// updating. If updating a specific auction, errors are logged, but do not
/// prevent updates to other auctions in the queue.
///
/// Returns whether the auction is still ongoing.
#[tracing::instrument(skip(pool))]
pub async fn update_round_space_results_for_auction(
    auction: &store::Auction,
    previous_round: &store::AuctionRound,
    pool: &PgPool,
) -> anyhow::Result<bool> {
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

    let mut tx = pool.begin().await.context("failed to begin transaction")?;
    let mut any_bids = false;

    for space in &spaces {
        // Check how many bids exist for this space in the concluded round
        let bid_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM bids 
            WHERE space_id = $1 AND round_id = $2",
        )
        .bind(space.id)
        .bind(previous_round.id)
        .fetch_one(&mut *tx)
        .await
        .with_context(|| {
            format!("failed to get bid count for space {}", space.id)
        })?;

        // Track if there are any bids
        any_bids = any_bids || bid_count > 0;

        // Get previous value if it exists
        let prev_value = sqlx::query_scalar::<_, rust_decimal::Decimal>(
            "SELECT value FROM round_space_results 
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
            .bind(previous_round.id)
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
        .bind(previous_round.end_at.to_sqlx())
        .bind(auction.id)
        .execute(&mut *tx)
        .await
        .with_context(|| {
            format!("failed to conclude auction {}", auction.id)
        })?;
    }

    tx.commit().await.context("failed to commit transaction")?;

    Ok(any_bids)
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
        "SELECT * FROM spaces WHERE site_id = $1 AND is_available = true",
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

/// For an in-progress auction, create the next auction round as needed.
#[tracing::instrument(skip(tx))]
pub async fn add_subsequent_rounds_for_auction(
    auction: &store::Auction,
    previous_round: &Option<store::AuctionRound>,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
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
            eligibility_threshold
        ) VALUES ($1, $2, $3, $4, $5)
        RETURNING *",
    )
    .bind(auction.id)
    .bind(round_num)
    .bind(start_time_ts.to_sqlx())
    .bind(zoned_end_time.timestamp().to_sqlx())
    .bind(eligibility_threshold)
    .fetch_one(&mut **tx)
    .await
    .context("inserting round into database")?;

    Ok(new_round.id)
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
