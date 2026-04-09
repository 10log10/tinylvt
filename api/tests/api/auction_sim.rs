use std::collections::HashMap;

use api::scheduler;
use jiff::Span;
use payloads::{auction_sim, requests, responses};
use rust_decimal::Decimal;
use test_helpers::{self, spawn_app};

#[tokio::test]
async fn test_simulation_matches_full_system() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Create two spaces
    let space_a = app.create_test_space(&site.site_id).await?;
    let space_b = app
        .client
        .create_space(&test_helpers::space_details_b(site.site_id))
        .await?;
    let space_b = app.client.get_space(&space_b).await?;

    // Create auction starting now
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = start_time;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Set user values: Alice(A=5, B=2), Bob(B=4)
    app.login_alice().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: space_a.space_id,
            value: Decimal::new(5, 0),
        })
        .await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: space_b.space_id,
            value: Decimal::new(2, 0),
        })
        .await?;
    let alice_profile = app.client.user_profile().await?;

    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: space_b.space_id,
            value: Decimal::new(4, 0),
        })
        .await?;
    let bob_profile = app.client.user_profile().await?;

    // Enable proxy bidding with max_items = 1
    app.login_alice().await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;
    app.login_bob().await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    // Run auction to completion via scheduler
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    loop {
        let rounds = app.client.list_auction_rounds(&auction_id).await?;
        let latest_round = rounds.last().unwrap();
        app.time_source
            .set(latest_round.round_details.end_at + Span::new().seconds(1));
        scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
        let auction = app.client.get_auction(&auction_id).await?;
        if auction.end_at.is_some() {
            break;
        }
    }

    // Collect full-system results per round
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let mut system_rounds: Vec<Vec<(String, Decimal)>> = Vec::new();
    for round in &rounds {
        let results = app
            .client
            .list_round_space_results_for_round(&round.round_id)
            .await?;
        let mut round_data: Vec<(String, Decimal)> = results
            .iter()
            .map(|r| (r.winner.username.clone(), r.value))
            .collect();
        round_data.sort_by(|a, b| a.0.cmp(&b.0));
        system_rounds.push(round_data);
    }

    // Run simulation with the same inputs
    let sim_result = auction_sim::simulate_auction(&auction_sim::SimInput {
        spaces: vec![
            (space_a.space_id, space_a.space_details.name.clone()),
            (space_b.space_id, space_b.space_details.name.clone()),
        ],
        bidders: vec![
            responses::UserIdentity {
                user_id: alice_profile.user_id,
                username: alice_profile.username.clone(),
                display_name: None,
            },
            responses::UserIdentity {
                user_id: bob_profile.user_id,
                username: bob_profile.username.clone(),
                display_name: None,
            },
        ],
        user_values: HashMap::from([
            (
                (alice_profile.user_id, space_a.space_id),
                Decimal::new(5, 0),
            ),
            (
                (alice_profile.user_id, space_b.space_id),
                Decimal::new(2, 0),
            ),
            ((bob_profile.user_id, space_b.space_id), Decimal::new(4, 0)),
        ]),
        bid_increment: Decimal::new(1, 0),
    });

    assert_eq!(
        sim_result.len(),
        system_rounds.len(),
        "Number of rounds should match: sim={}, system={}",
        sim_result.len(),
        system_rounds.len()
    );

    // Compare each round's results
    for (i, (sim_round, sys_results)) in
        sim_result.iter().zip(system_rounds.iter()).enumerate()
    {
        assert_eq!(sim_round.round_num, i as i32);

        let mut sim_data: Vec<(String, Decimal)> = sim_round
            .results
            .iter()
            .map(|r| (r.winner.username.clone(), r.value))
            .collect();
        sim_data.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(
            &sim_data, sys_results,
            "Round {} results mismatch.\n  sim: {:?}\n  sys: {:?}",
            i, sim_data, sys_results
        );
    }

    Ok(())
}
