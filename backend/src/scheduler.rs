//! Top-level orchestration of time-based triggers and scheduling.
//!
//! E.g. on the proxy bidding lead time is reached and auto scheduling is
//! enabled, the scheduler creates the auction row so proxy bids can start to be
//! associated with it. Other scheduling tasks include, starting the auction,
//! computing auction rounds, and updating members' is_active state based on the
//! membership schedule.

use anyhow::Context;
use jiff::tz::TimeZone;
use jiff_sqlx::ToSqlx;
use sqlx::PgPool;
use tracing::{Level, span};

use crate::{store, time};

/// Update state once right now.
#[tracing::instrument(skip(pool), err(level = Level::ERROR))]
pub async fn schedule_tick(pool: &PgPool) -> anyhow::Result<()> {
    // tracing will log the errors if the occur
    let _ = store::update_is_active_from_schedule(pool).await;
    // TODO: update space rounds
    // let _ = update_space_rounds(pool).await;
    let _ = add_subsequent_rounds(pool).await;
    // TODO: conlclude auctions
    Ok(())
}

#[tracing::instrument(skip(pool), err(level = Level::ERROR))]
pub async fn add_subsequent_rounds(pool: &PgPool) -> anyhow::Result<()> {
    // Get all auctions where the start time is in the past, the auction hasn't
    // yet concluded, and there is not an ongoing auction round the an end time
    // in the future.
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
    .bind(time::now().to_sqlx())
    .fetch_all(pool)
    .await?;

    for auction in auctions {
        // attach the auction id to any tracing events using a span
        let _ = span!(
            Level::INFO,
            "creating next round",
            auction_id = auction.id.to_string()
        )
        .entered();

        let auction_params = match sqlx::query_as::<_, store::AuctionParams>(
            "SELECT * FROM auction_params WHERE id = $1",
        )
        .bind(&auction.auction_params_id)
        .fetch_one(pool)
        .await
        .context("getting auction params; skipping")
        {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("{e:#}");
                continue;
            }
        };

        let timezone = match sqlx::query_as::<_, store::Site>(
            "SELECT * FROM sites where id = $1",
        )
        .bind(auction.site_id)
        .fetch_one(pool)
        .await
        .context("getting site; skipping")
        {
            Ok(s) => s.timezone,
            Err(e) => {
                tracing::error!("{e:#}");
                continue;
            }
        };

        let last_auction_round = match sqlx::query_as::<_, store::AuctionRound>(
            "SELECT * FROM auction_rounds
            WHERE auction_id = $1
            ORDER BY round_num ASC LIMIT 1",
        )
        .bind(auction.id)
        .fetch_optional(pool)
        .await
        .context("getting last auction round; skipping")
        {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("{e:#}");
                continue;
            }
        };

        let start_time_ts = last_auction_round
            .as_ref()
            .map(|r| r.end_at)
            .unwrap_or(auction.start_at);

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

        let zoned_end_time = match zoned_start_time
            .checked_add(auction_params.round_duration)
            .context("computing round end time; skipping")
        {
            Ok(t) => t,
            Err(e) => {
                // only if the result would exceed the range of Zoned
                tracing::error!("{e:#}");
                continue;
            }
        };

        let round_num: i32 =
            last_auction_round.map(|r| r.round_num + 1).unwrap_or(0);

        let eligibility_threshold = get_eligibility_for_round_num(
            round_num,
            &auction_params.activity_rule_params.eligibility_progression,
        );

        if let Err(e) = sqlx::query(
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
        .context("inserting round into database")
        {
            tracing::error!("{e:#}");
        }
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
