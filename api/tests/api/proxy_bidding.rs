use api::scheduler;
use jiff::Span;
use jiff_sqlx::ToSqlx;
use payloads::requests;
use rust_decimal::Decimal;
use sqlx::Row;
use test_helpers::{self, spawn_app};

#[tokio::test]
async fn test_proxy_bidding_two_spaces_auction() -> anyhow::Result<()> {
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

    // Create an auction that starts now
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(start_time);
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
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // Check initial round creation
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let _round = &rounds[0];

    // Run the auction to completion by repeatedly advancing time and scheduling
    // ticks
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
        scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

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
        space_a_result.winner.username, "alice",
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
        space_b_result.winner.username, "bob",
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
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(start_time);
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
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
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

#[tokio::test]
async fn test_proxy_bidding_three_bidders_debug() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_three_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Create three spaces: A, B, C
    let space_a = app.create_test_space(&site.site_id).await?; // This will be "test space" (A)
    let space_b_id = app
        .client
        .create_space(&test_helpers::space_details_b(site.site_id))
        .await?;
    let space_b = app.client.get_space(&space_b_id).await?;
    let space_c_id = app
        .client
        .create_space(&test_helpers::space_details_c(site.site_id))
        .await?;
    let space_c = app.client.get_space(&space_c_id).await?;

    println!("Created spaces:");
    println!(
        "Space A: {} (id: {:?})",
        space_a.space_details.name, space_a.space_id
    );
    println!(
        "Space B: {} (id: {:?})",
        space_b.space_details.name, space_b.space_id
    );
    println!(
        "Space C: {} (id: {:?})",
        space_c.space_details.name, space_c.space_id
    );

    // Create an auction that starts now
    let start_time = app.time_source.now();
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(start_time);
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Set user values for Bidder 1 (alice): A=5, B=0, max_items=2
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
            value: Decimal::new(0, 0), // Alice values space B at 0
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 2,
        })
        .await?;

    // Set user values for Bidder 2 (bob): A=4, C=3, max_items=1
    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: space_a.space_id,
            value: Decimal::new(4, 0), // Bob values space A at 4
        })
        .await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: space_c.space_id,
            value: Decimal::new(3, 0), // Bob values space C at 3
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    // Set user values for Bidder 3 (charlie): B=2, C=9, max_items=1
    app.login_charlie().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: space_b.space_id,
            value: Decimal::new(2, 0), // Charlie values space B at 2
        })
        .await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: space_c.space_id,
            value: Decimal::new(9, 0), // Charlie values space C at 9
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    println!("\nBidder valuations:");
    println!("Alice (Bidder 1): A=5, B=0, max_items=2");
    println!("Bob (Bidder 2): A=4, C=3, max_items=1");
    println!("Charlie (Bidder 3): B=2, C=9, max_items=1");

    // Create initial round and run proxy bidding
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // Run the auction to completion
    let mut rounds_processed = 0;
    const MAX_ROUNDS: usize = 20;
    let mut prev_round_id = None;

    loop {
        if rounds_processed >= MAX_ROUNDS {
            panic!("Auction did not complete within {} rounds", MAX_ROUNDS);
        }

        let rounds = app.client.list_auction_rounds(&auction_id).await?;
        let latest_round = &rounds[rounds.len() - 1];

        // Get space results for the previous round (if any)
        if let Some(prev_round_id) = &prev_round_id {
            let space_results = app
                .client
                .list_round_space_results_for_round(prev_round_id)
                .await?;

            println!("Space results:");
            for result in &space_results {
                let space = app.client.get_space(&result.space_id).await?;
                println!(
                    "  {}: price={}, winner={:?}",
                    space.space_details.name,
                    result.value,
                    result.winner.username
                );
            }
        } else {
            println!("No previous round results yet");
        }

        // Store current round ID for next iteration
        prev_round_id = Some(latest_round.round_id);

        println!("\n=== Round {} ===", latest_round.round_details.round_num);

        // Get bids for this round
        let bids = sqlx::query(
            "SELECT b.user_id, u.username, s.name as space_name, b.space_id
             FROM bids b
             JOIN users u ON b.user_id = u.id
             JOIN spaces s ON b.space_id = s.id
             WHERE b.round_id = $1
             ORDER BY u.username, s.name",
        )
        .bind(latest_round.round_id)
        .fetch_all(&app.db_pool)
        .await?;

        if !bids.is_empty() {
            println!("Bids placed:");
            for bid in &bids {
                let username: String = bid.get("username");
                let space_name: String = bid.get("space_name");
                println!("  {} bid on {}", username, space_name);
            }
        } else {
            println!("No bids placed");
        }

        // Advance time past the round end
        app.time_source
            .set(latest_round.round_details.end_at + Span::new().seconds(1));

        // Create the next round and do proxy bidding
        scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

        // Check if auction has ended
        let auction = app.client.get_auction(&auction_id).await?;
        if auction.end_at.is_some() {
            println!("\nAuction completed!");
            break;
        }

        rounds_processed += 1;
    }

    // Get final results
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let final_round = rounds.last().unwrap();
    let round_results = app
        .client
        .list_round_space_results_for_round(&final_round.round_id)
        .await?;

    println!("\n=== FINAL RESULTS ===");

    // Track how many spaces each bidder has won
    let mut spaces_won = std::collections::HashMap::new();

    // First pass: count spaces won by each bidder
    for result in &round_results {
        *spaces_won.entry(&result.winner.username).or_insert(0) += 1;
    }

    // Second pass: print results and check max_items constraints
    for result in &round_results {
        let space_name = match result.space_id {
            id if id == space_a.space_id => "A",
            id if id == space_b.space_id => "B",
            id if id == space_c.space_id => "C",
            _ => "Unknown",
        };

        let winner = &result.winner.username;
        println!(
            "Space {}: Winner = {}, Price = {}",
            space_name, winner, result.value
        );

        // Check max_items constraint
        let max_items = match winner.as_str() {
            "alice" => 2,   // Alice's max_items
            "bob" => 1,     // Bob's max_items
            "charlie" => 1, // Charlie's max_items
            _ => 0,
        };

        let spaces = spaces_won.get(winner).copied().unwrap_or(0);
        assert!(
            spaces <= max_items,
            "{} won {} spaces but max_items was set to {}",
            winner,
            spaces,
            max_items
        );
    }

    Ok(())
}

