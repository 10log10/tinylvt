use crate::helpers::{self, spawn_app};
use backend::scheduler;
use jiff::Span;
use payloads::requests;
use rust_decimal::Decimal;

#[tokio::test]
async fn test_proxy_bidding_two_spaces_auction() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Create two spaces
    let space_a = app.create_test_space(&site.site_id).await?;
    let space_b = app
        .client
        .create_space(&helpers::space_details_b(site.site_id))
        .await?;
    let space_b = app.client.get_space(&space_b).await?;

    // Create an auction that starts now
    let start_time = app.time_source.now();
    let mut auction_details =
        helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Set user values for Alice: space A = 5, space B = 2
    app.login_alice().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: space_a.space_id,
            value: Decimal::new(5, 0), // Alice values space A at 5
        })
        .await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: space_b.space_id,
            value: Decimal::new(2, 0), // Alice values space B at 2
        })
        .await?;

    // Set user value for Bob: space B = 4
    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: space_b.space_id,
            value: Decimal::new(4, 0), // Bob values space B at 4
        })
        .await?;

    // Enable proxy bidding for Alice with max 2 items
    app.login_alice().await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 2,
        })
        .await?;

    // Enable proxy bidding for Bob with max 1 item
    app.login_bob().await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    // Create initial round and do first round proxy bidding
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

    // Check initial round creation
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let _round = &rounds[0];

    // Run the auction to completion by repeatedly advancing time and scheduling ticks
    let mut rounds_processed = 0;
    const MAX_ROUNDS: usize = 20; // Safety limit to prevent infinite loops

    loop {
        if rounds_processed >= MAX_ROUNDS {
            panic!("Auction did not complete within {} rounds", MAX_ROUNDS);
        }

        let rounds = app.client.list_auction_rounds(&auction_id).await?;
        let latest_round = &rounds[rounds.len() - 1];

        // Advance time past the round end
        app.time_source
            .set(latest_round.round_details.end_at + Span::new().seconds(1));

        // Create the next round and do proxy bidding
        scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

        // Check if auction has ended
        let auction = app.client.get_auction(&auction_id).await?;
        if auction.end_at.is_some() {
            break;
        }

        rounds_processed += 1;
    }

    // Verify the auction has completed
    let auction = app.client.get_auction(&auction_id).await?;
    assert!(auction.end_at.is_some(), "Auction should have completed");

    // Get all rounds and find the final round results
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert!(!rounds.is_empty(), "Should have at least one round");

    // Get the last round results
    let round_results = app
        .client
        .list_round_space_results_for_round(&rounds.last().unwrap().round_id)
        .await?;

    // Find results for each space
    let space_a_result = round_results
        .iter()
        .find(|r| r.space_id == space_a.space_id)
        .expect("Space A should have a result");
    let space_b_result = round_results
        .iter()
        .find(|r| r.space_id == space_b.space_id)
        .expect("Space B should have a result");

    // Verify Alice wins space A for a price of 0 (no competition)
    assert_eq!(
        space_a_result.winning_username.as_deref(),
        Some("alice"),
        "Alice should win space A"
    );
    assert_eq!(
        space_a_result.value,
        Decimal::ZERO,
        "Alice should win space A for price 0 (no competition)"
    );

    // Verify Bob wins space B for a price between Alice's max value and Alice's
    // max value + bid increment (The exact price depends on random winner
    // selection in rounds with multiple bids)
    assert_eq!(
        space_b_result.winning_username.as_deref(),
        Some("bob"),
        "Bob should win space B"
    );

    let alice_max_value = Decimal::new(2, 0);
    let bid_increment = Decimal::new(1, 0);
    let min_expected_price = alice_max_value;
    let max_expected_price = alice_max_value + bid_increment;

    assert!(
        space_b_result.value >= min_expected_price
            && space_b_result.value <= max_expected_price,
        "Bob should win space B for a price between {} and {} (Alice's max \
        value ± bid increment), but got {}",
        min_expected_price,
        max_expected_price,
        space_b_result.value
    );

    Ok(())
}

#[tokio::test]
async fn test_proxy_bidding_basic() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let space = app.create_test_space(&site.site_id).await?;

    // Create an auction that starts now
    let start_time = app.time_source.now();
    let mut auction_details =
        helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Set user values for both users
    app.login_alice().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: space.space_id,
            value: Decimal::new(100, 0), // Alice values at 100
        })
        .await?;

    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: space.space_id,
            value: Decimal::new(50, 0), // Bob values at 50
        })
        .await?;

    // Enable proxy bidding for Alice
    app.login_alice().await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    // Create initial round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 1);
    let round = &rounds[0];

    // Verify proxy bidding is set for Alice
    let proxy_bidding = app.client.get_proxy_bidding(&auction_id).await?;
    assert!(proxy_bidding.is_some());
    let proxy_bidding = proxy_bidding.unwrap();
    assert_eq!(proxy_bidding.max_items, 1);

    // Bob places a bid
    app.login_bob().await?;
    app.client
        .create_bid(&space.space_id, &round.round_id)
        .await?;

    // Delete proxy bidding for Alice
    app.login_alice().await?;
    app.client.delete_proxy_bidding(&auction_id).await?;

    // Verify proxy bidding is deleted
    let proxy_bidding = app.client.get_proxy_bidding(&auction_id).await?;
    assert!(proxy_bidding.is_none());

    Ok(())
}
