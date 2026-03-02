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
            description: None,
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
    let num_rounds_to_process = 20; // start in the past
    // let num_rounds_to_process = -1; // start in the future

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
        description: None,
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

/// Dataset for desk allocation screenshot on landing page
///
/// Creates a points allocation community with:
/// - 5 bidders (Alice, Bob, Charlie, Diana, Eve)
/// - 4 desks with descriptive names
/// - 100 points issued to each bidder
/// - Proxy bidding configured for all bidders
/// - Auction progressed to show interesting price discovery
pub struct DeskAllocationScreenshot {
    pub community_id: CommunityId,
    pub site: responses::Site,
    pub auction: responses::Auction,
    pub desks: Vec<responses::Space>,
}

impl DeskAllocationScreenshot {
    pub async fn create(app: &TestApp) -> Result<Self> {
        app.time_source.set(Timestamp::now());

        // Create users: Alice (leader), Bob, Charlie, Diana, Eve
        tracing::info!("Creating 5-person desk allocation community");
        let community_id = create_points_allocation_community(app).await?;

        // Create the site
        let site = create_grad_office_site(app, &community_id).await?;

        // Create 4 desks
        let desks = create_grad_office_desks(app, &site.site_id).await?;

        // Issue 250 points to all active members (representing 2.5 terms of
        // savings, so students can compete for premium desks that require
        // saving across multiple terms)
        app.login_alice().await?;
        let issue_request = requests::TreasuryCreditOperation {
            community_id,
            recipient: payloads::TreasuryRecipient::AllActiveMembers,
            amount_per_recipient: Decimal::new(250, 0),
            note: Some("Initial allocation (2.5 terms)".to_string()),
            idempotency_key: payloads::IdempotencyKey(uuid::Uuid::new_v4()),
        };
        app.client.treasury_credit_operation(&issue_request).await?;

        // Create auction and set up proxy bidding
        let auction =
            create_desk_auction_with_bidding(app, &site, &desks).await?;

        // Switch back to Alice
        app.login_alice().await?;

        tracing::info!("Desk allocation screenshot dataset created");

        Ok(DeskAllocationScreenshot {
            community_id,
            site,
            auction,
            desks,
        })
    }

    pub fn print_summary(&self) {
        tracing::info!("📋 Desk Allocation Screenshot Data:");
        tracing::info!(
            "   Community: {} (Points Allocation mode)",
            self.community_id
        );
        tracing::info!(
            "   Site: {} ({})",
            self.site.site_details.name,
            self.site.site_id
        );
        tracing::info!("   Auction: {}", self.auction.auction_id);
        tracing::info!("   Desks:");
        for desk in &self.desks {
            tracing::info!(
                "      - {} ({})",
                desk.space_details.name,
                desk.space_id
            );
        }
        tracing::info!("   Bidders: Alice, Bob, Charlie, Diana, Eve");
        tracing::info!(
            "   Each bidder has 250 points (2.5 terms of allowance)"
        );
    }
}

/// Creates a 5-person community in points allocation mode
async fn create_points_allocation_community(
    app: &TestApp,
) -> Result<CommunityId> {
    // Create Alice (leader)
    let alice_creds = crate::alice_credentials();
    app.client.create_account(&alice_creds).await?;
    app.mark_user_email_verified(&alice_creds.username).await?;
    app.client.login(&crate::alice_login_credentials()).await?;

    // Create community with points allocation
    let community_body = requests::CreateCommunity {
        name: "Economics Department".into(),
        description: Some(
            "Graduate student desk allocation for Fall 2026".into(),
        ),
        currency: payloads::CurrencySettings {
            mode_config: payloads::CurrencyModeConfig::PointsAllocation(
                Box::new(payloads::PointsAllocationConfig {
                    allowance_amount: Decimal::new(100, 0),
                    allowance_period: Span::new().days(90), // Quarterly
                    allowance_start: app.time_source.now(),
                }),
            ),
            name: "points".into(),
            symbol: "P".into(),
            minor_units: 0,
            balances_visible_to_members: true,
            new_members_default_active: true,
        },
    };
    let community_id = app.client.create_community(&community_body).await?;

    // Helper to invite and add a member
    async fn add_member(
        app: &TestApp,
        community_id: &CommunityId,
        community_name: &str,
        creds: requests::CreateAccount,
        login_creds: requests::LoginCredentials,
    ) -> Result<()> {
        // Invite from Alice
        app.login_alice().await?;
        let invite = requests::InviteCommunityMember {
            community_id: *community_id,
            new_member_email: Some(creds.email.clone()),
            single_use: false,
        };
        app.client.invite_member(&invite).await?;

        // Create account and accept
        app.client.create_account(&creds).await?;
        app.mark_user_email_verified(&creds.username).await?;
        app.client.logout().await?;
        app.client.login(&login_creds).await?;
        let invites = app.client.get_received_invites().await?;
        let invite = invites
            .iter()
            .find(|i| i.community_name == community_name)
            .unwrap();
        app.client.accept_invite(&invite.id).await?;
        Ok(())
    }

    let community_name = "Economics Department";

    app.time_source.advance(Span::new().seconds(1));

    // Add Bob
    add_member(
        app,
        &community_id,
        community_name,
        crate::bob_credentials(),
        crate::bob_login_credentials(),
    )
    .await?;

    app.time_source.advance(Span::new().seconds(1));

    // Add Charlie
    add_member(
        app,
        &community_id,
        community_name,
        crate::charlie_credentials(),
        crate::charlie_login_credentials(),
    )
    .await?;

    app.time_source.advance(Span::new().seconds(1));

    // Add Diana
    let diana_creds = requests::CreateAccount {
        username: "diana".into(),
        password: "dianapw".into(),
        email: "diana@example.com".into(),
    };
    let diana_login = requests::LoginCredentials {
        username: diana_creds.username.clone(),
        password: diana_creds.password.clone(),
    };
    add_member(app, &community_id, community_name, diana_creds, diana_login)
        .await?;

    app.time_source.advance(Span::new().seconds(1));

    // Add Eve
    let eve_creds = requests::CreateAccount {
        username: "eve".into(),
        password: "evepw".into(),
        email: "eve@example.com".into(),
    };
    let eve_login = requests::LoginCredentials {
        username: eve_creds.username.clone(),
        password: eve_creds.password.clone(),
    };
    add_member(app, &community_id, community_name, eve_creds, eve_login)
        .await?;

    app.login_alice().await?;
    Ok(community_id)
}