/// Coleaders+ can list which members have enabled proxy bidding for an
/// auction (to nudge others), but plain members cannot. The response
/// carries identities only, never `max_items`.
#[tokio::test]
async fn test_list_proxy_bidding_participants_permissions() -> anyhow::Result<()>
{
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Auction scheduled to start in the future (the list is a pre-start
    // nudge tool).
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at =
        Some(app.time_source.now() + Span::new().hours(1));
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Only Bob (a plain member) enables proxy bidding; Alice (the leader)
    // does not. The list should contain Bob but not Alice.
    app.login_bob().await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 3,
        })
        .await?;

    // A plain member cannot see the participant list.
    let member_result = app
        .client
        .list_proxy_bidding_participants(&auction_id)
        .await;
    assert!(
        matches!(member_result, Err(payloads::ClientError::APIError(..))),
        "member should be denied, got {member_result:?}"
    );

    // The leader can, and sees exactly the members who opted in.
    app.login_alice().await?;
    let participants = app
        .client
        .list_proxy_bidding_participants(&auction_id)
        .await?;
    assert_eq!(participants.len(), 1, "only Bob opted in");
    assert_eq!(
        participants[0].username,
        test_helpers::bob_credentials().username,
    );

    // Once the auction has started, the list is no longer retrievable.
    app.time_source.advance(Span::new().hours(2));
    let after_start = app
        .client
        .list_proxy_bidding_participants(&auction_id)
        .await;
    assert!(
        matches!(after_start, Err(payloads::ClientError::APIError(..))),
        "list should be refused after start, got {after_start:?}"
    );

    Ok(())
}

/// A member's proxy bidding settings are deleted when they leave a
/// community, whether they leave voluntarily or are removed. This keeps a
/// `use_proxy_bidding` row meaning "a current member is proxy bidding".
#[tokio::test]
async fn test_proxy_bidding_deleted_on_community_exit() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Future start so the leader can query the participant list (only
    // available pre-start).
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at =
        Some(app.time_source.now() + Span::new().hours(1));
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Bob opts into proxy bidding, then leaves voluntarily.
    app.login_bob().await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;
    app.client
        .leave_community(&requests::LeaveCommunity { community_id })
        .await?;

    // The leader's participant list is now empty.
    app.login_alice().await?;
    let after_leave = app
        .client
        .list_proxy_bidding_participants(&auction_id)
        .await?;
    assert!(
        after_leave.is_empty(),
        "leaving should delete proxy bidding, got {after_leave:?}"
    );

    // Bob rejoins and re-enables, then the leader removes him. Removal must
    // also clear the settings.
    app.invite_bob().await?;
    app.login_bob().await?;
    app.accept_invite().await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    let members = app.client.get_members(&community_id).await?;
    let bob_id = members
        .iter()
        .find(|m| m.user.username == test_helpers::bob_credentials().username)
        .expect("Bob is a member again")
        .user
        .user_id;

    app.login_alice().await?;
    app.client
        .remove_member(&requests::RemoveMember {
            community_id,
            member_user_id: bob_id,
        })
        .await?;

    let after_remove = app
        .client
        .list_proxy_bidding_participants(&auction_id)
        .await?;
    assert!(
        after_remove.is_empty(),
        "removal should delete proxy bidding, got {after_remove:?}"
    );

    Ok(())
}

