//! Comprehensive mock data module for TinyLVT testing
//!
//! This module provides realistic test data that can be used across:
//! - Development server (dev-server)
//! - API integration tests
//! - Any other testing scenarios
//!
//! The data is designed to represent real-world usage patterns with:
//! - Multiple communities (Alice's and Bob's)
//! - Different site types (coworking spaces, meeting rooms)
//! - Various auction states (upcoming, ongoing, concluded)
//! - Cross-community interactions (invites, memberships)

use crate::TestApp;
use anyhow::Result;
use api::scheduler;
use jiff::{Span, Timestamp};
use payloads::{CommunityId, SiteId, requests, responses};
use rust_decimal::Decimal;

// Single timezone for now
pub const TZ: &str = "America/Los_Angeles";

/// Comprehensive development dataset that creates a realistic TinyLVT environment
pub struct DevDataset {
    pub alice_community_id: CommunityId,
    pub bob_community_id: CommunityId,
    pub coworking_site: responses::Site,
    pub meetup_site: responses::Site,
    pub upcoming_auction: responses::Auction,
    pub ongoing_auction: responses::Auction,
    pub ongoing_auction_space_a: responses::Space,
    pub ongoing_auction_space_b: responses::Space,
    pub ongoing_auction_space_c: responses::Space,
}

impl DevDataset {
    /// Creates the complete development dataset with realistic hierarchical data
    pub async fn create(app: &TestApp) -> Result<Self> {
        app.time_source.set(Timestamp::now());

        // === Alice's Community with Three Members ===
        tracing::info!(
            "👤 Creating three-person community (Alice, Bob, Charlie)"
        );
        let alice_community_id = app.create_three_person_community().await?;

        // Create multiple sites with different characteristics
        let coworking_site =
            create_coworking_site(app, &alice_community_id).await?;
        let meetup_site = create_meetup_site(app, &alice_community_id).await?;

        // === Create Auctions in Different States ===
        tracing::info!("🏛️ Creating auctions with realistic work schedules");

        // Upcoming auction for day after tomorrow (9am-9pm possession)
        let upcoming_auction = create_work_day_auction(
            app,
            &coworking_site.site_id,
            2, // Day after tomorrow
            "upcoming",
        )
        .await?;

        // Ongoing auction with 12 processed rounds (three-bidders pattern)
        let (
            ongoing_auction,
            ongoing_auction_space_a,
            ongoing_auction_space_b,
            ongoing_auction_space_c,
        ) = create_ongoing_auction_with_rounds(app, &coworking_site.site_id)
            .await?;

        // === Bob's Community (Secondary community for multi-community testing) ===
        tracing::info!("👥 Creating Bob's secondary community");
        // Bob already exists from three-person community, so just login and create
        app.login_bob().await?;
        let bob_community_body = requests::CreateCommunity {
            name: "Tech Startup Collective".into(),
            currency: payloads::CurrencySettings {
                mode_config: crate::default_currency_config(),
                name: "dollars".into(),
                symbol: "$".into(),
                minor_units: 2,
                balances_visible_to_members: true,
                new_members_default_active: true,
            },
        };
        let bob_community_id =
            app.client.create_community(&bob_community_body).await?;

        // Create invite from Bob to Alice for cross-community testing
        create_cross_community_invite(app, &bob_community_id).await?;

        // Switch back to Alice for the main user session
        app.login_alice().await?;

        tracing::info!("✅ Comprehensive test dataset created");

        Ok(DevDataset {
            alice_community_id,
            bob_community_id,
            coworking_site,
            meetup_site,
            upcoming_auction,
            ongoing_auction,
            ongoing_auction_space_a,
            ongoing_auction_space_b,
            ongoing_auction_space_c,
        })
    }

    /// Print a summary of the created test data
    pub fn print_summary(&self) {
        tracing::info!("📋 Available test data:");
        tracing::info!(
            "   📊 Alice's Community ({}) - Three members: Alice, Bob, Charlie",
            self.alice_community_id
        );
        tracing::info!(
            "      - {} ({}): Coworking site",
            self.coworking_site.site_details.name,
            self.coworking_site.site_id
        );
        tracing::info!(
            "        ├─ {} ({})",
            self.ongoing_auction_space_a.space_details.name,
            self.ongoing_auction_space_a.space_id
        );
        tracing::info!(
            "        ├─ {} ({})",
            self.ongoing_auction_space_b.space_details.name,
            self.ongoing_auction_space_b.space_id
        );
        tracing::info!(
            "        └─ {} ({})",
            self.ongoing_auction_space_c.space_details.name,
            self.ongoing_auction_space_c.space_id
        );
        tracing::info!(
            "      - {} ({}): Meeting rooms",
            self.meetup_site.site_details.name,
            self.meetup_site.site_id
        );
        tracing::info!("   📊 Bob's Community ({}):", self.bob_community_id);
        tracing::info!("      - Basic community with invite to Alice");
        tracing::info!("   ⏰ Auctions:");
        tracing::info!(
            "      - Upcoming ({}): Starts in ~23 hours, for day after \
             tomorrow",
            self.upcoming_auction.auction_id
        );
        tracing::info!(
            "      - Ongoing ({}): Started 1 hour ago with 12 rounds of \
             bidding, for tomorrow",
            self.ongoing_auction.auction_id
        );
        tracing::info!("      - (Concluded auctions: TODO - add in future)");
    }
}

