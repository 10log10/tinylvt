use payloads::{AuctionId, SpaceId};
use rust_decimal::Decimal;
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{
    State,
    components::{
        AuctionTabHeader, AuctionToplineInfo, CountdownTimer,
        ProxyBiddingControls, RoundIndicator, SpaceListForBidding,
        UserEligibilityDisplay, auction_tab_header::ActiveTab,
    },
    hooks::{
        use_auction_detail, use_auction_rounds, use_current_round,
        use_exponential_refetch, use_proxy_bidding_settings, use_round_prices,
        use_spaces, use_user_bids, use_user_eligibility, use_user_space_values,
    },
};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub auction_id: AuctionId,
}

#[function_component]
pub fn AuctionDetailPage(props: &Props) -> Html {
    let auction_hook = use_auction_detail(props.auction_id);
    let (state, _) = use_store::<State>();

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

    // Get site timezone for timestamp display
    let site_id = auction.auction_details.site_id;
    let site = state.get_site(site_id);
    let site_timezone = site.and_then(|s| s.site_details.timezone.clone());

    html! {
        <div>
            <AuctionTabHeader
                auction={auction.clone()}
                active_tab={ActiveTab::Current}
            />
            <div class="py-6">
                <AuctionContent
                    auction={auction.clone()}
                    site_timezone={site_timezone}
                    auction_refetch={auction_hook.refetch.clone()}
                />
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct AuctionContentProps {
    auction: payloads::responses::Auction,
    site_timezone: Option<String>,
    auction_refetch: Callback<()>,
}

#[function_component]
fn AuctionContent(props: &AuctionContentProps) -> Html {
    let auction_id = props.auction.auction_id;
    let site_id = props.auction.auction_details.site_id;
    let (state, _) = use_store::<State>();

    // Fetch all the data we need
    let current_round_hook = use_current_round(auction_id);
    let proxy_bidding_hook = use_proxy_bidding_settings(auction_id);
    let spaces_hook = use_spaces(site_id);
    let user_values_hook = use_user_space_values(site_id);

    // Get current user's username
    let current_username = match &state.auth_state {
        crate::state::AuthState::LoggedIn(profile) => {
            Some(profile.username.clone())
        }
        _ => None,
    };

    // Set up exponential backoff refetch for round transitions
    // We need to refetch both the round AND the auction (for end_at)
    let combined_refetch = {
        let round_refetch = current_round_hook.refetch.clone();
        let auction_refetch = props.auction_refetch.clone();
        Callback::from(move |_| {
            round_refetch.emit(());
            auction_refetch.emit(());
        })
    };

    let (start_transition_refetch, cancel_transition_refetch, transition_error) =
        use_exponential_refetch(combined_refetch, 1000, 16000);

    // Cancel refetch when round data changes (successful refetch)
    {
        let cancel_refetch = cancel_transition_refetch.clone();
        let current_round = current_round_hook.current_round.clone();

        use_effect_with(current_round, move |_| {
            // When round changes, cancel any pending refetch timeouts
            cancel_refetch.emit(());
            || ()
        });
    }

    // Cancel refetch when auction ends
    {
        let cancel_refetch = cancel_transition_refetch.clone();
        let auction_end_at = props.auction.end_at;

        use_effect_with(auction_end_at, move |_| {
            // When auction ends, cancel any pending refetch timeouts
            if auction_end_at.is_some() {
                cancel_refetch.emit(());
            }
            || ()
        });
    }

    // Show loading state only for initial load (when data is None)
    // During refetches, keep the UI rendered and let data update smoothly
    if current_round_hook.current_round.is_none()
        || proxy_bidding_hook.settings.is_none()
        || spaces_hook.spaces.is_none()
        || user_values_hook.values.is_none()
    {
        return html! {
            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400">
                    {"Loading auction data..."}
                </p>
            </div>
        };
    }

    // Handle case where auction hasn't started yet
    // current_round is Option<Option<AuctionRound>>:
    // - None: still loading (handled above)
    // - Some(None): fetched, but no rounds exist
    // - Some(Some(round)): fetched with a round
    let Some(Some(current_round)) = &current_round_hook.current_round else {
        let on_auction_start = start_transition_refetch.clone();

        return html! {
            <div class="space-y-6">
                <AuctionToplineInfo
                    auction={props.auction.clone()}
                    site_timezone={props.site_timezone.clone()}
                />
                <div class="border border-neutral-200 dark:border-neutral-700 \
                            rounded-lg p-8 bg-white dark:bg-neutral-800">
                    <div class="text-center space-y-4">
                        <h3 class="text-lg font-medium text-neutral-900 \
                                   dark:text-white">
                            {"Auction Not Yet Started"}
                        </h3>
                        <p class="text-neutral-600 dark:text-neutral-400">
                            {"The auction will begin in:"}
                        </p>
                        <div class="text-3xl font-semibold text-neutral-900 \
                                    dark:text-white">
                            <CountdownTimer
                                target_time={props.auction.auction_details.start_at}
                                on_complete={Some(on_auction_start)}
                            />
                        </div>
                        {if let Some(error) = transition_error {
                            html! {
                                <div class="mt-4 p-3 bg-red-50 \
                                            dark:bg-red-900/20 rounded-md \
                                            border border-red-200 \
                                            dark:border-red-800">
                                    <p class="text-sm text-red-700 \
                                              dark:text-red-400">
                                        {error}
                                    </p>
                                </div>
                            }
                        } else {
                            html! {}
                        }}
                    </div>
                </div>
            </div>
        };
    };

    // Extract data from hooks to pass to child component
    let spaces = spaces_hook.spaces.clone();
    let user_values = user_values_hook.values.clone();
    let proxy_bidding_enabled =
        proxy_bidding_hook.settings.clone().flatten().is_some();
    let proxy_max_items = proxy_bidding_hook
        .settings
        .clone()
        .flatten()
        .map(|s| s.max_items)
        .unwrap_or(1);
    let update_value = user_values_hook.update_value.clone();
    let delete_value = user_values_hook.delete_value.clone();
    let proxy_update = proxy_bidding_hook.update.clone();
    let proxy_delete = proxy_bidding_hook.delete.clone();

    // Render component with round-specific hooks
    html! {
        <AuctionRoundContent
            auction={props.auction.clone()}
            site_timezone={props.site_timezone.clone()}
            current_round={current_round.clone()}
            current_username={current_username}
            spaces={spaces}
            user_values={user_values}
            proxy_bidding_enabled={proxy_bidding_enabled}
            proxy_max_items={proxy_max_items}
            update_value={update_value}
            delete_value={delete_value}
            proxy_update={proxy_update}
            proxy_delete={proxy_delete}
            on_round_end={start_transition_refetch}
            transition_error={transition_error}
        />
    }
}

#[derive(Properties, PartialEq)]
struct AuctionRoundContentProps {
    auction: payloads::responses::Auction,
    site_timezone: Option<String>,
    current_round: payloads::responses::AuctionRound,
    current_username: Option<String>,
    spaces: Option<Vec<payloads::responses::Space>>,
    user_values: Option<std::collections::HashMap<SpaceId, Decimal>>,
    proxy_bidding_enabled: bool,
    proxy_max_items: i32,
    update_value: Callback<(SpaceId, Decimal)>,
    delete_value: Callback<SpaceId>,
    proxy_update: Callback<i32>,
    proxy_delete: Callback<()>,
    on_round_end: Callback<()>,
    transition_error: Option<String>,
}

#[function_component]
fn AuctionRoundContent(props: &AuctionRoundContentProps) -> Html {
    // Now we can safely call round-specific hooks unconditionally
    let round_id = props.current_round.round_id;
    let auction_id = props.auction.auction_id;
    let current_round_num = props.current_round.round_details.round_num;

    // Fetch all rounds to find the previous one (for prices)
    let rounds_hook = use_auction_rounds(auction_id);

    // Refetch rounds when current round changes (to get updated price data)
    {
        let rounds_refetch = rounds_hook.refetch.clone();
        let current_round = props.current_round.clone();

        use_effect_with(current_round, move |_| {
            rounds_refetch.emit(());
            || ()
        });
    }

    // Prices are stored in round_space_results for the PREVIOUS round
    // (when the scheduler processes a round, it stores results FOR that round,
    // then creates the next round). So we need to fetch prices from round_num - 1.
    // For round 0, there is no previous round, so we pass None.
    let prices_round_id = if current_round_num == 0 {
        None
    } else {
        rounds_hook.rounds.as_ref().and_then(|rounds| {
            rounds
                .iter()
                .find(|r| r.round_details.round_num == current_round_num - 1)
                .map(|r| r.round_id)
        })
    };

    let round_prices_hook = use_round_prices(prices_round_id);
    let eligibility_hook = use_user_eligibility(round_id);

    // Fetch user's existing bids for this round
    let user_bids_hook = use_user_bids(round_id);

    // Show loading for round-specific data
    if rounds_hook.is_loading
        || round_prices_hook.is_loading
        || eligibility_hook.is_loading
        || user_bids_hook.is_loading
    {
        return html! {
            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400">
                    {"Loading round data..."}
                </p>
            </div>
        };
    }

    // Get the data we need
    let prices = round_prices_hook.prices.unwrap_or_default();

    let user_values = props.user_values.clone().unwrap_or_default();
    let spaces = props.spaces.clone().unwrap_or_default();
    let eligibility = eligibility_hook.eligibility.unwrap_or(0.0);
    let user_bid_space_ids = user_bids_hook.bid_space_ids.unwrap_or_default();

    // Calculate current activity: sum of points for spaces where user is
    // high bidder or has placed a bid in this round
    let current_activity: f64 = spaces
        .iter()
        .filter(|space| {
            let is_high_bidder = props
                .current_username
                .as_ref()
                .and_then(|username| {
                    prices
                        .iter()
                        .find(|p| p.space_id == space.space_id)
                        .and_then(|p| p.winning_username.as_ref())
                        .map(|winner| winner == username)
                })
                .unwrap_or(false);
            let has_bid = user_bid_space_ids.contains(&space.space_id);
            is_high_bidder || has_bid
        })
        .map(|space| space.space_details.eligibility_points)
        .sum();

    // Callbacks for proxy bidding controls
    let on_proxy_toggle = {
        let update = props.proxy_update.clone();
        let delete = props.proxy_delete.clone();
        let max_items = props.proxy_max_items;
        Callback::from(move |enabled: bool| {
            if enabled {
                update.emit(max_items);
            } else {
                delete.emit(());
            }
        })
    };

    let on_proxy_update = {
        let update = props.proxy_update.clone();
        Callback::from(move |new_max_items: i32| {
            update.emit(new_max_items);
        })
    };

    // Callback for bidding
    let on_bid = {
        let round_id = props.current_round.round_id;
        let round_prices_refetch = round_prices_hook.refetch.clone();
        let eligibility_refetch = eligibility_hook.refetch.clone();
        let rounds_refetch = rounds_hook.refetch.clone();
        let user_bids_refetch = user_bids_hook.refetch.clone();

        Callback::from(move |space_id: SpaceId| {
            let round_id = round_id;
            let round_prices_refetch = round_prices_refetch.clone();
            let eligibility_refetch = eligibility_refetch.clone();
            let rounds_refetch = rounds_refetch.clone();
            let user_bids_refetch = user_bids_refetch.clone();

            yew::platform::spawn_local(async move {
                let api_client = crate::get_api_client();
                match api_client.create_bid(&space_id, &round_id).await {
                    Ok(_) => {
                        // Refresh the data to show the new bid
                        round_prices_refetch.emit(());
                        eligibility_refetch.emit(());
                        rounds_refetch.emit(());
                        user_bids_refetch.emit(());
                        tracing::info!(
                            "Successfully placed bid on {:?}",
                            space_id
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to place bid: {}", e);
                        web_sys::console::error_1(
                            &format!("Failed to place bid: {}", e).into(),
                        );
                    }
                }
            });
        })
    };

    // Callback for deleting a bid
    let on_delete_bid = {
        let round_id = props.current_round.round_id;
        let round_prices_refetch = round_prices_hook.refetch.clone();
        let eligibility_refetch = eligibility_hook.refetch.clone();
        let rounds_refetch = rounds_hook.refetch.clone();
        let user_bids_refetch = user_bids_hook.refetch.clone();

        Callback::from(move |space_id: SpaceId| {
            let round_id = round_id;
            let round_prices_refetch = round_prices_refetch.clone();
            let eligibility_refetch = eligibility_refetch.clone();
            let rounds_refetch = rounds_refetch.clone();
            let user_bids_refetch = user_bids_refetch.clone();

            yew::platform::spawn_local(async move {
                let api_client = crate::get_api_client();
                match api_client.delete_bid(&space_id, &round_id).await {
                    Ok(_) => {
                        // Refresh the data to show the bid removal
                        round_prices_refetch.emit(());
                        eligibility_refetch.emit(());
                        rounds_refetch.emit(());
                        user_bids_refetch.emit(());
                        tracing::info!(
                            "Successfully removed bid on {:?}",
                            space_id
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to remove bid: {}", e);
                        web_sys::console::error_1(
                            &format!("Failed to remove bid: {}", e).into(),
                        );
                    }
                }
            });
        })
    };

    // Callback for updating user values
    let on_update_value = props.update_value.clone();
    let on_delete_value = props.delete_value.clone();

    html! {
        <div class="space-y-6">
            // Auction info
            <AuctionToplineInfo
                auction={props.auction.clone()}
                site_timezone={props.site_timezone.clone()}
            />

            // Current round indicator
            <RoundIndicator
                round_num={props.current_round.round_details.round_num}
                round_end_at={props.current_round.round_details.end_at}
                auction_end_at={props.auction.end_at}
                on_round_end={Some(props.on_round_end.clone())}
            />

            // Show error if round transition failed
            {if let Some(error) = &props.transition_error {
                html! {
                    <div class="p-4 bg-red-50 dark:bg-red-900/20 rounded-md \
                                border border-red-200 dark:border-red-800">
                        <p class="text-sm text-red-700 dark:text-red-400">
                            {error}
                        </p>
                    </div>
                }
            } else {
                html! {}
            }}

            // User eligibility
            <UserEligibilityDisplay
                eligibility_points={eligibility}
                eligibility_threshold={
                    props.current_round.round_details.eligibility_threshold
                }
                current_activity={current_activity}
            />

            // Proxy bidding controls
            <ProxyBiddingControls
                is_enabled={props.proxy_bidding_enabled}
                max_items={props.proxy_max_items}
                on_toggle={on_proxy_toggle}
                on_update={on_proxy_update}
                is_loading={false}
            />

            // Space list for bidding
            <SpaceListForBidding
                spaces={spaces}
                prices={prices}
                user_values={user_values}
                proxy_bidding_enabled={props.proxy_bidding_enabled}
                user_bid_space_ids={user_bid_space_ids}
                current_username={props.current_username.clone()}
                bid_increment={props.auction.auction_details.auction_params.bid_increment}
                current_eligibility={eligibility}
                eligibility_threshold={
                    props.current_round.round_details.eligibility_threshold
                }
                on_bid={on_bid}
                on_delete_bid={on_delete_bid}
                on_update_value={on_update_value}
                on_delete_value={on_delete_value}
                auction_ended={props.auction.end_at.is_some()}
            />
        </div>
    }
}
