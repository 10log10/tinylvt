use api::scheduler;
use api::time::TimeSource;
use jiff::Timestamp;
use jiff::{Span, Zoned};
use payloads::{
    ApiError, AuctionParamsError, EligibilityProgressionError, PermissionLevel,
    requests,
};
use test_helpers::{self, spawn_app};

use test_helpers::assert_api_error;

#[tokio::test]
async fn test_mock_time() -> anyhow::Result<()> {
    let initial_time = Timestamp::now();
    let time_source = TimeSource::new(initial_time);

    time_source.advance(Span::new().hours(1));
    assert_eq!(time_source.now(), initial_time + Span::new().hours(1));

    let new_time = initial_time + Span::new().hours(2);
    time_source.set(new_time);
    assert_eq!(time_source.now(), new_time);

    Ok(())
}

#[tokio::test]
async fn test_auction_crud() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let auction = app.create_test_auction(&site.site_id).await?;
    let retrieved = app.client.get_auction(&auction.auction_id).await?;
    assert_eq!(auction.auction_id, retrieved.auction_id);

    let auctions = app.client.list_auctions(&site.site_id).await?;
    assert_eq!(auctions.len(), 1);
    assert_eq!(auctions[0].auction_id, auction.auction_id);

    // Hard deletion is only allowed after cancellation
    assert_api_error(
        app.client.delete_auction(&auction.auction_id).await,
        ApiError::AuctionNotCanceled,
    );

    app.client.cancel_auction(&auction.auction_id).await?;
    let retrieved = app.client.get_auction(&auction.auction_id).await?;
    assert!(retrieved.was_canceled);
    assert_eq!(retrieved.end_at, Some(app.time_source.now()));

    app.client.delete_auction(&auction.auction_id).await?;
    let auctions = app.client.list_auctions(&site.site_id).await?;
    assert!(auctions.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_auction_unauthorized() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let auction = app.create_test_auction(&site.site_id).await?;

    // new user that's not part of the community
    app.client.logout().await?;
    let details = payloads::requests::CreateAccount {
        username: "charlie".into(),
        password: "charliepw".into(),
        email: "charlie@example.com".into(),
    };
    app.client.create_account(&details).await?;
    app.client
        .login(&test_helpers::to_login_credentials(&details))
        .await?;

    assert_api_error(
        app.client.get_auction(&auction.auction_id).await,
        ApiError::MemberNotFound,
    );
    assert_api_error(
        app.client.list_auctions(&site.site_id).await,
        ApiError::MemberNotFound,
    );
    assert_api_error(
        app.client.delete_auction(&auction.auction_id).await,
        ApiError::MemberNotFound,
    );
    assert_api_error(
        app.client
            .schedule_auction(&requests::ScheduleAuction {
                auction_id: auction.auction_id,
                start_at: None,
            })
            .await,
        ApiError::MemberNotFound,
    );
    assert_api_error(
        app.client.cancel_auction(&auction.auction_id).await,
        ApiError::MemberNotFound,
    );

    Ok(())
}

#[tokio::test]
async fn test_auction_round_creation() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Create an auction that starts now
    let start_time = app.time_source.now();
    let auction = app.create_test_auction(&site.site_id).await?;
    // Turn back the clock by 5 minutes
    app.time_source.set(start_time - Span::new().minutes(5));

    // No rounds should exist yet
    let rounds = app.client.list_auction_rounds(&auction.auction_id).await?;
    assert!(rounds.is_empty());

    // Advance time to auction start
    app.time_source.set(start_time);
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // Round 0 should now exist
    let rounds = app.client.list_auction_rounds(&auction.auction_id).await?;
    assert_eq!(rounds.len(), 1);
    let round = &rounds[0];
    assert_eq!(round.round_details.round_num, 0);
    assert_eq!(round.round_details.auction_id, auction.auction_id);
    assert_eq!(round.round_details.start_at, start_time);
    assert_eq!(
        round.round_details.end_at,
        start_time + Span::new().minutes(1)
    );
    assert_eq!(round.round_details.eligibility_threshold, 0.5);

    // Test get_auction_round
    let round_by_id = app.client.get_auction_round(&round.round_id).await?;
    assert_eq!(round_by_id.round_id, round.round_id);

    Ok(())
}

