use crate::helpers::{self, spawn_app};
use backend::scheduler;
use payloads::requests;
use rust_decimal::Decimal;

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

