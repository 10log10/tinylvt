//! Chore auction dataset for exercising negative reserve prices.
//!
//! Adds a second community in distributed_clearing mode whose only site is
//! a list of household chores. Each chore has a negative reserve price, and
//! members set negative `user_value` entries representing the compensation
//! they would need to take the chore on. Proxy bidding drives the auction
//! so the member willing to accept the least compensation wins.
//!
//! Assumes Alice/Bob/Charlie already exist (e.g. from
//! [`DeskAllocationScreenshot`]).

use crate::TestApp;
use anyhow::Result;
use api::scheduler;
use jiff::{Span, Timestamp};
use payloads::{
    BidIncrement, CommunityId, ReservePrice, SiteId, requests, responses,
};
use rust_decimal::Decimal;

use super::TZ;
use crate::{BOB, CHARLIE};

/// Dataset that adds a chore-auction community with negative reserves.
pub struct ChoreDataset {
    pub community_id: CommunityId,
    pub site: responses::Site,
    pub auction: responses::Auction,
    pub chores: Vec<responses::Space>,
}

impl ChoreDataset {
    pub async fn create(app: &TestApp) -> Result<Self> {
        app.time_source.set(Timestamp::now());

        tracing::info!("Creating Roommate Co-op community for chore auction");
        let community_id = create_roommate_community(app).await?;

        let site = create_chore_site(app, &community_id).await?;
        let chores = create_chore_spaces(app, &site.site_id).await?;
        let auction = run_chore_auction(app, &site, &chores).await?;

        app.login_alice().await?;

        tracing::info!("Chore dataset created");

        Ok(ChoreDataset {
            community_id,
            site,
            auction,
            chores,
        })
    }

    pub fn print_summary(&self) {
        tracing::info!("Chore Auction Data:");
        tracing::info!(
            "   Community: {} (Distributed clearing)",
            self.community_id
        );
        tracing::info!(
            "   Site: {} ({})",
            self.site.site_details.name,
            self.site.site_id
        );
        tracing::info!("   Auction: {}", self.auction.auction_id);
        tracing::info!("   Chores (negative reserves):");
        for chore in &self.chores {
            tracing::info!(
                "      - {} ({}) reserve {}",
                chore.space_details.name,
                chore.space_id,
                chore.space_details.reserve_price.0,
            );
        }
        tracing::info!("   Members: Alice, Bob, Charlie");
    }
}

/// Create a distributed_clearing community with Alice as leader and Bob and
/// Charlie as members. Assumes all three users already exist.
async fn create_roommate_community(app: &TestApp) -> Result<CommunityId> {
    app.login_alice().await?;

    let community_name = "Roommate Co-op";
    let community_body = requests::CreateCommunity {
        name: community_name.into(),
        description: Some(
            "Shared household where chores are auctioned -- whoever \
             accepts the least compensation takes the task, and the cost \
             is split across the household."
                .into(),
        ),
        currency: payloads::CurrencySettings {
            mode_config: crate::default_currency_config(),
            name: "dollars".into(),
            symbol: "$".into(),
            minor_units: 2,
            balances_visible_to_members: true,
            new_members_default_active: true,
        },
    };
    let community_id = app.client.create_community(&community_body).await?;

    for username in [BOB, CHARLIE] {
        app.time_source.advance(Span::new().seconds(1));
        invite_existing_user(app, &community_id, community_name, username)
            .await?;
    }

    app.login_alice().await?;
    Ok(community_id)
}