#[tokio::test]
async fn test_immediate_auction_round_creation() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Create an auction that starts now
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(start_time);
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Round 0 should be created immediately
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 1);
    let round = &rounds[0];
    assert_eq!(round.round_details.round_num, 0);
    assert_eq!(round.round_details.auction_id, auction_id);
    assert_eq!(round.round_details.start_at, start_time);
    assert_eq!(
        round.round_details.end_at,
        start_time + Span::new().minutes(1)
    );
    assert_eq!(round.round_details.eligibility_threshold, 0.5);

    Ok(())
}

#[tokio::test]
async fn test_create_auction_time_validation() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // A start time in the past is rejected
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at =
        Some(app.time_source.now() - Span::new().minutes(1));
    assert_api_error(
        app.client.create_auction(&auction_details).await,
        ApiError::AuctionStartInPast,
    );

    // Starting exactly at now is allowed (immediate start)
    auction_details.start_at = Some(app.time_source.now());
    app.client.create_auction(&auction_details).await?;

    // Possession start must be before possession end
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.possession_end_at = auction_details.possession_start_at;
    assert_api_error(
        app.client.create_auction(&auction_details).await,
        ApiError::InvalidPossessionPeriod,
    );

    Ok(())
}

#[tokio::test]
async fn test_create_auction_eligibility_validation() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // A threshold above 100% is rejected.
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details
        .auction_params
        .activity_rule_params
        .eligibility_progression = vec![(0, 1.5)];
    assert_api_error(
        app.client.create_auction(&auction_details).await,
        ApiError::InvalidAuctionParams(
            AuctionParamsError::EligibilityProgression(
                EligibilityProgressionError::ThresholdOutOfRange {
                    index: 0,
                    round: 0,
                },
            ),
        ),
    );

    // Round numbers must be strictly ascending (duplicates included), since
    // the scheduler binary-searches the progression.
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details
        .auction_params
        .activity_rule_params
        .eligibility_progression = vec![(5, 0.5), (3, 0.75)];
    assert_api_error(
        app.client.create_auction(&auction_details).await,
        ApiError::InvalidAuctionParams(
            AuctionParamsError::EligibilityProgression(
                EligibilityProgressionError::RoundsNotAscending { index: 1 },
            ),
        ),
    );

    // A round 0 breakpoint is valid: it sets eligibility going into round 1
    // without constraining round 0's own bids.
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details
        .auction_params
        .activity_rule_params
        .eligibility_progression = vec![(0, 0.5), (3, 0.75)];
    app.client.create_auction(&auction_details).await?;

    Ok(())
}

#[tokio::test]
async fn test_unscheduled_auction_ignored_by_scheduler() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = None;
    let auction_id = app.client.create_auction(&auction_details).await?;

    let retrieved = app.client.get_auction(&auction_id).await?;
    assert_eq!(retrieved.auction_details.start_at, None);

    // The scheduler should never pick up an unscheduled auction, no matter
    // how much time passes.
    app.time_source.advance(Span::new().hours(24));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert!(rounds.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_schedule_auction() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = None;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // bob is a plain member, not coleader+
    app.login_bob().await?;
    assert_api_error(
        app.client
            .schedule_auction(&requests::ScheduleAuction {
                auction_id,
                start_at: Some(app.time_source.now() + Span::new().hours(1)),
            })
            .await,
        ApiError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        },
    );
    app.login_alice().await?;

    // Schedule a future start time
    let scheduled = app.time_source.now() + Span::new().hours(1);
    app.client
        .schedule_auction(&requests::ScheduleAuction {
            auction_id,
            start_at: Some(scheduled),
        })
        .await?;
    let retrieved = app.client.get_auction(&auction_id).await?;
    assert_eq!(retrieved.auction_details.start_at, Some(scheduled));

    // Reschedule
    let rescheduled = app.time_source.now() + Span::new().hours(2);
    app.client
        .schedule_auction(&requests::ScheduleAuction {
            auction_id,
            start_at: Some(rescheduled),
        })
        .await?;
    let retrieved = app.client.get_auction(&auction_id).await?;
    assert_eq!(retrieved.auction_details.start_at, Some(rescheduled));

    // Clear the schedule
    app.client
        .schedule_auction(&requests::ScheduleAuction {
            auction_id,
            start_at: None,
        })
        .await?;
    let retrieved = app.client.get_auction(&auction_id).await?;
    assert_eq!(retrieved.auction_details.start_at, None);

    // A start time in the past is rejected
    assert_api_error(
        app.client
            .schedule_auction(&requests::ScheduleAuction {
                auction_id,
                start_at: Some(app.time_source.now() - Span::new().minutes(1)),
            })
            .await,
        ApiError::AuctionStartNotInFuture,
    );

    // Once the scheduled start passes, the auction has started and can no
    // longer be rescheduled. This is also the "start now" UI flow, which
    // schedules a start a few seconds in the future.
    let soon = app.time_source.now() + Span::new().seconds(15);
    app.client
        .schedule_auction(&requests::ScheduleAuction {
            auction_id,
            start_at: Some(soon),
        })
        .await?;
    app.time_source.set(soon);
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 1);
    assert_eq!(rounds[0].round_details.round_num, 0);
    assert_eq!(rounds[0].round_details.start_at, soon);

    assert_api_error(
        app.client
            .schedule_auction(&requests::ScheduleAuction {
                auction_id,
                start_at: Some(app.time_source.now() + Span::new().hours(1)),
            })
            .await,
        ApiError::AuctionAlreadyStarted,
    );

    Ok(())
}