/// Creates the grad student office site
async fn create_grad_office_site(
    app: &TestApp,
    community_id: &CommunityId,
) -> Result<responses::Site> {
    use payloads::{ActivityRuleParams, AuctionParams, Site};

    let site_details = Site {
        community_id: *community_id,
        name: "Graduate Student Office".to_string(),
        description: Some(
            "Shared office space for economics PhD students.\n\
            Desks are allocated each term via auction."
                .to_string(),
        ),
        default_auction_params: AuctionParams {
            round_duration: Span::new().seconds(15),
            bid_increment: Decimal::new(10, 0), // P10 increments
            activity_rule_params: ActivityRuleParams {
                // 100% eligibility required from round 0 - bidders can freely
                // move between desks since they're substitutes, but must
                // participate every round.
                eligibility_progression: vec![(0, 1.0)],
            },
        },
        possession_period: Span::new().days(90), // One term
        auction_lead_time: Span::new().days(7),
        proxy_bidding_lead_time: Span::new().days(3),
        open_hours: None,
        auto_schedule: false,
        timezone: Some(TZ.to_string()),
        site_image_id: None,
    };

    let site_id = app.client.create_site(&site_details).await?;
    let site_response = app.client.get_site(&site_id).await?;
    Ok(site_response)
}

/// Creates 4 desks with varied desirability
async fn create_grad_office_desks(
    app: &TestApp,
    site_id: &SiteId,
) -> Result<Vec<responses::Space>> {
    use payloads::Space;

    // All desks have 1 eligibility point since they're substitutes - bidders
    // can freely move between them while maintaining participation.
    let desk_specs = [
        (
            "Desk 1 — Window",
            "Corner desk with natural light and city view",
        ),
        (
            "Desk 2 — Quiet corner",
            "Away from door, minimal foot traffic",
        ),
        ("Desk 3 — Near whiteboard", "Good for collaboration"),
        ("Desk 4 — By entrance", "Easy access, more noise"),
    ];

    let mut desks = Vec::new();
    for (name, description) in desk_specs {
        let space_details = Space {
            site_id: *site_id,
            name: name.to_string(),
            description: Some(description.to_string()),
            eligibility_points: 1.0,
            is_available: true,
            site_image_id: None,
        };
        let space_id = app.client.create_space(&space_details).await?;
        let space = app.client.get_space(&space_id).await?;
        desks.push(space);
    }

    Ok(desks)
}