/// (processed_at, failure_count, last_failed_at) for a user's marker row,
/// timestamps as text for cheap change/equality comparisons.
async fn proxy_marker(
    pool: &sqlx::PgPool,
    round_id: &payloads::AuctionRoundId,
    username: &str,
) -> anyhow::Result<Option<(Option<String>, i32, Option<String>)>> {
    Ok(sqlx::query_as::<_, (Option<String>, i32, Option<String>)>(
        "SELECT prp.processed_at::text, prp.failure_count,
            prp.last_failed_at::text
        FROM proxy_round_processing prp
        JOIN users u ON prp.user_id = u.id
        WHERE prp.round_id = $1 AND u.username = $2",
    )
    .bind(round_id)
    .bind(username)
    .fetch_optional(pool)
    .await?)
}

async fn needs_processing(
    pool: &sqlx::PgPool,
    auction_id: &payloads::AuctionId,
    username: &str,
) -> anyhow::Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT upb.needs_processing FROM use_proxy_bidding upb
        JOIN users u ON upb.user_id = u.id
        WHERE upb.auction_id = $1 AND u.username = $2",
    )
    .bind(auction_id)
    .bind(username)
    .fetch_one(pool)
    .await?)
}

async fn bid_count(
    pool: &sqlx::PgPool,
    round_id: &payloads::AuctionRoundId,
    username: &str,
) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM bids b
        JOIN users u ON b.user_id = u.id
        WHERE b.round_id = $1 AND u.username = $2",
    )
    .bind(round_id)
    .bind(username)
    .fetch_one(pool)
    .await?)
}

/// Shared setup for the granular-processing regression tests: alice and bob
/// both have a value on one space and proxy bidding enabled, the auction has
/// started, and one tick has run baseline processing for round 0. Returns
/// (auction_id, round_id, space_id).
async fn setup_processed_round(
    app: &test_helpers::TestApp,
) -> anyhow::Result<(
    payloads::AuctionId,
    payloads::AuctionRoundId,
    payloads::SpaceId,
)> {
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let space = app.create_test_space(&site.site_id).await?;

    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(app.time_source.now());
    let auction_id = app.client.create_auction(&auction_details).await?;

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

    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: space.space_id,
            value: Decimal::new(4, 0),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    // Round 0 creation + baseline proxy processing for both users
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    assert_eq!(rounds.len(), 1);
    let round_id = rounds[0].round_id;

    for user in ["alice", "bob"] {
        let marker = proxy_marker(&app.db_pool, &round_id, user)
            .await?
            .expect("baseline processing creates a marker");
        assert!(marker.0.is_some(), "{user} should be processed");
        assert_eq!(marker.1, 0);
        assert!(!needs_processing(&app.db_pool, &auction_id, user).await?);
        assert_eq!(bid_count(&app.db_pool, &round_id, user).await?, 1);
    }

    Ok((auction_id, round_id, space.space_id))
}

