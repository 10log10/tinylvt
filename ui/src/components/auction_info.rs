use jiff::{fmt::friendly::{Designator, Spacing, SpanPrinter}, Timestamp};
use payloads::responses;
use yew::prelude::*;
use crate::utils::time::{localize_timestamp, format_zoned_timestamp};

#[derive(Debug, Clone, PartialEq)]
enum AuctionStatus {
    Upcoming,
    Ongoing,
    Concluded,
}

impl AuctionStatus {
    fn from_auction(auction: &responses::Auction) -> Self {
        let now = Timestamp::now();
        
        if let Some(_end_at) = auction.end_at {
            Self::Concluded
        } else if now >= auction.auction_details.start_at {
            Self::Ongoing
        } else {
            Self::Upcoming
        }
    }
    
    fn label(&self) -> &'static str {
        match self {
            Self::Upcoming => "Upcoming",
            Self::Ongoing => "Ongoing",
            Self::Concluded => "Concluded",
        }
    }
    
    fn badge_classes(&self) -> &'static str {
        match self {
            Self::Upcoming => "bg-neutral-100 text-neutral-800 dark:bg-neutral-800 dark:text-neutral-200",
            Self::Ongoing => "bg-neutral-800 text-white dark:bg-neutral-200 dark:text-neutral-900",
            Self::Concluded => "bg-neutral-300 text-neutral-600 dark:bg-neutral-600 dark:text-neutral-400",
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct AuctionInfoProps {
    pub auction: responses::Auction,
    pub site: responses::Site,
}


#[function_component]
pub fn AuctionInfo(props: &AuctionInfoProps) -> Html {
    let auction_details = &props.auction.auction_details;
    let site_details = &props.site.site_details;
    let site_timezone = site_details.timezone.as_deref();
    
    // Calculate auction status
    let status = AuctionStatus::from_auction(&props.auction);

    // Format timestamps for display in the appropriate timezone
    let possession_start = format_zoned_timestamp(&localize_timestamp(
        auction_details.possession_start_at,
        site_timezone,
    ));

    let possession_end = format_zoned_timestamp(&localize_timestamp(
        auction_details.possession_end_at,
        site_timezone,
    ));

    let auction_start = format_zoned_timestamp(&localize_timestamp(
        auction_details.start_at,
        site_timezone,
    ));

    let printer = SpanPrinter::new()
        .spacing(Spacing::BetweenUnitsAndDesignators)
        .comma_after_designator(true)
        .designator(Designator::Verbose);
    let round_duration_value =
        printer.span_to_string(&auction_details.auction_params.round_duration);
    let bid_increment_value =
        format!("${}", auction_details.auction_params.bid_increment);

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 rounded-lg p-6 bg-white dark:bg-neutral-800">
            <div class="space-y-4">
                <div class="flex items-center justify-between">
                    <h2 class="text-xl font-semibold text-neutral-900 dark:text-white">
                        {"Auction for "}{&site_details.name}
                    </h2>
                    <span class={format!("px-3 py-1 rounded-full text-xs font-medium {}", status.badge_classes())}>
                        {status.label()}
                    </span>
                </div>

                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <div class="space-y-3">
                        <h3 class="text-sm font-medium text-neutral-700 dark:text-neutral-300 uppercase tracking-wide">
                            {"Possession Period"}
                        </h3>
                        <div class="space-y-1">
                            <div class="text-sm">
                                <span class="text-neutral-900 dark:text-white font-medium">{possession_start}</span>
                                <span class="text-neutral-600 dark:text-neutral-400">{" start"}</span>
                            </div>
                            <div class="text-sm">
                                <span class="text-neutral-900 dark:text-white font-medium">{possession_end}</span>
                            <span class="text-neutral-600 dark:text-neutral-400">{" end"}</span>
                            </div>
                        </div>
                    </div>

                    <div class="space-y-3">
                        <h3 class="text-sm font-medium text-neutral-700 dark:text-neutral-300 uppercase tracking-wide">
                            {"Auction Details"}
                        </h3>
                        <div class="space-y-1">
                            <div class="text-sm">
                                <span class="text-neutral-900 dark:text-white font-medium">{auction_start}</span>
                                <span class="text-neutral-600 dark:text-neutral-400">{" start"}</span>
                            </div>
                            <div class="text-sm">
                                <span class="text-neutral-900 dark:text-white font-medium">{round_duration_value}</span>
                                <span class="text-neutral-600 dark:text-neutral-400">{" per round"}</span>
                            </div>
                            <div class="text-sm">
                                <span class="text-neutral-900 dark:text-white font-medium">{bid_increment_value}</span>
                                <span class="text-neutral-600 dark:text-neutral-400">{" bid increment"}</span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}