/// Creates auction with proxy bidding for all 5 bidders
async fn create_desk_auction_with_bidding(
    app: &TestApp,
    site: &responses::Site,
    desks: &[responses::Space],
) -> Result<responses::Auction> {
    use payloads::Auction;

    // Use the site's default auction params as the single source of truth
    let auction_params = site.site_details.default_auction_params.clone();

    // Configuration for mock data timing
    let num_rounds_to_process: i64 = 12;
    let round_duration_secs: i64 = auction_params.round_duration.get_seconds();

    let real_now = Timestamp::now();

    // After N schedule_tick calls, round (N-1) exists with:
    //   start_at = auction_start + (N-1) * round_duration
    //   end_at = auction_start + N * round_duration
    //
    // We want real_now to fall within round (N-1), so:
    //   auction_start + (N-1) * round_duration < real_now < auction_start + N * round_duration
    //
    // To have ~10 seconds remaining:
    //   real_now = auction_start + N * round_duration - 10
    //   auction_start = real_now - N * round_duration + 10
    let seconds_remaining: i64 = 10;
    let offset_seconds =
        num_rounds_to_process * round_duration_secs - seconds_remaining;
    let auction_start = real_now - Span::new().seconds(offset_seconds);
    app.time_source.set(auction_start);

    let auction_start_la = auction_start.in_tz(TZ)?;

    // Possession period: next term (90 days starting in 2 weeks)
    let possession_start = auction_start_la.date() + Span::new().days(14);
    let possession_start_at =
        possession_start.at(0, 0, 0, 0).in_tz(TZ)?.timestamp();
    let possession_end_at = (possession_start + Span::new().days(90))
        .at(0, 0, 0, 0)
        .in_tz(TZ)?
        .timestamp();

    let auction_details = Auction {
        site_id: site.site_id,
        possession_start_at,
        possession_end_at,
        start_at: auction_start,
        auction_params,
    };

    app.login_alice().await?;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Bidder values designed for scarcity dynamics where premium desks
    // require saving across multiple terms (100 pts/term allowance):
    //
    // Desk 1 (Window):     Premium, worth 1.5-1.8 terms of saving
    // Desk 2 (Quiet):      Good, worth ~1 term of saving
    // Desk 3 (Whiteboard): Moderate, affordable each term
    // Desk 4 (Entrance):   Budget, always affordable
    //
    // - Alice: Saved up for window desk (D1=180), fallback quiet (D2=110)
    // - Bob: Flexible, values all but won't overpay (D1=140, D2=120, D3=70, D4=45)
    // - Charlie: Really wants quiet corner (D2=150), fallback window (D1=130)
    // - Diana: Budget conscious, prefers cheaper desks (D3=65, D4=55)
    // - Eve: Collaboration focused (D3=85, D4=35)
    //
    // Expected outcome after bidding:
    // - D1 (Window): Alice wins ~P140 (Bob drops out, spent 1.4 terms worth)
    // - D2 (Quiet): Charlie wins ~P120 (Bob switches to cheaper option)
    // - D3 (Whiteboard): Eve wins ~P65-70 (outbids Diana)
    // - D4 (Entrance): Bob or Diana wins at ~P45-55

    // Alice: D1=180, D2=110, max_items=1
    app.login_alice().await?;
    set_user_values(
        app,
        &desks[0].space_id,
        Decimal::new(180, 0),
        &desks[1].space_id,
        Decimal::new(110, 0),
    )
    .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    // Bob: D1=140, D2=120, D3=70, D4=45, max_items=1
    app.login_bob().await?;
    for (desk, value) in [
        (&desks[0], 140),
        (&desks[1], 120),
        (&desks[2], 70),
        (&desks[3], 45),
    ] {
        app.client
            .create_or_update_user_value(&requests::UserValue {
                space_id: desk.space_id,
                value: Decimal::new(value, 0),
            })
            .await?;
    }
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    // Charlie: D1=130, D2=150, max_items=1
    app.login_charlie().await?;
    set_user_values(
        app,
        &desks[0].space_id,
        Decimal::new(130, 0),
        &desks[1].space_id,
        Decimal::new(150, 0),
    )
    .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    // Diana: D3=65, D4=55, max_items=1
    let diana_login = requests::LoginCredentials {
        username: "diana".into(),
        password: "dianapw".into(),
    };
    app.client.logout().await?;
    app.client.login(&diana_login).await?;
    set_user_values(
        app,
        &desks[2].space_id,
        Decimal::new(65, 0),
        &desks[3].space_id,
        Decimal::new(55, 0),
    )
    .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    // Eve: D3=85, D4=35, max_items=1
    let eve_login = requests::LoginCredentials {
        username: "eve".into(),
        password: "evepw".into(),
    };
    app.client.logout().await?;
    app.client.login(&eve_login).await?;
    set_user_values(
        app,
        &desks[2].space_id,
        Decimal::new(85, 0),
        &desks[3].space_id,
        Decimal::new(35, 0),
    )
    .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    tracing::info!("Processing {} auction rounds...", num_rounds_to_process);

    // Process rounds
    let round_duration = auction_details.auction_params.round_duration;
    for round_num in 0..num_rounds_to_process {
        scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;
        let current_time = app.time_source.now();
        app.time_source
            .set(current_time + round_duration + Span::new().seconds(1));
        tracing::debug!(
            "Completed round {}/{}",
            1 + round_num,
            num_rounds_to_process
        );
    }

    // Reset to real time
    app.time_source.set(real_now);

    let auction_response = app.client.get_auction(&auction_id).await?;
    Ok(auction_response)
}

/// Helper to set two user values
async fn set_user_values(
    app: &TestApp,
    space1: &payloads::SpaceId,
    value1: Decimal,
    space2: &payloads::SpaceId,
    value2: Decimal,
) -> Result<()> {
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: *space1,
            value: value1,
        })
        .await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: *space2,
            value: value2,
        })
        .await?;
    Ok(())
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
