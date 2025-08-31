use crate::components::AuctionInfo;
use jiff::{Span, Timestamp};
use payloads::{
    ActivityRuleParams, Auction, AuctionParams, CommunityId, Site, SiteId,
};
use rust_decimal::Decimal;
use uuid::Uuid;
use yew::prelude::*;

#[function_component]
pub fn AuctionInfoTest() -> Html {
    // Create test auction data
    let now = Timestamp::now();
    let test_auction = Auction {
        site_id: SiteId(Uuid::new_v4()),
        possession_start_at: now + Span::new().hours(24 + 2), // 1 day = 24 hours
        possession_end_at: now + Span::new().hours(192 + 2), // 8 days = 192 hours
        start_at: now + Span::new().hours(2),
        auction_params: AuctionParams {
            round_duration: Span::new().minutes(5),
            bid_increment: Decimal::new(250, 2), // $5.00
            activity_rule_params: ActivityRuleParams {
                eligibility_progression: vec![(1, 1.0), (5, 0.8), (10, 0.6)],
            },
        },
    };

    // Create test site data
    let test_site = Site {
        community_id: CommunityId(Uuid::new_v4()),
        name: "Downtown Coworking - All Desks".to_string(),
        description: Some("Premium desk in downtown location".to_string()),
        default_auction_params: AuctionParams {
            round_duration: Span::new().minutes(10),
            bid_increment: Decimal::new(250, 2), // $2.50
            activity_rule_params: ActivityRuleParams {
                eligibility_progression: vec![(1, 1.0), (3, 0.9)],
            },
        },
        possession_period: Span::new().hours(168), // 7 days = 168 hours
        auction_lead_time: Span::new().hours(24),
        proxy_bidding_lead_time: Span::new().hours(12),
        open_hours: None,
        auto_schedule: true,
        timezone: Some("America/New_York".to_string()),
        site_image_id: None,
    };

    html! {
        <div class="space-y-6">
            <div>
                <h1 class="text-2xl font-bold text-neutral-900 dark:text-white">
                    {"Auction Component Test"}
                </h1>
                <p class="text-neutral-600 dark:text-neutral-400 mt-2">
                    {"Testing the AuctionInfo component with sample data"}
                </p>
            </div>

            <AuctionInfo auction={test_auction} site={test_site} />

            <div class="text-sm text-neutral-500 dark:text-neutral-400">
                <p>{"This auction data is generated for testing purposes."}</p>
                <p>{"The component displays key auction information in a clean, neutral design."}</p>
            </div>
        </div>
    }
}