/// Invite an already-existing user to the community and accept the invite as
/// that user. Unlike the desk-allocation helper, this does not create the
/// account -- the user must already exist.
async fn invite_existing_user(
    app: &TestApp,
    community_id: &CommunityId,
    community_name: &str,
    username: &str,
) -> Result<()> {
    let creds = crate::credentials(username);
    let login_creds = crate::login_credentials(username);

    app.login_alice().await?;
    let invite = requests::InviteCommunityMember {
        community_id: *community_id,
        new_member_email: Some(creds.email.clone()),
        single_use: false,
    };
    app.client.invite_member(&invite).await?;

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

async fn create_chore_site(
    app: &TestApp,
    community_id: &CommunityId,
) -> Result<responses::Site> {
    use payloads::{ActivityRuleParams, AuctionParams, Site};

    let site_details = Site {
        community_id: *community_id,
        name: "Household Chores".to_string(),
        description: Some(
            "Weekly chore allocation. Each chore opens at a negative \
             reserve (the maximum the household will pay) and bidders \
             compete to accept the least compensation."
                .to_string(),
        ),
        default_auction_params: AuctionParams {
            round_duration: Span::new().seconds(15),
            bid_increment: BidIncrement(Decimal::new(200, 2)), // $2.00
            activity_rule_params: ActivityRuleParams {
                eligibility_progression: vec![(0, 1.0)],
            },
        },
        possession_period: Span::new().days(7), // One week of chores
        auction_lead_time: Span::new().days(2),
        proxy_bidding_lead_time: Span::new().days(1),
        open_hours: None,
        auto_schedule: false,
        timezone: Some(TZ.to_string()),
        site_image_id: None,
    };

    let site_id = app.client.create_site(&site_details).await?;
    Ok(app.client.get_site(&site_id).await?)
}

/// Create the chore spaces. Negative reserves represent the maximum the
/// household is willing to pay a member to take on the chore; bidding pushes
/// the price up (toward zero) as bidders compete to accept less.
async fn create_chore_spaces(
    app: &TestApp,
    site_id: &SiteId,
) -> Result<Vec<responses::Space>> {
    use payloads::Space;

    // Each chore is the week-long commitment to do it at its natural
    // cadence -- so dishes (daily) is worth a lot more compensation than
    // vacuuming (once a week). Reserves are set generously above any
    // expected bid so the price moves up (toward zero) as bidders compete.
    let specs = [
        (
            "Wash dishes daily",
            "Run the dishwasher and put away clean dishes every day for \
             the week",
            -50,
        ),
        (
            "Vacuum living room",
            "Vacuum the common room and hallway once during the week",
            -20,
        ),
        (
            "Take out trash",
            "Empty all bins and roll the cart to the curb on collection \
             days (Tuesday and Friday)",
            -10,
        ),
    ];

    let mut chores = Vec::new();
    for (name, description, reserve) in specs {
        let space_details = Space {
            site_id: *site_id,
            name: name.to_string(),
            description: Some(description.to_string()),
            eligibility_points: 1.0,
            is_available: true,
            site_image_id: None,
            reserve_price: ReservePrice(Decimal::new(reserve, 0)),
        };
        let space_id = app.client.create_space(&space_details).await?;
        chores.push(app.client.get_space(&space_id).await?);
    }

    Ok(chores)
}

/// Create the auction at a calculated past time, configure proxy bidding for
/// each member, and process enough rounds for the price to discover the
/// winner.
async fn run_chore_auction(
    app: &TestApp,
    site: &responses::Site,
    chores: &[responses::Space],
) -> Result<responses::Auction> {
    use payloads::Auction;

    let auction_params = site.site_details.default_auction_params.clone();
    let num_rounds_to_process: i64 = 12;
    let round_duration_secs: i64 = auction_params.round_duration.get_seconds();

    let real_now = Timestamp::now();

    // Position real_now inside the final round with a bit of time remaining,
    // matching the offset pattern from DeskAllocationScreenshot.
    let seconds_remaining: i64 = 8;
    let offset_seconds =
        num_rounds_to_process * round_duration_secs - seconds_remaining;
    let auction_start = real_now - Span::new().seconds(offset_seconds);
    app.time_source.set(auction_start);

    let auction_start_la = auction_start.in_tz(TZ)?;

    // Possession period: the week starting tomorrow.
    let possession_start = auction_start_la.date() + Span::new().days(1);
    let possession_start_at =
        possession_start.at(0, 0, 0, 0).in_tz(TZ)?.timestamp();
    let possession_end_at = (possession_start + Span::new().days(7))
        .at(0, 0, 0, 0)
        .in_tz(TZ)?
        .timestamp();

    let auction_details = Auction {
        site_id: site.site_id,
        possession_start_at,
        possession_end_at,
        start_at: Some(auction_start),
        auction_params,
    };

    app.login_alice().await?;
    let auction_id = app.client.create_auction(&auction_details).await?;

    // Each member sets a negative user_value per chore -- the weekly
    // compensation they'd need to take it on. Surplus = user_value -
    // next_bid, so a value closer to zero than the current price means
    // positive surplus and the member will keep bidding. Members may win
    // multiple chores (max_items=3) since chores aren't mutually exclusive.
    //
    // Weekly compensation each member would require:
    //                    dishes  vacuum  trash
    //   Alice            -28     -15     -4    (doesn't mind trash)
    //   Bob              -22     -10     -7    (would do anything cheap)
    //   Charlie          -30      -8     -6    (hates dishes, ok with rest)

    let dishes = &chores[0];
    let vacuum = &chores[1];
    let trash = &chores[2];

    // Each person is willing to do multiple chores if the compensation exceeds
    // their value.
    let set_proxy_bidding = async || {
        app.client
            .create_or_update_proxy_bidding(&requests::UseProxyBidding {
                auction_id,
                max_items: 3,
            })
            .await
    };

    app.login_alice().await?;
    set_chore_values(
        app,
        &[
            (&dishes.space_id, -28),
            (&vacuum.space_id, -15),
            (&trash.space_id, -4),
        ],
    )
    .await?;
    set_proxy_bidding().await?;

    app.login_bob().await?;
    set_chore_values(
        app,
        &[
            (&dishes.space_id, -22),
            (&vacuum.space_id, -10),
            (&trash.space_id, -7),
        ],
    )
    .await?;
    set_proxy_bidding().await?;

    app.login_charlie().await?;
    set_chore_values(
        app,
        &[
            (&dishes.space_id, -30),
            (&vacuum.space_id, -8),
            (&trash.space_id, -6),
        ],
    )
    .await?;
    set_proxy_bidding().await?;

    tracing::info!(
        "Processing {} chore-auction rounds...",
        num_rounds_to_process
    );

    let round_duration = auction_details.auction_params.round_duration;
    for round_num in 0..num_rounds_to_process {
        scheduler::schedule_tick(&app.db_pool, &app.time_source).await;
        let current_time = app.time_source.now();
        app.time_source.set(current_time + round_duration);
        tracing::debug!(
            "Completed chore round {}/{}",
            1 + round_num,
            num_rounds_to_process
        );
    }

    app.time_source.set(real_now);

    Ok(app.client.get_auction(&auction_id).await?)
}

async fn set_chore_values(
    app: &TestApp,
    values: &[(&payloads::SpaceId, i64)],
) -> Result<()> {
    for (space_id, value) in values {
        app.client
            .create_or_update_user_value(&requests::UserValue {
                space_id: **space_id,
                value: Decimal::new(*value, 0),
            })
            .await?;
    }
    Ok(())
}
