use payloads::AuctionId;
use yew::prelude::*;

use crate::{
    components::{
        AuctionTabHeader, TimestampDisplay, auction_tab_header::ActiveTab,
    },
    hooks::{use_auction_detail, use_auction_rounds},
};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub auction_id: AuctionId,
}

#[function_component]
pub fn AuctionRoundsPage(props: &Props) -> Html {
    let auction_hook = use_auction_detail(props.auction_id);
    let rounds_hook = use_auction_rounds(props.auction_id);

    // Handle auction loading state
    if auction_hook.is_loading {
        return html! {
            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400">
                    {"Loading auction..."}
                </p>
            </div>
        };
    }

    // Handle auction error
    if let Some(error) = &auction_hook.error {
        return html! {
            <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 \
                        border border-red-200 dark:border-red-800">
                <p class="text-sm text-red-700 dark:text-red-400">
                    {format!("Error loading auction: {}", error)}
                </p>
            </div>
        };
    }

    // Get auction data
    let Some(auction) = &auction_hook.auction else {
        return html! {
            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400">
                    {"No auction found"}
                </p>
            </div>
        };
    };

    html! {
        <div>
            <AuctionTabHeader
                auction={auction.clone()}
                active_tab={ActiveTab::Rounds}
            />
            <div class="py-6">
                <RoundsContent
                    auction={auction.clone()}
                    rounds={rounds_hook.rounds.clone()}
                    is_loading={rounds_hook.is_loading}
                    error={rounds_hook.error.clone()}
                />
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct RoundsContentProps {
    auction: payloads::responses::Auction,
    rounds: Option<Vec<payloads::responses::AuctionRound>>,
    is_loading: bool,
    error: Option<String>,
}

#[function_component]
fn RoundsContent(props: &RoundsContentProps) -> Html {
    let rounds = &props.rounds;
    let is_loading = props.is_loading;
    let error = &props.error;

    if is_loading {
        return html! {
            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400">
                    {"Loading rounds..."}
                </p>
            </div>
        };
    }

    if let Some(err) = error {
        return html! {
            <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 \
                        border border-red-200 dark:border-red-800">
                <p class="text-sm text-red-700 dark:text-red-400">
                    {format!("Error loading rounds: {}", err)}
                </p>
            </div>
        };
    }

    match rounds {
        Some(rounds) => {
            if rounds.is_empty() {
                html! {
                    <div class="text-center py-12">
                        <p class="text-neutral-600 dark:text-neutral-400">
                            {"This auction has not started yet. No rounds have been \
                             created."}
                        </p>
                    </div>
                }
            } else {
                // Sort rounds by round_num (descending, most recent first)
                let mut sorted_rounds = rounds.clone();
                sorted_rounds.sort_by(|a, b| {
                    b.round_details.round_num.cmp(&a.round_details.round_num)
                });

                html! {
                    <div>
                        <h2 class="text-xl font-semibold text-neutral-900 \
                                   dark:text-neutral-100 mb-6">
                            {"Auction Rounds"}
                        </h2>
                        <div class="space-y-4">
                            {sorted_rounds.iter().map(|round| {
                                html! {
                                    <RoundCard
                                        key={round.round_id.0.to_string()}
                                        round={round.clone()}
                                    />
                                }
                            }).collect::<Html>()}
                        </div>
                    </div>
                }
            }
        }
        None => {
            html! {
                <div class="text-center py-12">
                    <p class="text-neutral-600 dark:text-neutral-400">
                        {"No rounds data available"}
                    </p>
                </div>
            }
        }
    }
}

#[derive(Properties, PartialEq)]
struct RoundCardProps {
    round: payloads::responses::AuctionRound,
}

#[function_component]
fn RoundCard(props: &RoundCardProps) -> Html {
    let round = &props.round;

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 rounded-lg p-6 hover:shadow-md transition-shadow">
            <div class="grid grid-cols-2 gap-4">
                <div>
                    <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                        {format!("Round {}", round.round_details.round_num)}
                    </h3>
                    <div class="space-y-2">
                        <div>
                            <span class="text-sm text-neutral-600 dark:text-neutral-400">
                                {"Started: "}
                            </span>
                            <TimestampDisplay
                                timestamp={round.round_details.start_at}
                                site_timezone={Option::<String>::None}
                            />
                        </div>
                        <div>
                            <span class="text-sm text-neutral-600 dark:text-neutral-400">
                                {"Ended: "}
                            </span>
                            <TimestampDisplay
                                timestamp={round.round_details.end_at}
                                site_timezone={Option::<String>::None}
                            />
                        </div>
                    </div>
                </div>
                <div>
                    <div class="space-y-2">
                        <div>
                            <span class="text-sm font-medium text-neutral-700 dark:text-neutral-300">
                                {"Eligibility Threshold"}
                            </span>
                            <p class="text-sm text-neutral-600 dark:text-neutral-400">
                                {format!("{:.2}", round.round_details.eligibility_threshold)}
                            </p>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}
