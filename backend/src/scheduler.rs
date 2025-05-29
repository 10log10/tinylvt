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
    let _ = start_auctions(pool).await;
    Ok(())
}

#[tracing::instrument(skip(pool), err(level = Level::ERROR))]
pub async fn start_auctions(pool: &PgPool) -> anyhow::Result<()> {
    // get all auctions where the start time has just passed, but the first
    // auction round is not yet created.
    let auctions = sqlx::query_as::<_, store::Auction>(
        "SELECT * FROM auctions
        WHERE start_at >= $1
        AND NOT EXISTS (
            SELECT 1 FROM auction_rounds
            WHERE auction_id = auctions.id
        )",
    )
    .bind(time::now().to_sqlx())
    .fetch_all(pool)
    .await?;

    for auction in auctions {
        let _ = span!(
            Level::INFO,
            "creating round 0",
            auction_id = auction.id.to_string()
        )
        .entered();

        let auction_params = sqlx::query_as::<_, store::AuctionParams>(
            "SELECT * FROM auction_params WHERE id = $1",
        )
        .bind(&auction.auction_params_id)
        .fetch_one(pool)
        .await?;
        let timezone = sqlx::query_as::<_, store::Site>(
            "SELECT * FROM sites where id = $1",
        )
        .bind(auction.site_id)
        .fetch_one(pool)
        .await?
        .timezone;

        let zoned_start_time = match auction
            .start_at
            .in_tz(&timezone)
            .context("computing round 0 start time; falling back to UTC")
        {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("{e:#}");
                auction.start_at.to_zoned(TimeZone::UTC)
            }
        };

        let zoned_end_time = match zoned_start_time
            .checked_add(auction_params.round_duration)
            .context("computing round 0 end time; skipping")
        {
            Ok(t) => t,
            Err(e) => {
                // only if the result would exceed the range of Zoned
                tracing::error!("{e:#}");
                continue;
            }
        };

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
        .bind(0)
        .bind(auction.start_at.to_sqlx())
        .bind(zoned_end_time.timestamp().to_sqlx())
        .execute(pool)
        .await
        .context("inserting round 0 into database")
        {
            tracing::error!("{e:#}");
        }
    }

    Ok(())
}