#[tokio::test]
async fn test_cancel_before_start() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let start_time = app.time_source.now() + Span::new().hours(1);
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(start_time);
    let auction_id = app.client.create_auction(&auction_details).await?;

    // bob is a plain member, not coleader+
    app.login_bob().await?;
    assert_api_error(
        app.client.cancel_auction(&auction_id).await,
        ApiError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        },
    );

    app.login_alice().await?;
    let cancel_time = app.time_source.now();
    app.client.cancel_auction(&auction_id).await?;
    let retrieved = app.client.get_auction(&auction_id).await?;
    assert!(retrieved.was_canceled);
    assert_eq!(retrieved.end_at, Some(cancel_time));

    // The scheduler never starts a canceled auction, even past its
    // scheduled start time: jump the clock to one minute after the
    // scheduled start and tick.
    app.time_source.set(start_time + Span::new().minutes(1));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert!(rounds.is_empty());

    // Canceling again fails
    assert_api_error(
        app.client.cancel_auction(&auction_id).await,
        ApiError::AuctionAlreadyEnded,
    );

    Ok(())
}

#[tokio::test]
async fn test_cancel_mid_auction_blocks_settlement() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let space = app.create_test_space(&site.site_id).await?;

    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(start_time);
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Round 0 with a bid, then round 1 created from the standing bid
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space.space_id, &rounds[0].round_id)
        .await?;
    app.time_source
        .set(rounds[0].round_details.end_at + Span::new().seconds(1));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 2);

    // Cancel mid-round
    let cancel_time = app.time_source.now();
    app.client.cancel_auction(&auction_id).await?;
    let retrieved = app.client.get_auction(&auction_id).await?;
    assert!(retrieved.was_canceled);
    assert_eq!(retrieved.end_at, Some(cancel_time));

    // Without cancellation, the bidless round 1 ending would conclude the
    // auction and create a settlement entry. After cancellation the
    // scheduler ignores the auction entirely: no new rounds, no settlement.
    app.time_source
        .set(rounds[1].round_details.end_at + Span::new().seconds(1));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 2);

    let settlement_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM journal_entries
        WHERE entry_type = 'auction_settlement' AND auction_id = $1",
    )
    .bind(auction_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(settlement_count, 0);

    // Despite round/bid history, the canceled auction can be hard-deleted
    app.client.delete_auction(&auction_id).await?;
    assert_api_error(
        app.client.get_auction(&auction_id).await,
        ApiError::AuctionNotFound,
    );

    Ok(())
}

