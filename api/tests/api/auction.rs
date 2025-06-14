use api::scheduler;
use api::time::TimeSource;
use jiff::Timestamp;
use jiff::{Span, Zoned};
use payloads::requests;
use reqwest::StatusCode;
use test_helpers::{self, spawn_app};

use test_helpers::assert_status_code;

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

    assert_status_code(
        app.client.get_auction(&auction.auction_id).await,
        StatusCode::UNAUTHORIZED,
    );
    assert_status_code(
        app.client.list_auctions(&site.site_id).await,
        StatusCode::UNAUTHORIZED,
    );
    assert_status_code(
        app.client.delete_auction(&auction.auction_id).await,
        StatusCode::UNAUTHORIZED,
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
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

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
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Round 0 should be created immediately
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;
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
async fn test_auction_rounds_dst() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let mut site = app.create_test_site(&community_id).await?;

    // Set timezone to Los Angeles
    site.site_details.timezone = "America/Los_Angeles".to_string();
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
    auction_details.start_at = start_time;

    // Set round duration to one day
    auction_details.auction_params.round_duration = jiff::Span::new().days(1);

    // Create the auction
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Start the auction to create initial round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

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
async fn test_round_space_result_creation() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Create a space in the site
    let space = app.create_test_space(&site.site_id).await?;

    // Create an auction that starts now
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Create initial round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 1);
    let round = &rounds[0];

    let round_space_results = app
        .client
        .list_round_space_results_for_round(&round.round_id)
        .await?;
    assert_eq!(round_space_results.len(), 0);

    // Advance time past the round end
    app.time_source
        .set(round.round_details.end_at + Span::new().seconds(1));

    // Update space rounds - this should create entries with zero values since there are no bids
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

    // Check space round was created
    let round_space_results = app
        .client
        .list_round_space_results_for_round(&round.round_id)
        .await?;
    assert_eq!(round_space_results.len(), 1);
    let round_space_result = &round_space_results[0];

    // Verify space round properties
    assert_eq!(round_space_result.space_id, space.space_id);
    assert_eq!(round_space_result.round_id, round.round_id);
    assert_eq!(round_space_result.winning_username, None);
    assert_eq!(round_space_result.value, rust_decimal::Decimal::ZERO);

    // Verify conclusion of the auction
    let auction = app.client.get_auction(&auction_id).await?;
    assert_eq!(auction.end_at, Some(round.round_details.end_at));

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
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Create initial round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 1);
    let round = &rounds[0];

    // Initially no bids should exist
    let bids = app
        .client
        .list_bids(&space.space_id, &round.round_id)
        .await?;
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
    let bids = app
        .client
        .list_bids(&space.space_id, &round.round_id)
        .await?;
    assert_eq!(bids.len(), 1);

    assert_eq!(bids[0].space_id, space.space_id);
    assert_eq!(bids[0].round_id, round.round_id);

    // Delete the bid
    app.client
        .delete_bid(&space.space_id, &round.round_id)
        .await?;

    // Verify bid no longer exists
    let bids = app
        .client
        .list_bids(&space.space_id, &round.round_id)
        .await?;
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
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Create initial round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;
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
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Create initial round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;
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
        scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

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
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

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
    assert_eq!(
        round_space_result,
        payloads::RoundSpaceResult {
            space_id: space.space_id,
            round_id: round.round_id,
            winning_username: Some("bob".into()),
            value: rust_decimal::Decimal::from(max_rounds),
        }
    );

    // Verify conclusion of the auction after bidding stops

    rounds = app.client.list_auction_rounds(&auction_id).await?;
    round = &rounds[6];
    app.time_source
        .set(round.round_details.end_at + Span::new().seconds(1));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

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
        })
        .await?;
    let space_b = app.client.get_space(&space_b).await?;

    // Create an auction that starts now
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

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
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

    // Round 1 - eligibility is based on round 0 results
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_1 = &rounds[1];

    // Get round 0 results - Alice should have won space_a and Bob should have won space_b
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

    assert_eq!(space_a_result.winning_username.as_deref(), Some("alice"));
    assert_eq!(space_b_result.winning_username.as_deref(), Some("bob"));

    // Alice cannot bid on space_a in round 1 since she's already winning it
    app.login_alice().await?;
    let result = app
        .client
        .create_bid(&space_a.space_id, &round_1.round_id)
        .await;
    assert!(matches!(result, Err(payloads::ClientError::APIError(..))));

    // But she can bid on space_b (though it will fail due to insufficient eligibility)
    let result = app
        .client
        .create_bid(&space_b.space_id, &round_1.round_id)
        .await;
    assert!(matches!(result, Err(payloads::ClientError::APIError(..))));

    // Bob cannot bid on space_b in round 1 since he's already winning it
    app.login_bob().await?;
    let result = app
        .client
        .create_bid(&space_b.space_id, &round_1.round_id)
        .await;
    assert!(matches!(result, Err(payloads::ClientError::APIError(..))));

    // But he can bid on space_a since he has enough eligibility (15 points * 2 = 30 points > 25 points needed)
    app.client
        .create_bid(&space_a.space_id, &round_1.round_id)
        .await?;

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
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Create initial round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;
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
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

    // Get rounds again - should now have round 1
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 2);
    let round1 = &rounds[1];

    // Test get_eligibility for round 1
    let eligibility = app.client.get_eligibility(&round1.round_id).await?;
    assert!(
        eligibility > 0.0,
        "Expected non-zero eligibility for round 1"
    );

    // Test list_eligibility for all rounds
    let eligibilities = app.client.list_eligibility(&auction_id).await?;
    assert_eq!(
        eligibilities.len(),
        1,
        "Expected eligibility values for both rounds"
    );
    assert_eq!(
        eligibilities[0], eligibility,
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

    assert_status_code(
        app.client.get_eligibility(&round1.round_id).await,
        reqwest::StatusCode::UNAUTHORIZED,
    );
    assert_status_code(
        app.client.list_eligibility(&auction_id).await,
        reqwest::StatusCode::UNAUTHORIZED,
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
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Create initial round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 1);
    let round = &rounds[0];

    // Try to create a bid on the unavailable space - should fail
    let result = app.client.create_bid(&space_id, &round.round_id).await;

    // The error message should indicate the space is not available
    if let Err(payloads::ClientError::APIError(code, message)) = result {
        // Verify we got a 400 Bad Request
        assert_eq!(code, StatusCode::BAD_REQUEST);
        assert!(
            message.contains("Space is not available for bidding"),
            "Unexpected error message: {}",
            message
        );
    } else {
        panic!("Expected APIError but got {:?}", result);
    }

    Ok(())
}
