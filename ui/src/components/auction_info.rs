use jiff::fmt::friendly::{Designator, Spacing, SpanPrinter};
use payloads::{Auction, Site};
use yew::prelude::*;
use crate::utils::time::{localize_timestamp, format_zoned_timestamp};

#[derive(Properties, PartialEq)]
pub struct AuctionInfoProps {
    pub auction: Auction,
    pub site: Site,
}


#[function_component]
pub fn AuctionInfo(props: &AuctionInfoProps) -> Html {
    let auction = &props.auction;
    let site_timezone = props.site.timezone.as_deref();

    // Format timestamps for display in the appropriate timezone
    let possession_start = format_zoned_timestamp(&localize_timestamp(
        auction.possession_start_at,
        site_timezone,
    ));

    let possession_end = format_zoned_timestamp(&localize_timestamp(
        auction.possession_end_at,
        site_timezone,
    ));

    let auction_start = format_zoned_timestamp(&localize_timestamp(
        auction.start_at,
        site_timezone,
    ));

    let printer = SpanPrinter::new()
        .spacing(Spacing::BetweenUnitsAndDesignators)
        .comma_after_designator(true)
        .designator(Designator::Verbose);
    let round_duration_value =
        printer.span_to_string(&auction.auction_params.round_duration);
    let bid_increment_value =
        format!("${}", auction.auction_params.bid_increment);

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 rounded-lg p-6 bg-white dark:bg-neutral-800">
            <div class="space-y-4">
                <div>
                    <h2 class="text-xl font-semibold text-neutral-900 dark:text-white">
                        {"Auction for "}{&props.site.name}
                    </h2>
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