#[tokio::test]
async fn test_auction_rounds_dst() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let mut site = app.create_test_site(&community_id).await?;

    // Set timezone to Los Angeles
    site.site_details.timezone = Some("America/Los_Angeles".to_string());
    app.client
        .update_site(&requests::UpdateSite {
            site_id: site.site_id,
            site_details: site.site_details,
        })
        .await?;

    // Create an auction starting March 8, 2024 at noon PST
    // This will span the DST transition on March 10 at 2am
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);

    // Set start time just before DST transition (March 10, 2024 1:59 AM PST)
    let start_time: Zoned =
        "2024-03-10T01:59:00-08:00[America/Los_Angeles]".parse()?;
    let start_time = start_time.timestamp();
    app.time_source.set(start_time);
    auction_details.start_at = Some(start_time);

    // Set round duration to one day
    auction_details.auction_params.round_duration = jiff::Span::new().days(1);

    // Create the auction
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Start the auction to create initial round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // Verify initial round
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 1);
    let round0 = &rounds[0];
    assert_eq!(round0.round_details.round_num, 0);

    let expected_round_end_time: Zoned =
        "2024-03-11T01:59:00-07:00[America/Los_Angeles]".parse()?;

    assert_eq!(
        round0.round_details.end_at,
        expected_round_end_time.timestamp()
    );
    assert_eq!(
        round0.round_details.end_at.in_tz("America/Los_Angeles")?,
        expected_round_end_time
    );

    Ok(())
}

#[tokio::test]
async fn test_bid_crud() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let space = app.create_test_space(&site.site_id).await?;

    // Create an auction that starts now
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(start_time);
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Create initial round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 1);
    let round = &rounds[0];

    // Initially no bids should exist
    let bids = app.client.list_bids(&round.round_id).await?;
    assert!(bids.is_empty());

    // Create a bid
    app.client
        .create_bid(&space.space_id, &round.round_id)
        .await?;

    // Verify bid exists via get
    let bid = app.client.get_bid(&space.space_id, &round.round_id).await?;
    assert_eq!(bid.space_id, space.space_id);
    assert_eq!(bid.round_id, round.round_id);

    // Verify bid appears in list
    let bids = app.client.list_bids(&round.round_id).await?;
    assert_eq!(bids.len(), 1);

    assert_eq!(bids[0].space_id, space.space_id);
    assert_eq!(bids[0].round_id, round.round_id);

    // Delete the bid
    app.client
        .delete_bid(&space.space_id, &round.round_id)
        .await?;

    // Verify bid no longer exists
    let bids = app.client.list_bids(&round.round_id).await?;
    assert!(bids.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_bid_after_round_end() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let space = app.create_test_space(&site.site_id).await?;

    // Create an auction that starts now
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(start_time);
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Create initial round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 1);
    let round = &rounds[0];

    // Create a bid
    app.client
        .create_bid(&space.space_id, &round.round_id)
        .await?;

    // Advance time past round end
    app.time_source
        .set(round.round_details.end_at + Span::new().seconds(1));

    // Attempt to create/delete bids should fail
    assert!(
        app.client
            .create_bid(&space.space_id, &round.round_id)
            .await
            .is_err()
    );
    assert!(
        app.client
            .delete_bid(&space.space_id, &round.round_id)
            .await
            .is_err()
    );

    Ok(())
}

