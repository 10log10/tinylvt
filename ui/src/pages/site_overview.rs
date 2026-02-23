use jiff::Timestamp;
use payloads::{SiteId, responses};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::{
    Route,
    components::{
        MarkdownText, SitePageWrapper, SiteTabHeader, SiteWithRole,
        TimestampDisplay, site_tab_header::ActiveTab,
    },
    hooks::use_auctions,
};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub site_id: SiteId,
}

#[function_component]
pub fn SiteOverviewPage(props: &Props) -> Html {
    let render_content = Callback::from(|site_with_role: SiteWithRole| {
        html! {
            <div>
                <SiteTabHeader
                    site={site_with_role.site.clone()}
                    active_tab={ActiveTab::Overview}
                />
                <div class="py-6">
                    <SiteOverviewContent site={site_with_role.site} />
                </div>
            </div>
        }
    });

    html! {
        <SitePageWrapper
            site_id={props.site_id}
            children={render_content}
        />
    }
}

#[derive(Properties, PartialEq)]
pub struct SiteOverviewContentProps {
    pub site: responses::Site,
}

#[function_component]
fn SiteOverviewContent(props: &SiteOverviewContentProps) -> Html {
    let auctions_hook = use_auctions(props.site.site_id);
    let site_timezone = props.site.site_details.timezone.clone();

    // Find ongoing, recent completed, and next upcoming auctions
    let (ongoing_auctions, recent_auction, upcoming_auction) = auctions_hook
        .data
        .as_ref()
        .map(|auctions| {
            let now = Timestamp::now();

            // Ongoing: start_at <= now AND end_at is None
            let ongoing: Vec<_> = auctions
                .iter()
                .filter(|a| {
                    a.end_at.is_none() && a.auction_details.start_at <= now
                })
                .cloned()
                .collect();

            // Recent: end_at is Some, sort by end_at desc, take first
            let recent = auctions
                .iter()
                .filter(|a| a.end_at.is_some())
                .max_by_key(|a| a.end_at);

            // Upcoming: end_at is None AND start_at > now, sort by start_at
            // asc, take first
            let upcoming = auctions
                .iter()
                .filter(|a| {
                    a.end_at.is_none() && a.auction_details.start_at > now
                })
                .min_by_key(|a| a.auction_details.start_at);

            (ongoing, recent.cloned(), upcoming.cloned())
        })
        .unwrap_or((Vec::new(), None, None));

    html! {
        <div class="space-y-8">
            // Ongoing Auctions Section
            {if !ongoing_auctions.is_empty() {
                html! {
                    <OngoingAuctionsSection
                        auctions={ongoing_auctions}
                        site_timezone={site_timezone.clone()}
                    />
                }
            } else {
                html! {}
            }}

            // Auction Highlights Section
            <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                // Recent Completed Auction
                <AuctionHighlightCard
                    title="Recently Completed"
                    auction={recent_auction}
                    site_timezone={site_timezone.clone()}
                    empty_message="No completed auctions yet"
                />

                // Next Upcoming Auction
                <AuctionHighlightCard
                    title="Next Upcoming"
                    auction={upcoming_auction}
                    site_timezone={site_timezone.clone()}
                    empty_message="No upcoming auctions scheduled"
                />
            </div>

            // Site Description Section (display only, editing in Settings)
            {if let Some(description) = &props.site.site_details.description {
                html! {
                    <div class="bg-white dark:bg-neutral-800 p-6 rounded-lg shadow-md
                                border border-neutral-200 dark:border-neutral-700">
                        <MarkdownText text={description.clone()} />
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct AuctionHighlightCardProps {
    title: &'static str,
    auction: Option<responses::Auction>,
    site_timezone: Option<String>,
    empty_message: &'static str,
}

#[function_component]
fn AuctionHighlightCard(props: &AuctionHighlightCardProps) -> Html {
    html! {
        <div class="bg-white dark:bg-neutral-800 p-6 rounded-lg shadow-md
                    border border-neutral-200 dark:border-neutral-700">
            <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100
                       mb-4">
                {props.title}
            </h3>

            {match &props.auction {
                Some(auction) => {
                    let auction_details = &auction.auction_details;
                    let site_timezone = props.site_timezone.clone();

                    html! {
                        <Link<Route>
                            to={Route::AuctionDetail { id: auction.auction_id }}
                            classes="block hover:bg-neutral-50 dark:hover:bg-neutral-700/50
                                     -m-3 p-3 rounded-lg transition-colors"
                        >
                            <div class="space-y-3 text-sm">
                                <div>
                                    <span class="font-medium text-neutral-700
                                                 dark:text-neutral-300">
                                        {"Possession Period: "}
                                    </span>
                                    <span class="text-neutral-600
                                                 dark:text-neutral-400">
                                        <TimestampDisplay
                                            timestamp={
                                                auction_details.possession_start_at
                                            }
                                            site_timezone={site_timezone.clone()}
                                        />
                                        {" — "}
                                        <TimestampDisplay
                                            timestamp={
                                                auction_details.possession_end_at
                                            }
                                            site_timezone={site_timezone.clone()}
                                        />
                                    </span>
                                </div>

                                <div>
                                    <span class="font-medium text-neutral-700
                                                 dark:text-neutral-300">
                                        {"Auction: "}
                                    </span>
                                    <span class="text-neutral-600
                                                 dark:text-neutral-400">
                                        {if let Some(end_at) = auction.end_at {
                                            html! {
                                                <>
                                                    {"Ended "}
                                                    <TimestampDisplay
                                                        timestamp={end_at}
                                                    />
                                                </>
                                            }
                                        } else {
                                            html! {
                                                <>
                                                    {"Starts "}
                                                    <TimestampDisplay
                                                        timestamp={
                                                            auction_details.start_at
                                                        }
                                                    />
                                                </>
                                            }
                                        }}
                                    </span>
                                </div>

                                <p class="text-neutral-500 dark:text-neutral-500
                                          text-xs">
                                    {"Click to view details →"}
                                </p>
                            </div>
                        </Link<Route>>
                    }
                }
                None => {
                    html! {
                        <p class="text-neutral-500 dark:text-neutral-400 text-sm">
                            {props.empty_message}
                        </p>
                    }
                }
            }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct OngoingAuctionsSectionProps {
    auctions: Vec<responses::Auction>,
    site_timezone: Option<String>,
}

#[function_component]
fn OngoingAuctionsSection(props: &OngoingAuctionsSectionProps) -> Html {
    html! {
        <div class="space-y-4">
            <div class="flex items-center gap-3">
                <h2 class="text-xl font-semibold text-neutral-900
                           dark:text-neutral-100">
                    {"Ongoing Auctions"}
                </h2>
                <span class="inline-flex items-center px-2.5 py-0.5 rounded-full
                             text-xs font-medium bg-green-100 text-green-800
                             dark:bg-green-900/30 dark:text-green-400">
                    {"Live"}
                </span>
            </div>

            <div class="grid grid-cols-1 gap-4">
                {props.auctions.iter().map(|auction| {
                    html! {
                        <OngoingAuctionCard
                            auction={auction.clone()}
                            site_timezone={props.site_timezone.clone()}
                        />
                    }
                }).collect::<Html>()}
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct OngoingAuctionCardProps {
    auction: responses::Auction,
    site_timezone: Option<String>,
}

#[function_component]
fn OngoingAuctionCard(props: &OngoingAuctionCardProps) -> Html {
    let auction = &props.auction;
    let auction_details = &auction.auction_details;
    let site_timezone = props.site_timezone.clone();

    html! {
        <Link<Route>
            to={Route::AuctionDetail { id: auction.auction_id }}
            classes="block bg-white dark:bg-neutral-800 p-5 rounded-lg shadow-md
                     border-2 border-green-200 dark:border-green-800
                     hover:border-green-300 dark:hover:border-green-700
                     transition-colors"
        >
            <div class="flex flex-col sm:flex-row sm:items-center
                        sm:justify-between gap-4">
                <div class="space-y-2">
                    <div class="flex items-center gap-2">
                        <span class="inline-flex items-center px-2 py-0.5
                                     rounded text-xs font-medium bg-green-100
                                     text-green-800 dark:bg-green-900/30
                                     dark:text-green-400">
                            {"Ongoing"}
                        </span>
                    </div>

                    <div class="text-sm">
                        <span class="font-medium text-neutral-700
                                     dark:text-neutral-300">
                            {"Possession Period: "}
                        </span>
                        <span class="text-neutral-600 dark:text-neutral-400">
                            <TimestampDisplay
                                timestamp={auction_details.possession_start_at}
                                site_timezone={site_timezone.clone()}
                            />
                            {" — "}
                            <TimestampDisplay
                                timestamp={auction_details.possession_end_at}
                                site_timezone={site_timezone.clone()}
                            />
                        </span>
                    </div>

                    <div class="text-sm">
                        <span class="font-medium text-neutral-700
                                     dark:text-neutral-300">
                            {"Started: "}
                        </span>
                        <span class="text-neutral-600 dark:text-neutral-400">
                            <TimestampDisplay
                                timestamp={auction_details.start_at}
                            />
                        </span>
                    </div>
                </div>

                <div class="flex items-center text-sm text-neutral-500
                            dark:text-neutral-400">
                    {"View auction →"}
                </div>
            </div>
        </Link<Route>>
    }
}
