use payloads::AuctionId;
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{
    State,
    components::{
        AuctionTabHeader, TimestampDisplay, auction_tab_header::ActiveTab,
    },
    hooks::{
        use_auction_detail, use_auction_round_results, use_auction_rounds,
        use_auction_user_bids, use_spaces,
    },
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
    let auction_id = props.auction.auction_id;
    let site_id = props.auction.auction_details.site_id;
    let (state, _) = use_store::<State>();

    // Get current user's username
    let current_username = match &state.auth_state {
        crate::state::AuthState::LoggedIn(profile) => {
            Some(profile.username.clone())
        }
        _ => None,
    };

    // Fetch all user bids and round results for the auction
    let user_bids_hook = use_auction_user_bids(auction_id, rounds.clone());
    let round_results_hook =
        use_auction_round_results(auction_id, rounds.clone());
    let spaces_hook = use_spaces(site_id);

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

                let bids_by_round = user_bids_hook.bids_by_round.clone();
                let results_by_round =
                    round_results_hook.results_by_round.clone();
                let spaces = spaces_hook.spaces.clone().unwrap_or_default();

                html! {
                    <div>
                        <h2 class="text-xl font-semibold text-neutral-900 \
                                   dark:text-neutral-100 mb-6">
                            {"Auction Rounds"}
                        </h2>
                        <div class="space-y-4">
                            {sorted_rounds.iter().map(|round| {
                                let round_id = round.round_id;
                                let round_num = round.round_details.round_num;
                                let user_bids_for_round = bids_by_round.as_ref()
                                    .and_then(|map| map.get(&round_id))
                                    .cloned();
                                let results_for_round = results_by_round.as_ref()
                                    .and_then(|map| map.get(&round_id))
                                    .cloned();
                                // Get previous round's results for calculating bid values
                                let previous_round_results = if round_num > 0 {
                                    sorted_rounds.iter()
                                        .find(|r| r.round_details.round_num == round_num - 1)
                                        .and_then(|prev_round| {
                                            results_by_round.as_ref()
                                                .and_then(|map| map.get(&prev_round.round_id))
                                                .cloned()
                                        })
                                } else {
                                    None
                                };
                                html! {
                                    <RoundCard
                                        key={round_id.0.to_string()}
                                        round={round.clone()}
                                        bid_increment={props.auction.auction_details.auction_params.bid_increment}
                                        user_bids={user_bids_for_round}
                                        round_results={results_for_round}
                                        previous_round_results={previous_round_results}
                                        current_username={current_username.clone()}
                                        spaces={spaces.clone()}
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
    bid_increment: rust_decimal::Decimal,
    user_bids: Option<Vec<payloads::SpaceId>>,
    round_results: Option<Vec<payloads::RoundSpaceResult>>,
    previous_round_results: Option<Vec<payloads::RoundSpaceResult>>,
    current_username: Option<String>,
    spaces: Vec<payloads::responses::Space>,
}

#[function_component]
fn RoundCard(props: &RoundCardProps) -> Html {
    let round = &props.round;
    let round_num = round.round_details.round_num;

    // Get the results from the PREVIOUS round to determine which spaces
    // the user was high bidder on in that round (which means they won
    // those spaces in the previous round)
    let high_bidder_spaces = if let (Some(username), Some(results)) =
        (&props.current_username, &props.previous_round_results)
    {
        results
            .iter()
            .filter(|result| &result.winning_username == username)
            .collect::<Vec<_>>()
    } else {
        vec![]
    };

    // User bids in this round with their values
    // Bid value = previous round result value + bid increment
    let user_bid_details = if let Some(bid_space_ids) = &props.user_bids {
        bid_space_ids
            .iter()
            .filter_map(|space_id| {
                let space =
                    props.spaces.iter().find(|s| s.space_id == *space_id)?;
                // Get the previous round's result for this space
                let prev_value =
                    props.previous_round_results.as_ref().and_then(|results| {
                        results
                            .iter()
                            .find(|r| r.space_id == *space_id)
                            .map(|r| r.value)
                    });
                // Bid value is previous value + increment, or just increment
                // if no previous value
                let bid_value = prev_value
                    .unwrap_or(rust_decimal::Decimal::ZERO)
                    + props.bid_increment;
                Some((space.space_details.name.clone(), bid_value))
            })
            .collect::<Vec<_>>()
    } else {
        vec![]
    };

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-6 hover:shadow-md transition-shadow">
            <div class="grid grid-cols-3 gap-6">
                <div>
                    <h3 class="text-lg font-semibold text-neutral-900 \
                               dark:text-neutral-100 mb-4">
                        {format!("Round {}", round_num)}
                    </h3>
                    <div class="space-y-2">
                        <div>
                            <span class="text-sm text-neutral-600 \
                                         dark:text-neutral-400">
                                {"Started: "}
                            </span>
                            <TimestampDisplay
                                timestamp={round.round_details.start_at}
                                site_timezone={Option::<String>::None}
                            />
                        </div>
                        <div>
                            <span class="text-sm text-neutral-600 \
                                         dark:text-neutral-400">
                                {"Ended: "}
                            </span>
                            <TimestampDisplay
                                timestamp={round.round_details.end_at}
                                site_timezone={Option::<String>::None}
                            />
                        </div>
                        <div>
                            <span class="text-sm text-neutral-600 \
                                         dark:text-neutral-400">
                                {"Eligibility Threshold: "}
                            </span>
                            <span class="text-sm text-neutral-900 \
                                         dark:text-neutral-100">
                                {format!("{:.0}%", round.round_details.eligibility_threshold * 100.0)}
                            </span>
                        </div>
                    </div>
                </div>
                <div>
                    <div class="space-y-2">
                        <h4 class="text-sm font-medium text-neutral-700 \
                                   dark:text-neutral-300 mb-2">
                            {"Your Bids"}
                        </h4>
                        {if user_bid_details.is_empty() {
                            html! {
                                <p class="text-sm text-neutral-600 \
                                          dark:text-neutral-400">
                                    {"No bids placed"}
                                </p>
                            }
                        } else {
                            html! {
                                <ul class="space-y-1">
                                    {user_bid_details.iter().map(|(name, value)| {
                                        html! {
                                            <li class="text-sm text-neutral-900 \
                                                       dark:text-neutral-100">
                                                {format!("{}: ${:.2}", name, value)}
                                            </li>
                                        }
                                    }).collect::<Html>()}
                                </ul>
                            }
                        }}
                    </div>
                </div>
                <div>
                    <div class="space-y-2">
                        <h4 class="text-sm font-medium text-neutral-700 \
                                   dark:text-neutral-300 mb-2">
                            {if round_num > 0 {
                                format!("High Bidder from Round {}", round_num - 1)
                            } else {
                                "High Bidder Status".to_string()
                            }}
                        </h4>
                        {if high_bidder_spaces.is_empty() || round_num == 0 {
                            html! {
                                <p class="text-sm text-neutral-600 \
                                          dark:text-neutral-400">
                                    {"None"}
                                </p>
                            }
                        } else {
                            html! {
                                <ul class="space-y-1">
                                    {high_bidder_spaces.iter().filter_map(|result| {
                                        let space = props.spaces.iter()
                                            .find(|s| s.space_id == result.space_id)?;
                                        Some(html! {
                                            <li class="text-sm text-neutral-900 \
                                                       dark:text-neutral-100">
                                                {format!("{}: ${:.2}", space.space_details.name, result.value)}
                                            </li>
                                        })
                                    }).collect::<Html>()}
                                </ul>
                            }
                        }}
                    </div>
                </div>
            </div>
        </div>
    }
}