#[tokio::test]
async fn test_continued_bidding() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let space = app.create_test_space(&site.site_id).await?;
    // a dummy space so bob can get eligibility in the first round, while not
    // bidding for space a
    let space_b_id = app
        .client
        .create_space(&test_helpers::space_details_b(site.site_id))
        .await?;

    // Create an auction that starts now
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(start_time);
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Create initial round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let mut rounds = app.client.list_auction_rounds(&auction_id).await?;
    let mut round = &rounds[0];

    let max_rounds = 5;

    // Ensure bob has eligibility in the first round
    app.login_bob().await?;
    app.client.create_bid(&space_b_id, &round.round_id).await?;

    for i in 0..max_rounds {
        if i % 2 == 0 {
            // Create a bid by Alice
            app.login_alice().await?;
            app.client
                .create_bid(&space.space_id, &round.round_id)
                .await?;
        } else {
            // Create a bid by Bob
            app.login_bob().await?;
            app.client
                .create_bid(&space.space_id, &round.round_id)
                .await?;
        }

        // Advance time past round end
        app.time_source
            .set(round.round_details.end_at + Span::new().seconds(1));

        // View results and create the next round
        scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

        // View the result of the last round
        let round_space_result = app
            .client
            .get_round_space_result(&space.space_id, &round.round_id)
            .await?;
        dbg!(&round_space_result);
        assert_eq!(round_space_result.value, rust_decimal::Decimal::from(i));

        // Get the next round
        rounds = app.client.list_auction_rounds(&auction_id).await?;
        round = &rounds[i + 1];

        dbg!(i);
    }

    // now only bob makes another bid, and should win the space for 5
    app.login_bob().await?;
    app.client
        .create_bid(&space.space_id, &round.round_id)
        .await?;
    // Advance time past round end
    app.time_source
        .set(round.round_details.end_at + Span::new().seconds(1));
    // View results and conclude the auction
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // View the result of the last round
    let round_space_result = app
        .client
        .get_round_space_result(&space.space_id, &round.round_id)
        .await?;
    dbg!(&round_space_result);
    assert_eq!(
        round_space_result.value,
        rust_decimal::Decimal::from(max_rounds)
    );
    assert_eq!(round_space_result.space_id, space.space_id);
    assert_eq!(round_space_result.round_id, round.round_id);
    assert_eq!(round_space_result.winner.username, "bob");
    assert_eq!(
        round_space_result.value,
        rust_decimal::Decimal::from(max_rounds)
    );

    // Verify conclusion of the auction after bidding stops

    rounds = app.client.list_auction_rounds(&auction_id).await?;
    round = &rounds[6];
    app.time_source
        .set(round.round_details.end_at + Span::new().seconds(1));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    let auction = app.client.get_auction(&auction_id).await?;
    assert_eq!(auction.end_at, Some(round.round_details.end_at));

    Ok(())
}

#[tokio::test]
async fn test_bid_eligibility() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Create two spaces with different eligibility points
    let space_a = app.create_test_space(&site.site_id).await?; // 10 points
    let space_b = app
        .client
        .create_space(&payloads::Space {
            site_id: site.site_id,
            name: "test space b".into(),
            description: None,
            eligibility_points: 15.0, // Higher points than space_a
            is_available: true,
            site_image_id: None,
            reserve_price: payloads::ReservePrice(rust_decimal::Decimal::ZERO),
        })
        .await?;
    let space_b = app.client.get_space(&space_b).await?;

    // Create an auction that starts now
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(start_time);
    let auction_id = app.client.create_auction(&auction_details).await?;

    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // Round 0 - no eligibility constraints
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_0 = &rounds[0];

    // Alice bids only for space_a, Bob bids only for space_b
    app.login_alice().await?;
    app.client
        .create_bid(&space_a.space_id, &round_0.round_id)
        .await?; // Alice bids on space_a

    app.login_bob().await?;
    app.client
        .create_bid(&space_b.space_id, &round_0.round_id)
        .await?; // Bob bids on space_b

    // Advance time to end round 0
    app.time_source
        .advance(auction_details.auction_params.round_duration);
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // Round 1 - eligibility is based on round 0 results
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_1 = &rounds[1];

    // Get round 0 results - Alice should have won space_a and Bob should have
    // won space_b
    let results = app
        .client
        .list_round_space_results_for_round(&round_0.round_id)
        .await?;
    let space_a_result = results
        .iter()
        .find(|r| r.space_id == space_a.space_id)
        .unwrap();
    let space_b_result = results
        .iter()
        .find(|r| r.space_id == space_b.space_id)
        .unwrap();

    assert_eq!(space_a_result.winner.username, "alice");
    assert_eq!(space_b_result.winner.username, "bob");

    // Alice cannot bid on space_a in round 1 since she's already winning it
    app.login_alice().await?;
    let result = app
        .client
        .create_bid(&space_a.space_id, &round_1.round_id)
        .await;
    assert_api_error(result, ApiError::AlreadyWinningSpace);

    // But she can bid on space_b (though it will fail due to insufficient
    // eligibility: her standing 10-point win on space_a plus the 15-point
    // space_b exceeds her 10 / 0.5 = 20 point budget)
    let result = app
        .client
        .create_bid(&space_b.space_id, &round_1.round_id)
        .await;
    assert_api_error(
        result,
        ApiError::ExceedsEligibility {
            available: 20.0,
            required: 25.0,
        },
    );

    // Bob cannot bid on space_b in round 1 since he's already winning it
    app.login_bob().await?;
    let result = app
        .client
        .create_bid(&space_b.space_id, &round_1.round_id)
        .await;
    assert_api_error(result, ApiError::AlreadyWinningSpace);

    // But he can bid on space_a since he has enough eligibility (15 points * 2
    // = 30 points > 25 points needed)
    app.client
        .create_bid(&space_a.space_id, &round_1.round_id)
        .await?;

    Ok(())
}

