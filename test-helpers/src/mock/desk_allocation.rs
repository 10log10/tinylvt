//! Desk allocation dataset for screenshot automation
//!
//! Creates a points allocation community with:
//! - 5 bidders (Alice, Bob, Charlie, Diana, Eve)
//! - 4 desks with descriptive names
//! - Credits issued to each bidder
//! - Proxy bidding configured for all bidders
//! - Auction progressed to show interesting price discovery

use crate::TestApp;
use anyhow::Result;
use api::scheduler;
use jiff::{Span, Timestamp};
use payloads::{CommunityId, SiteId, requests, responses};
use rust_decimal::Decimal;

use super::TZ;
use crate::{BOB, CHARLIE, DIANA, EVE};

/// Dataset for desk allocation screenshot on landing page
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

        // Issue 250 credits to all active members (representing 2.5 terms of
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
        tracing::info!("Desk Allocation Screenshot Data:");
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
            "   Each bidder has 250 credits (2.5 terms of allowance)"
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
            name: "credits".into(),
            symbol: "C".into(),
            minor_units: 0,
            balances_visible_to_members: true,
            new_members_default_active: true,
        },
    };
    let community_id = app.client.create_community(&community_body).await?;

    // Helper to invite and add a member by username
    async fn add_member(
        app: &TestApp,
        community_id: &CommunityId,
        community_name: &str,
        username: &str,
    ) -> Result<()> {
        let creds = crate::credentials(username);
        let login_creds = crate::login_credentials(username);

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

    for username in [BOB, CHARLIE, DIANA, EVE] {
        app.time_source.advance(Span::new().seconds(1));
        add_member(app, &community_id, community_name, username).await?;
    }

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
            bid_increment: Decimal::new(10, 0), // C10 increments
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
    let num_rounds_to_process: i64 = 20;
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
    let seconds_remaining: i64 = 12;
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
    // require saving across multiple terms (100 credits/term allowance):
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
    // - D1 (Window): Alice wins ~C140 (Bob drops out, spent 1.4 terms worth)
    // - D2 (Quiet): Charlie wins ~C120 (Bob switches to cheaper option)
    // - D3 (Whiteboard): Eve wins ~C65-70 (outbids Diana)
    // - D4 (Entrance): Bob or Diana wins at ~C45-55

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
        (&desks[3], 40),
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
    app.client.logout().await?;
    app.client.login(&crate::login_credentials(DIANA)).await?;
    set_user_values(
        app,
        &desks[2].space_id,
        Decimal::new(60, 0),
        &desks[3].space_id,
        Decimal::new(50, 0),
    )
    .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;

    // Eve: D3=85, D4=35, max_items=1
    app.client.logout().await?;
    app.client.login(&crate::login_credentials(EVE)).await?;
    set_user_values(
        app,
        &desks[2].space_id,
        Decimal::new(80, 0),
        &desks[3].space_id,
        Decimal::new(30, 0),
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
        // Advance time by round_duration
        let current_time = app.time_source.now();
        app.time_source.set(current_time + round_duration);
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