/// One user's processing failure must not affect other users' items or put
/// the round into backoff (the old design's round-level failure mark gated
/// everyone), and the failed item must obey per-user backoff — except that a
/// settings change (dirty flag) makes it due immediately as a fresh-input
/// retry. The failure is injected with a trigger that rejects deleting
/// alice's bids, which is the first write of her item's reprocessing work.
#[tokio::test]
async fn test_per_user_failure_isolation() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let (auction_id, round_id, _) = setup_processed_round(&app).await?;

    let alice_id: String = sqlx::query_scalar(
        "SELECT id::text FROM users WHERE username = 'alice'",
    )
    .fetch_one(&app.db_pool)
    .await?;
    sqlx::query(
        "CREATE FUNCTION inject_bid_delete_failure() RETURNS trigger AS $$
        BEGIN
            RAISE EXCEPTION 'injected bid delete failure';
        END $$ LANGUAGE plpgsql",
    )
    .execute(&app.db_pool)
    .await?;
    sqlx::query(&format!(
        "CREATE TRIGGER inject_bid_delete_failure BEFORE DELETE ON bids
        FOR EACH ROW WHEN (OLD.user_id = '{alice_id}'::uuid)
        EXECUTE FUNCTION inject_bid_delete_failure()",
    ))
    .execute(&app.db_pool)
    .await?;

    // Both users change their settings mid-round, making both items due.
    app.login_alice().await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 2,
        })
        .await?;
    app.login_bob().await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 2,
        })
        .await?;

    app.time_source.advance(Span::new().seconds(1));
    let t1 = app.time_source.now();
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // Bob reprocessed fine; alice's failure was recorded on her marker only.
    let bob = proxy_marker(&app.db_pool, &round_id, "bob").await?.unwrap();
    assert_eq!(bob.1, 0, "bob must not inherit alice's failure");
    let alice = proxy_marker(&app.db_pool, &round_id, "alice")
        .await?
        .unwrap();
    assert_eq!(alice.1, 1, "alice's failure recorded");
    assert!(alice.2.is_some());
    // The savepoint rollback preserved her existing bids, and the flag
    // stayed cleared (failures never re-set it).
    assert_eq!(bid_count(&app.db_pool, &round_id, "alice").await?, 1);
    assert!(!needs_processing(&app.db_pool, &auction_id, "alice").await?);

    // Within backoff and with no input change, alice is not retried.
    app.time_source.advance(Span::new().seconds(1));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let alice = proxy_marker(&app.db_pool, &round_id, "alice")
        .await?
        .unwrap();
    assert_eq!(alice.1, 1, "no blind retry inside backoff");

    // A settings change makes her due immediately (fresh-input retry
    // through the flag arm, bypassing backoff).
    sqlx::query("DROP TRIGGER inject_bid_delete_failure ON bids")
        .execute(&app.db_pool)
        .await?;
    app.login_alice().await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;
    app.time_source.advance(Span::new().seconds(1));
    let t3 = app.time_source.now();
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    let alice = proxy_marker(&app.db_pool, &round_id, "alice")
        .await?
        .unwrap();
    assert_eq!(alice.1, 0, "successful retry resets failure tracking");
    assert!(alice.2.is_none());
    assert!(t3 > t1); // distinct instants, so processed_at moving is provable
    let bob_after =
        proxy_marker(&app.db_pool, &round_id, "bob").await?.unwrap();
    assert_eq!(bob, bob_after, "alice's retry must not reprocess bob");

    Ok(())
}

/// One user's mid-round settings change reprocesses only that user's item.
/// (The old design reprocessed every user in the round.)
#[tokio::test]
async fn test_per_user_reprocessing() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let (auction_id, round_id, _) = setup_processed_round(&app).await?;

    let bob_before =
        proxy_marker(&app.db_pool, &round_id, "bob").await?.unwrap();
    let alice_before = proxy_marker(&app.db_pool, &round_id, "alice")
        .await?
        .unwrap();

    // Alice changes a value mid-round (a distinct instant, so her
    // processed_at provably moves).
    app.time_source.advance(Span::new().seconds(1));
    app.login_alice().await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 2,
        })
        .await?;
    assert!(needs_processing(&app.db_pool, &auction_id, "alice").await?);
    assert!(!needs_processing(&app.db_pool, &auction_id, "bob").await?);

    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    let alice_after = proxy_marker(&app.db_pool, &round_id, "alice")
        .await?
        .unwrap();
    assert_ne!(
        alice_before.0, alice_after.0,
        "alice's item was reprocessed"
    );
    let bob_after =
        proxy_marker(&app.db_pool, &round_id, "bob").await?.unwrap();
    assert_eq!(bob_before, bob_after, "bob's item was left alone");

    Ok(())
}