// A 0% eligibility progression from the outset should effectively turn off
// the eligibility constraint: bids that would otherwise exceed a nonzero
// threshold are all accepted, and no eligibility rows are produced (so no
// non-finite values reach the database or the API).
#[tokio::test]
async fn test_eligibility_disabled_at_zero() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // space_a is 10 points, space_b is 15 points. Under the default 50%
    // threshold, a 10-point bidder could not also take the 15-point space
    // (10 + 15 = 25 > 10 / 0.5 = 20). With a 0% threshold this is allowed.
    let space_a = app.create_test_space(&site.site_id).await?;
    let space_b = app
        .client
        .create_space(&payloads::Space {
            site_id: site.site_id,
            name: "test space b".into(),
            description: None,
            eligibility_points: 15.0,
            is_available: true,
            site_image_id: None,
            reserve_price: payloads::ReservePrice(rust_decimal::Decimal::ZERO),
        })
        .await?;
    let space_b = app.client.get_space(&space_b).await?;

    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(start_time);
    // 0% eligibility required from the outset.
    auction_details
        .auction_params
        .activity_rule_params
        .eligibility_progression = vec![(0, 0.0)];
    let auction_id = app.client.create_auction(&auction_details).await?;

    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_0 = &rounds[0];

    // Alice bids only on the 10-point space in round 0.
    app.login_alice().await?;
    app.client
        .create_bid(&space_a.space_id, &round_0.round_id)
        .await?;

    app.time_source
        .advance(auction_details.auction_params.round_duration);
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_1 = &rounds[1];

    // With eligibility disabled, Alice can take the 15-point space in round
    // 1 even though her round-0 activity was only 10 points. This bid would
    // be rejected under a nonzero threshold.
    app.login_alice().await?;
    app.client
        .create_bid(&space_b.space_id, &round_1.round_id)
        .await?;

    // The prior round's threshold was 0%, so round 1 is unconstrained. The
    // API reports this as Unlimited (no row was written, so no division
    // produced a non-finite value to store).
    let eligibility = app.client.get_eligibility(&round_1.round_id).await?;
    assert_eq!(
        eligibility,
        payloads::Eligibility::Unlimited,
        "expected Unlimited eligibility when prior threshold is 0%"
    );

    Ok(())
}

// When the prior round imposed a nonzero threshold, a user who sat out that
// round has no eligibility row, so their eligibility is a finite 0 (a
// Finite(0.0) budget): they cannot bid on a positive-point space, but they
// can still bid on a zero-point space (bidding it adds nothing to their
// activity).
#[tokio::test]
async fn test_eligibility_required_when_nonzero() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let space = app.create_test_space(&site.site_id).await?; // 10 points
    // A zero-point space that a zero-budget (Finite(0.0)) user is still
    // allowed to bid.
    let free_space = app
        .client
        .create_space(&payloads::Space {
            site_id: site.site_id,
            name: "free space".into(),
            description: None,
            eligibility_points: 0.0,
            is_available: true,
            site_image_id: None,
            reserve_price: payloads::ReservePrice(rust_decimal::Decimal::ZERO),
        })
        .await?;
    let free_space = app.client.get_space(&free_space).await?;

    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(start_time);
    // Nonzero threshold from the outset (the default already is, but make it
    // explicit for this test's intent).
    auction_details
        .auction_params
        .activity_rule_params
        .eligibility_progression = vec![(0, 0.5)];
    let auction_id = app.client.create_auction(&auction_details).await?;

    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_0 = &rounds[0];

    // Alice bids in round 0 so the round concludes with activity; Bob sits
    // out entirely.
    app.login_alice().await?;
    app.client
        .create_bid(&space.space_id, &round_0.round_id)
        .await?;

    app.time_source
        .advance(auction_details.auction_params.round_duration);
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_1 = &rounds[1];

    // Bob never participated in round 0, so he has no eligibility row for
    // round 1. Because the prior threshold was nonzero, his eligibility is a
    // finite 0 (not Unlimited).
    app.login_bob().await?;
    assert_eq!(
        app.client.get_eligibility(&round_1.round_id).await?,
        payloads::Eligibility::Finite(0.0),
        "a sit-out bidder should have a finite 0 eligibility, not Unlimited"
    );

    // He cannot bid on the 10-point space: with eligibility 0, the 10 points
    // exceed it.
    assert_api_error(
        app.client
            .create_bid(&space.space_id, &round_1.round_id)
            .await,
        ApiError::ExceedsEligibility {
            available: 0.0,
            required: 10.0,
        },
    );

    // But he can bid on the zero-point space, since it adds nothing to his
    // activity (0 > 0 is false).
    app.client
        .create_bid(&free_space.space_id, &round_1.round_id)
        .await?;

    Ok(())
}