/// Creates a coworking site optimized for flexible desk rentals
async fn create_coworking_site(
    app: &TestApp,
    community_id: &CommunityId,
) -> Result<responses::Site> {
    use payloads::{ActivityRuleParams, AuctionParams, Site};

    let site_details = Site {
        community_id: *community_id,
        name: "Downtown Coworking Hub".to_string(),
        description: Some(
            "Modern coworking space with flexible desk rentals. \n\
            Open hours are weekdays 9 AM to 9 PM."
                .to_string(),
        ),
        default_auction_params: AuctionParams {
            round_duration: Span::new().minutes(3), // Fast-paced desk auctions
            bid_increment: Decimal::new(150, 2),    // $1.50 increments
            activity_rule_params: ActivityRuleParams {
                eligibility_progression: vec![
                    (0, 0.5),
                    (10, 0.75),
                    (20, 0.9),
                    (30, 1.0),
                ],
            },
        },
        possession_period: Span::new().hours(12), // How long the site is open
        auction_lead_time: Span::new().hours(24), // 1 day advance booking
        proxy_bidding_lead_time: Span::new().hours(12), // Half day for proxy bids
        open_hours: None,                               // Not for MVP
        auto_schedule: true,
        timezone: Some(TZ.to_string()),
        site_image_id: None,
    };

    let site_id = app.client.create_site(&site_details).await?;
    let site_response = app.client.get_site(&site_id).await?;
    Ok(site_response)
}

/// Creates a meeting room site optimized for longer bookings
async fn create_meetup_site(
    app: &TestApp,
    community_id: &CommunityId,
) -> Result<responses::Site> {
    use payloads::{ActivityRuleParams, AuctionParams, Site};

    let site_details = Site {
        community_id: *community_id,
        name: "Meeting Rooms".to_string(),
        description: Some(
            "Premium conference rooms with full AV setup".to_string(),
        ),
        default_auction_params: AuctionParams {
            round_duration: Span::new().minutes(10), // Longer rounds for bigger decisions
            bid_increment: Decimal::new(500, 2),     // $5.00 - higher stakes
            activity_rule_params: ActivityRuleParams {
                eligibility_progression: vec![
                    (0, 0.5),
                    (10, 0.75),
                    (20, 0.9),
                    (30, 1.0),
                ],
            },
        },
        possession_period: Span::new().hours(4), // 4-hour meeting blocks
        auction_lead_time: Span::new().hours(48), // 2 days advance for planning
        proxy_bidding_lead_time: Span::new().hours(24), // 1 day for proxy decisions
        open_hours: None,                               // Not for MVP
        auto_schedule: true,
        timezone: Some(TZ.to_string()),
        site_image_id: None,
    };

    let site_id = app.client.create_site(&site_details).await?;
    let site_response = app.client.get_site(&site_id).await?;
    Ok(site_response)
}

/// Creates a hot desk space for individual work
#[allow(dead_code)]
async fn create_desk_space(
    app: &TestApp,
    site_id: &SiteId,
) -> Result<responses::Space> {
    use payloads::Space;

    let space_details = Space {
        site_id: *site_id,
        name: "Hot Desk Alpha".to_string(),
        description: Some(
            "Standing/sitting desk with dual monitor setup and natural light"
                .to_string(),
        ),
        eligibility_points: 8.0, // Moderate desirability
        is_available: true,
        site_image_id: None,
    };

    let space_id = app.client.create_space(&space_details).await?;
    let space_response = app.client.get_space(&space_id).await?;
    Ok(space_response)
}

/// Creates a premium conference room space
#[allow(dead_code)]
async fn create_conference_room(
    app: &TestApp,
    site_id: &SiteId,
) -> Result<responses::Space> {
    use payloads::Space;

    let space_details = Space {
        site_id: *site_id,
        name: "Boardroom".to_string(),
        description: Some("12-person boardroom with 4K video conferencing, whiteboard walls, and skyline view".to_string()),
        eligibility_points: 15.0, // High desirability - premium space
        is_available: true,
        site_image_id: None,
    };

    let space_id = app.client.create_space(&space_details).await?;
    let space_response = app.client.get_space(&space_id).await?;
    Ok(space_response)
}

