use std::time::Duration;

use api::scheduler;
use jiff::Span;
use payloads::{AuctionEvent, requests};
use reqwest::header::ACCEPT;
use rust_decimal::Decimal;
use test_helpers::spawn_app;
use tokio::sync::broadcast;

/// Drain pending events from the broadcast receiver with a short timeout
/// between attempts so we wait for the LISTEN→broadcast hop to land.
async fn collect_events(
    rx: &mut broadcast::Receiver<AuctionEvent>,
    expected: usize,
) -> Vec<AuctionEvent> {
    let mut events = Vec::new();
    while events.len() < expected {
        match tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
            Ok(Ok(e)) => events.push(e),
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(broadcast::error::RecvError::Closed)) => break,
            Err(_) => break,
        }
    }
    events
}

#[tokio::test]
async fn round_created_event_emitted_on_first_round() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let mut rx = app.pubsub.subscribe();

    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 1);
    let round_id = rounds[0].round_id;

    let events = collect_events(&mut rx, 1).await;
    assert!(
        events.iter().any(|e| matches!(
            e,
            AuctionEvent::RoundCreated { auction_id: a, round_id: r }
                if *a == auction_id && *r == round_id
        )),
        "expected RoundCreated for {auction_id}/{round_id}, got {events:?}",
    );

    Ok(())
}

#[tokio::test]
async fn auction_ended_event_emitted_on_completion() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let mut rx = app.pubsub.subscribe();

    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let _space = app.create_test_space(&site.site_id).await?;

    // Auction starts now; no bids will be placed.
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // First tick creates round 0.
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_0 = &rounds[0];

    // No bids; advance past round end and tick again to conclude.
    app.time_source
        .set(round_0.round_details.end_at + Span::new().seconds(1));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // Expect round 0's RoundCreated, then RoundEnded(round_0) and
    // AuctionEnded(auction) from the concluding tick.
    let events = collect_events(&mut rx, 3).await;
    assert!(
        events.iter().any(|e| matches!(
            e,
            AuctionEvent::AuctionEnded { auction_id: a } if *a == auction_id
        )),
        "expected AuctionEnded for {auction_id}, got {events:?}",
    );

    Ok(())
}

#[tokio::test]
async fn round_ended_event_emitted_on_round_transition() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let mut rx = app.pubsub.subscribe();

    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let space = app.create_test_space(&site.site_id).await?;

    // Auction starts now.
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Tick to create round 0.
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_0_id = rounds[0].round_id;

    // Place a bid on round 0 so the auction continues past it (so the next
    // tick creates round 1 rather than concluding).
    app.client.create_bid(&space.space_id, &round_0_id).await?;

    // Advance past round 0's end and tick to create round 1.
    app.time_source
        .set(rounds[0].round_details.end_at + Span::new().seconds(1));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // Expect: RoundCreated(round_0) and BidsChanged(round_0) from the first
    // tick + manual bid; RoundEnded(round_0) and RoundCreated(round_1) from
    // the second tick.
    let events = collect_events(&mut rx, 4).await;
    assert!(
        events.iter().any(|e| matches!(
            e,
            AuctionEvent::RoundEnded { auction_id: a, round_id: r }
                if *a == auction_id && *r == round_0_id
        )),
        "expected RoundEnded for {round_0_id}, got {events:?}",
    );

    // RoundEnded should come before the second RoundCreated.
    let pos_ended = events.iter().position(|e| {
        matches!(
            e,
            AuctionEvent::RoundEnded { round_id: r, .. } if *r == round_0_id
        )
    });
    let pos_created_round_1 = events.iter().position(|e| {
        matches!(
            e,
            AuctionEvent::RoundCreated { round_id: r, .. } if *r != round_0_id
        )
    });
    assert!(
        pos_ended < pos_created_round_1,
        "RoundEnded should arrive before the next RoundCreated, got {events:?}",
    );

    Ok(())
}

#[tokio::test]
async fn bids_changed_event_emitted_for_proxy_bidder() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let mut rx = app.pubsub.subscribe();

    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let space = app.create_test_space(&site.site_id).await?;

    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Alice configures proxy bidding with a value high enough to bid.
    app.login_alice().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: space.space_id,
            value: Decimal::new(5, 0),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    // Tick: creates round 0 (RoundCreated) AND runs proxy bidding for alice
    // (BidsChanged). Both happen in the same tick but in separate transactions.
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_0_id = rounds[0].round_id;

    let events = collect_events(&mut rx, 2).await;
    assert!(
        events.iter().any(|e| matches!(
            e,
            AuctionEvent::BidsChanged {
                auction_id: a,
                round_id: r,
                ..
            } if *a == auction_id && *r == round_0_id
        )),
        "expected BidsChanged for round {round_0_id}, got {events:?}",
    );

    Ok(())
}

#[tokio::test]
async fn auction_ended_event_emitted_on_site_soft_delete() -> anyhow::Result<()>
{
    let app = spawn_app().await;
    let mut rx = app.pubsub.subscribe();

    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Two active auctions for the site.
    let auction_a = app.create_test_auction(&site.site_id).await?;
    let auction_b = app.create_test_auction(&site.site_id).await?;

    // Drain anything that landed up to this point (e.g., from auto-scheduling).
    while let Ok(Ok(_)) =
        tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
    {}

    app.client.soft_delete_site(&site.site_id).await?;

    let events = collect_events(&mut rx, 2).await;
    let mut ended_ids: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AuctionEvent::AuctionEnded { auction_id } => Some(*auction_id),
            _ => None,
        })
        .collect();
    ended_ids.sort_by_key(|id| id.0);
    let mut expected = vec![auction_a.auction_id, auction_b.auction_id];
    expected.sort_by_key(|id| id.0);
    assert_eq!(ended_ids, expected, "got events: {events:?}");

    Ok(())
}

#[tokio::test]
async fn pubsub_reset_closes_client_sse_stream() -> anyhow::Result<()> {
    let app = spawn_app().await;

    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let auction = app.create_test_auction(&site.site_id).await?;

    let url = app.client.sse_auction_url(auction.auction_id);
    let mut response = app
        .client
        .inner_client
        .get(&url)
        .header(ACCEPT, "text/event-stream")
        .send()
        .await?;
    assert!(response.status().is_success());

    // Simulate a missed-event reset (the same call the listener makes on
    // entry / on Ok(None)). The SSE handler's Closed arm should drop the
    // streaming response, which propagates to the client as end-of-body.
    app.pubsub.reset();

    // Drain the body until the server closes it. The stream may carry an
    // initial heartbeat comment before close, so we read chunks until
    // exhausted rather than asserting on chunk contents.
    tokio::time::timeout(Duration::from_secs(5), async {
        while response.chunk().await?.is_some() {}
        Ok::<_, reqwest::Error>(())
    })
    .await
    .map_err(|_| {
        anyhow::anyhow!("timed out waiting for SSE close after pubsub.reset()")
    })??;

    Ok(())
}