// A progression that starts at 0% and switches to a nonzero threshold partway
// through should leave early rounds unconstrained, then activate the
// constraint once the prior round's threshold becomes nonzero.
#[tokio::test]
async fn test_eligibility_progression_activates_midway() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let space_a = app.create_test_space(&site.site_id).await?; // 10 points
    let space_b = app
        .client
        .create_space(&payloads::Space {
            site_id: site.site_id,
            name: "test space b".into(),
            description: None,
            eligibility_points: 15.0,
            is_available: true,
            site_image_id: None,
            reserve_price: payloads::ReservePrice(rust_decimal::Decimal::ZERO),
        })
        .await?;
    let space_b = app.client.get_space(&space_b).await?;

    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(start_time);
    // Rounds 0 and 1 are unconstrained (threshold 0.0); round 2 onward uses a
    // 50% threshold. Since a round's threshold governs the *following*
    // round's bids, the constraint first applies to round-3 bids.
    auction_details
        .auction_params
        .activity_rule_params
        .eligibility_progression = vec![(0, 0.0), (2, 0.5)];
    let auction_id = app.client.create_auction(&auction_details).await?;

    let round_duration = auction_details.auction_params.round_duration;
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // Alice and Bob alternate outbidding each other on the 10-point space_a so
    // that every round has a new bid, keeping the auction from concluding for
    // lack of demand. Whoever isn't the standing winner bids each round.

    // Round 0: Alice bids and becomes the standing winner.
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.login_alice().await?;
    app.client
        .create_bid(&space_a.space_id, &rounds[0].round_id)
        .await?;

    app.time_source.advance(round_duration);
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // Round 1 (prior threshold 0.0 -> unconstrained): Unlimited eligibility,
    // and Bob can outbid freely.
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(
        app.client.get_eligibility(&rounds[1].round_id).await?,
        payloads::Eligibility::Unlimited,
        "round 1 should be Unlimited (prior threshold 0.0)"
    );
    app.login_bob().await?;
    app.client
        .create_bid(&space_a.space_id, &rounds[1].round_id)
        .await?;

    app.time_source.advance(round_duration);
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // Round 2 (prior threshold 0.0 -> still unconstrained): still Unlimited.
    // Alice retakes the lead.
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(
        app.client.get_eligibility(&rounds[2].round_id).await?,
        payloads::Eligibility::Unlimited,
        "round 2 should still be Unlimited (prior threshold 0.0)"
    );
    app.login_alice().await?;
    app.client
        .create_bid(&space_a.space_id, &rounds[2].round_id)
        .await?;

    app.time_source.advance(round_duration);
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // Round 3: round 2's threshold was 50%, so the constraint now applies.
    // Both Alice and Bob have 10 points of activity going into round 2 (Alice
    // bid space_a in round 2; Bob holds the standing win from round 1), so
    // each gets an eligibility of 10 / 0.5 = 20 for round 3. This also
    // confirms the value is finite (not the inf/NaN that x/0.0 would produce).
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_3 = &rounds[3];

    app.login_alice().await?;
    assert_eq!(
        app.client.get_eligibility(&round_3.round_id).await?,
        payloads::Eligibility::Finite(20.0),
        "Alice's round-3 eligibility should be 10 / 0.5 = 20"
    );
    app.login_bob().await?;
    assert_eq!(
        app.client.get_eligibility(&round_3.round_id).await?,
        payloads::Eligibility::Finite(20.0),
        "Bob's round-3 eligibility should be 10 / 0.5 = 20"
    );

    // Alice is the standing high bidder of space_a from round 2, so that
    // 10-point standing win already counts toward her activity. Adding the
    // 15-point space_b would total 25 > 20, so she cannot bid on space_b.
    app.login_alice().await?;
    assert_api_error(
        app.client
            .create_bid(&space_b.space_id, &round_3.round_id)
            .await,
        ApiError::ExceedsEligibility {
            available: 20.0,
            required: 25.0,
        },
    );

    // Bob holds no standing win in round 3, so only his round-3 bids count. He
    // can take either space alone (10 <= 20, 15 <= 20), but not both: after
    // bidding space_a, adding space_b totals 10 + 15 = 25 > 20.
    app.login_bob().await?;
    app.client
        .create_bid(&space_a.space_id, &round_3.round_id)
        .await?;
    assert_api_error(
        app.client
            .create_bid(&space_b.space_id, &round_3.round_id)
            .await,
        ApiError::ExceedsEligibility {
            available: 20.0,
            required: 25.0,
        },
    );

    Ok(())
}

