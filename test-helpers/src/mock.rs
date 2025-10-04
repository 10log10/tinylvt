//! Comprehensive mock data module for TinyLVT testing
//!
//! This module provides realistic test data that can be used across:
//! - Development server (dev-server)
//! - API integration tests
//! - Browser automation tests (ui-tests)
//! - Any other testing scenarios
//!
//! The data is designed to represent real-world usage patterns with:
//! - Multiple communities (Alice's and Bob's)
//! - Different site types (coworking spaces, meeting rooms)
//! - Various auction states (upcoming, ongoing, concluded)
//! - Cross-community interactions (invites, memberships)

use crate::TestApp;
use anyhow::Result;
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
    pub desk_space: responses::Space,
    pub conference_room: responses::Space,
    pub upcoming_auction: responses::Auction,
    pub ongoing_auction: responses::Auction,
}

impl DevDataset {
    /// Creates the complete development dataset with realistic hierarchical data
    pub async fn create(app: &TestApp) -> Result<Self> {
        app.time_source.set(Timestamp::now());

        // === Alice's Community (Primary test user) ===
        tracing::info!("👤 Creating Alice user and community");
        app.create_alice_user().await?;
        let alice_community_id = app.create_test_community().await?;

        // Create multiple sites with different characteristics
        let coworking_site =
            create_coworking_site(app, &alice_community_id).await?;
        let meetup_site = create_meetup_site(app, &alice_community_id).await?;

        // Create spaces in each site
        let desk_space =
            create_desk_space(app, &coworking_site.site_id).await?;
        let conference_room =
            create_conference_room(app, &meetup_site.site_id).await?;

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

        // Ongoing auction (started 1 hour ago, for tomorrow's work day)
        let ongoing_auction = create_work_day_auction(
            app,
            &coworking_site.site_id,
            1, // Tomorrow
            "ongoing",
        )
        .await?;

        // === Bob's Community (Secondary community for multi-community testing) ===
        tracing::info!("👤 Creating Bob user and community");
        let bob_community_id = create_bob_community(app).await?;

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
            desk_space,
            conference_room,
            upcoming_auction,
            ongoing_auction,
        })
    }

    /// Print a summary of the created test data
    pub fn print_summary(&self) {
        tracing::info!("📋 Available test data:");
        tracing::info!(
            "   📊 Alice's Community ({}):",
            self.alice_community_id
        );
        tracing::info!(
            "      - {} ({}): Flexible desk spaces",
            self.coworking_site.site_details.name,
            self.coworking_site.site_id
        );
        tracing::info!(
            "        └─ {} ({}): Hot desk",
            self.desk_space.space_details.name,
            self.desk_space.space_id
        );
        tracing::info!(
            "      - {} ({}): Conference facilities",
            self.meetup_site.site_details.name,
            self.meetup_site.site_id
        );
        tracing::info!(
            "        └─ {} ({}): AV-equipped room",
            self.conference_room.space_details.name,
            self.conference_room.space_id
        );
        tracing::info!("   📊 Bob's Community ({}):", self.bob_community_id);
        tracing::info!("      - Basic community with invite to Alice");
        tracing::info!("   ⏰ Auction States (9am-9pm work days):");
        tracing::info!(
            "      - Upcoming ({}): Starts in ~3 hours, for day after tomorrow",
            self.upcoming_auction.auction_id
        );
        tracing::info!(
            "      - Ongoing ({}): Started ~1 hour ago, for tomorrow's work day",
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
                eligibility_progression: vec![(0, 0.8), (10, 0.9), (20, 1.0)],
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
                eligibility_progression: vec![(1, 1.0), (5, 0.8), (10, 0.6)],
            },
        },
    };

    let auction_id = app.client.create_auction(&auction_details).await?;
    let auction_response = app.client.get_auction(&auction_id).await?;

    Ok(auction_response)
}

/// Creates Bob's secondary community for testing multi-community scenarios
async fn create_bob_community(app: &TestApp) -> Result<CommunityId> {
    app.create_bob_user().await?;
    app.login_bob().await?;

    let bob_community_body = requests::CreateCommunity {
        name: "Tech Startup Collective".into(),
        new_members_default_active: true,
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