/// The watermark race, simulated with a real two-connection straddle: a
/// user-value write transaction is held open across a full processing pass
/// and commits afterwards, bearing a timestamp equal to the instant the
/// pass ran. The old design compared `updated_at` against a processing-time
/// watermark, so this write was lost forever (`T > T` is false); the dirty
/// flag commits with the writer's own transaction, so the next tick picks
/// it up regardless of timestamps.
#[tokio::test]
async fn test_straddling_write_not_lost() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let (auction_id, round_id, _) = setup_processed_round(&app).await?;

    // Open the writer transaction on its own connection: alice's value
    // drops below the next bid amount (so reprocessing must delete her
    // round-0 bid), mirroring create_or_update_user_value's statements.
    let mut writer_tx = app.db_pool.begin().await?;
    sqlx::query(
        "UPDATE user_values SET value = -3, updated_at = $1
        WHERE user_id = (SELECT id FROM users WHERE username = 'alice')",
    )
    .bind(app.time_source.now().to_sqlx())
    .execute(&mut *writer_tx)
    .await?;
    sqlx::query(
        "UPDATE use_proxy_bidding SET needs_processing = TRUE
        WHERE auction_id = $1
        AND user_id = (SELECT id FROM users WHERE username = 'alice')",
    )
    .bind(auction_id)
    .execute(&mut *writer_tx)
    .await?;

    // A full processing pass runs while the write is in flight and
    // invisible; nothing is due, and alice's bid survives.
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    assert_eq!(bid_count(&app.db_pool, &round_id, "alice").await?, 1);

    // The writer commits after the pass, with updated_at equal to the
    // pass's own instant — exactly the straddle the watermark lost.
    writer_tx.commit().await?;

    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    // The flag arm re-selected alice: her new negative value yields no
    // bids, so reprocessing deleted the round-0 bid.
    assert_eq!(
        bid_count(&app.db_pool, &round_id, "alice").await?,
        0,
        "the straddling write must be picked up via the dirty flag"
    );
    assert!(!needs_processing(&app.db_pool, &auction_id, "alice").await?);
    assert_eq!(bid_count(&app.db_pool, &round_id, "bob").await?, 1);

    Ok(())
}

/// The user-value writers set the dirty flag through the store layer: a
/// mid-round value save reprocesses only that user's item, and a value
/// deletion reprocesses too (the old timestamp watermark could never see a
/// deletion — the deleted row has no updated_at to compare). Ends by
/// canceling the auction and confirming a canceled auction's items are no
/// longer selected even with the flag set.
#[tokio::test]
async fn test_user_value_writers_trigger_reprocessing() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let (auction_id, round_id, space_id) = setup_processed_round(&app).await?;

    // Value save mid-round: only alice's item is reprocessed.
    let alice_before = proxy_marker(&app.db_pool, &round_id, "alice")
        .await?
        .unwrap();
    let bob_before =
        proxy_marker(&app.db_pool, &round_id, "bob").await?.unwrap();
    app.time_source.advance(Span::new().seconds(1));
    app.login_alice().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id,
            value: Decimal::new(6, 0),
        })
        .await?;
    assert!(needs_processing(&app.db_pool, &auction_id, "alice").await?);
    assert!(!needs_processing(&app.db_pool, &auction_id, "bob").await?);
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;

    let alice_after = proxy_marker(&app.db_pool, &round_id, "alice")
        .await?
        .unwrap();
    assert_ne!(alice_before.0, alice_after.0, "alice reprocessed");
    let bob_after =
        proxy_marker(&app.db_pool, &round_id, "bob").await?.unwrap();
    assert_eq!(bob_before, bob_after, "bob left alone");
    assert_eq!(bid_count(&app.db_pool, &round_id, "alice").await?, 1);

    // Value deletion: reprocessed, and with no value there is no surplus,
    // so alice's round-0 bid is removed.
    app.time_source.advance(Span::new().seconds(1));
    app.client.delete_user_value(&space_id).await?;
    assert!(needs_processing(&app.db_pool, &auction_id, "alice").await?);
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    assert_eq!(
        bid_count(&app.db_pool, &round_id, "alice").await?,
        0,
        "deletion must trigger reprocessing that removes the bid"
    );
    assert!(!needs_processing(&app.db_pool, &auction_id, "alice").await?);

    // A canceled auction's items are excluded from selection even when
    // flagged: the flag stays set (nothing claims the item to clear it)
    // and the marker never moves.
    app.client.cancel_auction(&auction_id).await?;
    sqlx::query(
        "UPDATE use_proxy_bidding SET needs_processing = TRUE
        WHERE auction_id = $1",
    )
    .bind(auction_id)
    .execute(&app.db_pool)
    .await?;
    let alice_canceled_before = proxy_marker(&app.db_pool, &round_id, "alice")
        .await?
        .unwrap();
    app.time_source.advance(Span::new().seconds(1));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
    let alice_canceled_after = proxy_marker(&app.db_pool, &round_id, "alice")
        .await?
        .unwrap();
    assert_eq!(
        alice_canceled_before, alice_canceled_after,
        "no processing after cancel"
    );
    assert!(
        needs_processing(&app.db_pool, &auction_id, "alice").await?,
        "item never claimed, so the flag stays set"
    );

    Ok(())
}