#[tokio::test]
async fn test_eligibility_routes() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let space = app.create_test_space(&site.site_id).await?;

    // Create an auction that starts now
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(start_time);
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Create initial round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 1);
    let round0 = &rounds[0];

    // Place a bid in round 0 to establish some eligibility
    app.client
        .create_bid(&space.space_id, &round0.round_id)
        .await?;

    // Advance time past round 0
    app.time_source
        .set(round0.round_details.end_at + Span::new().seconds(1));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // Get rounds again - should now have round 1
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 2);
    let round1 = &rounds[1];

    // Test get_eligibility for round 1. The default progression's round-0
    // threshold is 50%, and the user bid a 10-point space, so their round-1
    // eligibility is Finite(10 / 0.5 = 20).
    let eligibility = app.client.get_eligibility(&round1.round_id).await?;
    assert!(
        matches!(
            eligibility,
            payloads::Eligibility::Finite(e) if e > 0.0
        ),
        "expected a positive Finite eligibility for round 1, got \
         {eligibility:?}"
    );

    // Test list_eligibility for all rounds. It aligns 1:1 with the rounds:
    // index 0 is round 0 (always Unlimited), index 1 is round 1.
    let eligibilities = app.client.list_eligibility(&auction_id).await?;
    assert_eq!(
        eligibilities.len(),
        2,
        "Expected an eligibility entry per round"
    );
    assert_eq!(
        eligibilities[0],
        payloads::Eligibility::Unlimited,
        "Round 0 should be Unlimited"
    );
    assert_eq!(
        eligibilities[1], eligibility,
        "Expected matching eligibility for round 1"
    );

    // Test unauthorized access
    app.client.logout().await?;
    let details = payloads::requests::CreateAccount {
        username: "charlie".into(),
        password: "charliepw".into(),
        email: "charlie@example.com".into(),
    };
    app.client.create_account(&details).await?;
    app.client
        .login(&test_helpers::to_login_credentials(&details))
        .await?;

    assert_api_error(
        app.client.get_eligibility(&round1.round_id).await,
        ApiError::MemberNotFound,
    );
    assert_api_error(
        app.client.list_eligibility(&auction_id).await,
        ApiError::MemberNotFound,
    );

    Ok(())
}

#[tokio::test]
async fn test_bid_unavailable_space() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Create a space but mark it as unavailable
    let mut space_details = test_helpers::space_details_a(site.site_id);
    space_details.is_available = false;
    let space_id = app.client.create_space(&space_details).await?;

    // Create an auction that starts now
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(start_time);
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Create initial round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 1);
    let round = &rounds[0];

    // Try to create a bid on the unavailable space - should fail
    let result = app.client.create_bid(&space_id, &round.round_id).await;

    // The error should indicate the space is not available
    assert_api_error(result, ApiError::SpaceNotAvailable);

    Ok(())
}
