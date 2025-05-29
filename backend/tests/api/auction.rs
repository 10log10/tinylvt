use crate::helpers::{self, spawn_app};
use backend::{scheduler, time};
use jiff::{Span, Zoned};
use payloads::requests;

#[tokio::test]
async fn test_time_mock() -> anyhow::Result<()> {
    let initial_time = time::now();

    time::advance_mock_time(Span::new().hours(1));
    assert_eq!(time::now(), initial_time + Span::new().hours(1));

    let new_time = initial_time + Span::new().hours(7);
    time::set_mock_time(new_time);
    assert_eq!(time::now(), new_time);

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
    use crate::helpers::assert_status_code;
    use reqwest::StatusCode;

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
    app.client.login(&details).await?;

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
    let start_time = time::now();
    let auction = app.create_test_auction(&site.site_id).await?;
    // Turn back the clock by 5 minutes
    time::set_mock_time(start_time - Span::new().minutes(5));

    // No rounds should exist yet
    let rounds = app.client.list_auction_rounds(&auction.auction_id).await?;
    assert!(rounds.is_empty());

    // Advance time to auction start
    time::set_mock_time(start_time);
    scheduler::schedule_tick(&app.db_pool).await?;

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
    let start_time = time::now();
    let mut auction_details = crate::helpers::auction_details_a(site.site_id);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Round 0 should be created immediately
    scheduler::schedule_tick(&app.db_pool).await?;
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
    let mut auction_details = helpers::auction_details_a(site.site_id);

    // Set start time just before DST transition (March 10, 2024 1:59 AM PST)
    let start_time: Zoned =
        "2024-03-10T01:59:00-08:00[America/Los_Angeles]".parse()?;
    let start_time = start_time.timestamp();
    time::set_mock_time(start_time);
    auction_details.start_at = start_time;

    // Set round duration to one day
    auction_details.auction_params.round_duration = jiff::Span::new().days(1);

    // Create the auction
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Start the auction to create initial round
    scheduler::schedule_tick(&app.db_pool).await?;

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
async fn test_subsequent_auction_round_creation() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Create an auction that starts now
    let start_time = time::now();
    let auction = app.create_test_auction(&site.site_id).await?;
    scheduler::schedule_tick(&app.db_pool).await?;

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

    // test that the subsequent round gets created
    time::set_mock_time(round.round_details.end_at + Span::new().seconds(1));
    scheduler::schedule_tick(&app.db_pool).await?;

    let rounds = app.client.list_auction_rounds(&auction.auction_id).await?;
    assert_eq!(rounds.len(), 2);
    let round = &rounds[1];
    assert_eq!(round.round_details.round_num, 1);
    assert_eq!(round.round_details.auction_id, auction.auction_id);
    assert_eq!(
        round.round_details.start_at,
        start_time + Span::new().minutes(1)
    );
    assert_eq!(
        round.round_details.end_at,
        start_time + Span::new().minutes(2)
    );
    assert_eq!(round.round_details.eligibility_threshold, 0.5);

    Ok(())
}
