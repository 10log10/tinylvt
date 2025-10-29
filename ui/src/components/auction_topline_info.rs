use crate::components::{AuctionParamsViewer, TimestampDisplay};
use jiff::Timestamp;
use payloads::responses;
use yew::prelude::*;

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
            Self::Upcoming => {
                "bg-neutral-100 text-neutral-800 dark:bg-neutral-800 \
                 dark:text-neutral-200"
            }
            Self::Ongoing => {
                "bg-neutral-800 text-white dark:bg-neutral-200 \
                 dark:text-neutral-900"
            }
            Self::Concluded => {
                "bg-neutral-300 text-neutral-600 dark:bg-neutral-600 \
                 dark:text-neutral-400"
            }
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub auction: responses::Auction,
    pub site_timezone: Option<String>,
}

#[function_component]
pub fn AuctionToplineInfo(props: &Props) -> Html {
    let auction_details = &props.auction.auction_details;
    let status = AuctionStatus::from_auction(&props.auction);
    let params_expanded = use_state(|| false);

    let toggle_params = {
        let params_expanded = params_expanded.clone();
        Callback::from(move |_| {
            params_expanded.set(!*params_expanded);
        })
    };

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-6 bg-white dark:bg-neutral-800">
            <div class="space-y-4">
                <div class="flex items-center justify-between">
                    <h2 class="text-xl font-semibold text-neutral-900 \
                               dark:text-white">
                        {"Auction Details"}
                    </h2>
                    <span class={format!(
                        "px-3 py-1 rounded-full text-xs font-medium {}",
                        status.badge_classes()
                    )}>
                        {status.label()}
                    </span>
                </div>

                <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                    // Possession Period
                    <div class="space-y-2">
                        <h3 class="text-sm font-medium text-neutral-700 \
                                   dark:text-neutral-300 uppercase \
                                   tracking-wide">
                            {"Possession Period"}
                        </h3>
                        <div class="space-y-1">
                            <div class="text-sm">
                                <span class="text-neutral-600 \
                                             dark:text-neutral-400">
                                    {"Start: "}
                                </span>
                                <TimestampDisplay
                                    timestamp={auction_details.possession_start_at}
                                    site_timezone={props.site_timezone.clone()}
                                />
                            </div>
                            <div class="text-sm">
                                <span class="text-neutral-600 \
                                             dark:text-neutral-400">
                                    {"End: "}
                                </span>
                                <TimestampDisplay
                                    timestamp={auction_details.possession_end_at}
                                    site_timezone={props.site_timezone.clone()}
                                />
                            </div>
                        </div>
                    </div>

                    // Auction Times
                    <div class="space-y-2">
                        <h3 class="text-sm font-medium text-neutral-700 \
                                   dark:text-neutral-300 uppercase \
                                   tracking-wide">
                            {"Auction Times"}
                        </h3>
                        <div class="space-y-1">
                            <div class="text-sm">
                                <span class="text-neutral-600 \
                                             dark:text-neutral-400">
                                    {"Start: "}
                                </span>
                                <TimestampDisplay
                                    timestamp={auction_details.start_at}
                                />
                            </div>
                            {if let Some(end_at) = props.auction.end_at {
                                html! {
                                    <div class="text-sm">
                                        <span class="text-neutral-600 \
                                                     dark:text-neutral-400">
                                            {"End: "}
                                        </span>
                                        <TimestampDisplay
                                            timestamp={end_at}
                                        />
                                    </div>
                                }
                            } else {
                                html! {}
                            }}
                        </div>
                    </div>
                </div>

                // Auction Parameters (Collapsible)
                <div class="pt-4 border-t border-neutral-200 \
                            dark:border-neutral-700">
                    <button
                        onclick={toggle_params}
                        class="w-full flex items-center justify-between \
                               text-left mb-3 hover:opacity-70 transition-opacity"
                    >
                        <h3 class="text-sm font-medium text-neutral-700 \
                                   dark:text-neutral-300 uppercase tracking-wide">
                            {"Auction Parameters"}
                        </h3>
                        <svg
                            class={format!(
                                "w-4 h-4 text-neutral-700 dark:text-neutral-300 \
                                 transition-transform {}",
                                if *params_expanded { "rotate-180" } else { "" }
                            )}
                            fill="none"
                            stroke="currentColor"
                            viewBox="0 0 24 24"
                        >
                            <path
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                stroke-width="2"
                                d="M19 9l-7 7-7-7"
                            />
                        </svg>
                    </button>
                    {if *params_expanded {
                        html! {
                            <AuctionParamsViewer
                                auction_params={
                                    auction_details.auction_params.clone()
                                }
                            />
                        }
                    } else {
                        html! {}
                    }}
                </div>
            </div>
        </div>
    }
}