/// Creates an ongoing auction with actual processed rounds following the
/// three-bidders test pattern
///
/// This creates an auction in the past and processes multiple rounds to create
/// realistic auction history for UI testing. The auction start time is
/// calculated based on the round duration and number of rounds to process.
async fn create_ongoing_auction_with_rounds(
    app: &TestApp,
    site_id: &SiteId,
) -> Result<(
    responses::Auction,
    responses::Space,
    responses::Space,
    responses::Space,
)> {
    use payloads::{ActivityRuleParams, Auction, AuctionParams, Space};

    // Configuration: how many rounds to process and duration per round
    let round_duration = Span::new().seconds(15);
    let num_rounds_to_process = -1;

    // Save the real current time
    let real_now = Timestamp::now();

    // Calculate auction start time: far enough in the past to have processed
    // all the rounds we want (round_duration * num_rounds_to_process)
    let auction_start_offset = Span::new().seconds(-15);
    // let auction_start_offset = round_duration
    // .checked_mul(num_rounds_to_process)
    // .expect("round duration multiplication overflow");
    app.time_source.set(real_now - auction_start_offset);

    // Get LA timezone for proper work day calculation
    let auction_start = app.time_source.now();
    let auction_start_la = auction_start.in_tz(TZ)?;

    // Calculate possession for tomorrow (9am-9pm LA time)
    let possession_day = auction_start_la.date() + Span::new().days(1);
    let possession_start_at =
        possession_day.at(9, 0, 0, 0).in_tz(TZ)?.timestamp();
    let possession_end_at =
        possession_day.at(21, 0, 0, 0).in_tz(TZ)?.timestamp();

    // Create auction starting at the calculated past time
    let auction_details = Auction {
        site_id: *site_id,
        possession_start_at,
        possession_end_at,
        start_at: auction_start,
        auction_params: AuctionParams {
            round_duration,
            bid_increment: Decimal::new(100, 2), // $1.00
            activity_rule_params: ActivityRuleParams {
                eligibility_progression: vec![
                    (0, 0.5),
                    (2, 0.55),
                    (3, 0.60),
                    (4, 0.65),
                    (5, 0.70),
                    (6, 0.71),
                    (7, 0.73),
                    (8, 0.74),
                    (10, 0.75),
                    (20, 0.9),
                    (30, 1.0),
                ],
            },
        },
    };

    let auction_id = app.client.create_auction(&auction_details).await?;

    // Create three spaces for the auction
    let space_a_details = Space {
        site_id: *site_id,
        name: "Hot Desk Alpha".to_string(),
        description: Some("Prime desk with window view".to_string()),
        eligibility_points: 8.0,
        is_available: true,
        site_image_id: None,
    };
    let space_a_id = app.client.create_space(&space_a_details).await?;
    let space_a = app.client.get_space(&space_a_id).await?;

    let space_b_details = Space {
        site_id: *site_id,
        name: "Hot Desk Beta".to_string(),
        description: Some("Quiet corner desk".to_string()),
        eligibility_points: 5.0,
        is_available: true,
        site_image_id: None,
    };
    let space_b_id = app.client.create_space(&space_b_details).await?;
    let space_b = app.client.get_space(&space_b_id).await?;

    let space_c_details = Space {
        site_id: *site_id,
        name: "Hot Desk Gamma".to_string(),
        description: Some("Collaboration area desk".to_string()),
        eligibility_points: 10.0,
        is_available: true,
        site_image_id: None,
    };
    let space_c_id = app.client.create_space(&space_c_details).await?;
    let space_c = app.client.get_space(&space_c_id).await?;

    // Set up proxy bidding for Alice: A=5, B=0, max_items=2
    app.login_alice().await?;
    app.client
        .create_or_update_user_value(&payloads::requests::UserValue {
            space_id: space_a.space_id,
            value: Decimal::new(5, 0),
        })
        .await?;
    app.client
        .create_or_update_user_value(&payloads::requests::UserValue {
            space_id: space_b.space_id,
            value: Decimal::new(0, 0),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&payloads::requests::UseProxyBidding {
            auction_id,
            max_items: 2,
        })
        .await?;

    // Set up proxy bidding for Bob: A=4, C=3, max_items=1
    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&payloads::requests::UserValue {
            space_id: space_a.space_id,
            value: Decimal::new(4, 0),
        })
        .await?;
    app.client
        .create_or_update_user_value(&payloads::requests::UserValue {
            space_id: space_c.space_id,
            value: Decimal::new(3, 0),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&payloads::requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    // Set up proxy bidding for Charlie: B=2, C=9, max_items=1
    app.login_charlie().await?;
    app.client
        .create_or_update_user_value(&payloads::requests::UserValue {
            space_id: space_b.space_id,
            value: Decimal::new(2, 0),
        })
        .await?;
    app.client
        .create_or_update_user_value(&payloads::requests::UserValue {
            space_id: space_c.space_id,
            value: Decimal::new(9, 0),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&payloads::requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    tracing::info!(
        "Processing {} auction rounds at {} each...",
        num_rounds_to_process,
        round_duration
    );

    // Process the configured number of rounds
    for round_num in 0..num_rounds_to_process {
        // Create round and process proxy bids
        scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

        // Advance time by round_duration + 1 second
        let current_time = app.time_source.now();
        app.time_source
            .set(current_time + round_duration + Span::new().seconds(1));

        tracing::debug!(
            "Completed round {}/{}",
            1 + round_num,
            num_rounds_to_process
        );
    }

    // Set time back to real current time
    app.time_source.set(real_now);

    // Get the updated auction response
    let auction_response = app.client.get_auction(&auction_id).await?;

    // Switch back to Alice for consistency
    app.login_alice().await?;

    tracing::info!(
        "Ongoing auction created with {} rounds of bidding history",
        num_rounds_to_process
    );

    Ok((auction_response, space_a, space_b, space_c))
}

/// Creates an auction for a work day (9am-9pm) with realistic scheduling
async fn create_work_day_auction(
    app: &TestApp,
    site_id: &SiteId,
    days_from_now: i64, // Days from current time for possession
    auction_type: &str, // "upcoming" or "ongoing"
) -> Result<responses::Auction> {
    use payloads::{ActivityRuleParams, Auction, AuctionParams};

    // Get New York timezone for proper work day calculation
    let now = app.time_source.now();
    let now_la = now.in_tz(TZ)?;

    // Calculate the possession day (N days from now) at 9am LA time
    let possession_day = now_la.date() + Span::new().days(days_from_now);
    let possession_start_at = possession_day
        .at(9, 0, 0, 0) // 9:00:00.000 AM
        .in_tz(TZ)?
        .timestamp();
    let possession_end_at = possession_day
        .at(21, 0, 0, 0) // 9:00:00.000 PM
        .in_tz(TZ)?
        .timestamp();

    // Set auction start time based on type
    let auction_start_at = match auction_type {
        "upcoming" => {
            // Upcoming auction hasn't started yet - starts in 23 hours
            now + Span::new().hours(23)
        }
        "ongoing" => {
            // Ongoing auction started 1 hour ago
            now - Span::new().hours(1)
        }
        _ => now, // Default to now
    };

    let auction_details = Auction {
        site_id: *site_id,
        possession_start_at,
        possession_end_at,
        start_at: auction_start_at,
        auction_params: AuctionParams {
            round_duration: Span::new().minutes(5),
            bid_increment: Decimal::new(250, 2), // $2.50
            activity_rule_params: ActivityRuleParams {
                eligibility_progression: vec![
                    (0, 0.5),
                    (10, 0.75),
                    (20, 0.9),
                    (30, 1.0),
                ],
            },
        },
    };

    let auction_id = app.client.create_auction(&auction_details).await?;
    let auction_response = app.client.get_auction(&auction_id).await?;

    Ok(auction_response)
}

/// Creates Bob's secondary community for testing multi-community scenarios
#[allow(dead_code)]
async fn create_bob_community(app: &TestApp) -> Result<CommunityId> {
    app.create_bob_user().await?;
    app.login_bob().await?;

    let bob_community_body = requests::CreateCommunity {
        name: "Tech Startup Collective".into(),
        currency: payloads::CurrencySettings {
            mode_config: crate::default_currency_config(),
            name: "dollars".into(),
            symbol: "$".into(),
            minor_units: 2,
            balances_visible_to_members: true,
            new_members_default_active: true,
        },
    };
    let bob_community_id =
        app.client.create_community(&bob_community_body).await?;

    Ok(bob_community_id)
}

/// Creates a cross-community invite from Bob to Alice for testing invite flows
async fn create_cross_community_invite(
    app: &TestApp,
    bob_community_id: &CommunityId,
) -> Result<payloads::InviteId> {
    let alice_credentials = crate::alice_credentials();
    let invite_details = requests::InviteCommunityMember {
        community_id: *bob_community_id,
        new_member_email: Some(alice_credentials.email.clone()),
        single_use: false,
    };
    let invite_id = app.client.invite_member(&invite_details).await?;
    Ok(invite_id)
}

// TODO: Future expansion areas for the mock dataset:
//
// 1. Concluded auctions with realistic bid histories
// 2. Proxy bidding scenarios with different demand functions
// 3. Multiple users with varied participation patterns
// 4. Seasonal/time-based variations in space demand
// 5. Different community sizes and activity levels
// 6. Site images and branding variations
// 7. Complex membership schedules and role hierarchies
// 8. Rent redistribution scenarios
// 9. Activity rule edge cases and progressions
// 10. Multi-timezone coordination scenarios
