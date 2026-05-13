use payloads::{CurrencySettings, SpaceId};
use std::collections::HashSet;
use yew::prelude::*;

use crate::components::{
    AuctionContext, AuctionPageWrapper, AuctionTabHeader, AuctionToplineInfo,
    ConnectionStatusIndicator, CountdownTimer, ProxyBiddingControls,
    RoundIndicator, SpaceListForBidding, UserEligibilityDisplay,
    auction_tab_header::ActiveTab,
};
use crate::hooks::{
    Fetch, ProxyBiddingSettingsHookReturn, UserSpaceValuesHookReturn,
    render_section, stale_data_banner, use_last_round,
    use_proxy_bidding_settings, use_round_prices, use_spaces, use_user_bids,
    use_user_eligibility, use_user_space_values,
};
use payloads::AuctionId;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub auction_id: AuctionId,
}

#[function_component]
pub fn AuctionDetailPage(props: &Props) -> Html {
    let render_content = Callback::from(|ctx: AuctionContext| {
        html! {
            <div>
                <AuctionTabHeader
                    auction={ctx.auction.clone()}
                    active_tab={ActiveTab::Current}
                />
                <div class="py-6">
                    <AuctionContent
                        auction={ctx.auction.clone()}
                        site_timezone={ctx.site_timezone().map(String::from)}
                        currency={ctx.currency().clone()}
                        current_user={ctx.current_user.clone()}
                    />
                </div>
            </div>
        }
    });

    html! {
        <AuctionPageWrapper
            auction_id={props.auction_id}
            children={render_content}
        />
    }
}

#[derive(Properties, PartialEq)]
struct AuctionContentProps {
    auction: payloads::responses::Auction,
    site_timezone: Option<String>,
    currency: CurrencySettings,
    current_user: payloads::responses::UserProfile,
}

#[function_component]
fn AuctionContent(props: &AuctionContentProps) -> Html {
    let auction_id = props.auction.auction_id;
    let site_id = props.auction.auction_details.site_id;

    // Fetch all the data we need
    let last_round_hook = use_last_round(auction_id);
    let proxy_bidding_hook = use_proxy_bidding_settings(auction_id);
    let spaces_hook = use_spaces(site_id);
    let user_values_hook = use_user_space_values(site_id);

    // The fetch hooks now own their own SSE subscriptions; the connection
    // status is shared per-auction in the registry, so reading from any
    // subscribed hook returns the same live status.
    let connection_status = last_round_hook.connection_status;

    // Block rendering until the data needed to choose the auction-state branch
    // (cancelled / not-yet-started / active) is loaded.
    //
    // last_round_hook.data is FetchData<Option<LastRoundInfo>>:
    // - Fetched(None): no rounds exist yet (auction hasn't started)
    // - Fetched(Some(info)): rounds exist; info carries last/previous round
    render_section(
        &last_round_hook.inner,
        "auction",
        move |last_round_info_opt, _is_loading, errors| {
            let cancelled =
                props.auction.end_at.is_some() && last_round_info_opt.is_none();
            let body = if cancelled {
                html! {
                    <AuctionCancelledContent
                        auction={props.auction.clone()}
                        site_timezone={props.site_timezone.clone()}
                        currency={props.currency.clone()}
                        current_user={props.current_user.clone()}
                        spaces={spaces_hook.inner.clone()}
                        user_values={user_values_hook.clone()}
                    />
                }
            } else if let Some(last_round_info) = last_round_info_opt {
                html! {
                    <>
                        <ConnectionStatusIndicator status={connection_status} />
                        <AuctionRoundContent
                            auction={props.auction.clone()}
                            site_timezone={props.site_timezone.clone()}
                            currency={props.currency.clone()}
                            last_round={last_round_info.last_round.clone()}
                            previous_round_id={
                                last_round_info
                                    .previous_round
                                    .as_ref()
                                    .map(|r| r.round_id)
                            }
                            current_user={props.current_user.clone()}
                            spaces={spaces_hook.inner.clone()}
                            user_values={user_values_hook.clone()}
                            proxy_bidding={proxy_bidding_hook.clone()}
                        />
                    </>
                }
            } else {
                html! {
                    <AuctionNotStartedContent
                        auction={props.auction.clone()}
                        site_timezone={props.site_timezone.clone()}
                        currency={props.currency.clone()}
                        current_user={props.current_user.clone()}
                        connection_status={connection_status}
                        spaces={spaces_hook.inner.clone()}
                        user_values={user_values_hook.clone()}
                        proxy_bidding={proxy_bidding_hook.clone()}
                    />
                }
            };
            html! {
                <>
                    {stale_data_banner(errors)}
                    {body}
                </>
            }
        },
    )
}

#[derive(Properties, PartialEq)]
struct AuctionCancelledContentProps {
    auction: payloads::responses::Auction,
    site_timezone: Option<String>,
    currency: CurrencySettings,
    current_user: payloads::responses::UserProfile,
    spaces: Fetch<Vec<payloads::responses::Space>>,
    user_values: UserSpaceValuesHookReturn,
}

/// Auction was canceled before any rounds existed. Show topline info, a
/// cancellation banner, and the space list (for reference; bidding is
/// disabled and proxy bidding is hidden since the auction never ran).
#[function_component]
fn AuctionCancelledContent(props: &AuctionCancelledContentProps) -> Html {
    let no_op_bid = Callback::from(|_: SpaceId| {});
    html! {
        <div class="space-y-6">
            <AuctionToplineInfo
                auction={props.auction.clone()}
                site_timezone={props.site_timezone.clone()}
                currency={props.currency.clone()}
            />
            <div class="border border-neutral-200 dark:border-neutral-700 \
                        rounded-lg p-8 bg-white dark:bg-neutral-800">
                <div class="text-center space-y-4">
                    <h3 class="text-lg font-medium text-neutral-900 \
                               dark:text-white">
                        {"Auction Canceled"}
                    </h3>
                    <p class="text-neutral-600 dark:text-neutral-400">
                        {"This auction was canceled before it started."}
                    </p>
                </div>
            </div>

            // Auction was cancelled before any rounds existed, so prices and
            // bids are empty by definition; gate on spaces + user_values.
            {render_section(
                &props.spaces.zip_ref(&props.user_values.inner),
                "spaces",
                |(spaces, user_values), _, errors| html! {
                    <>
                        {stale_data_banner(errors)}
                        <SpaceListForBidding
                            spaces={(*spaces).clone()}
                            prices={Vec::new()}
                            user_values={(*user_values).clone()}
                            proxy_bidding_enabled={false}
                            user_bids={HashSet::new()}
                            current_user={props.current_user.clone()}
                            bid_increment={
                                props.auction.auction_details
                                    .auction_params.bid_increment
                            }
                            currency={props.currency.clone()}
                            on_bid={no_op_bid.clone()}
                            on_delete_bid={no_op_bid.clone()}
                            on_update_value={
                                props.user_values.update_value.clone()
                            }
                            on_delete_value={
                                props.user_values.delete_value.clone()
                            }
                            auction_ended={true}
                        />
                    </>
                },
            )}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct AuctionNotStartedContentProps {
    auction: payloads::responses::Auction,
    site_timezone: Option<String>,
    currency: CurrencySettings,
    current_user: payloads::responses::UserProfile,
    connection_status: crate::hooks::ConnectionStatus,
    spaces: Fetch<Vec<payloads::responses::Space>>,
    user_values: UserSpaceValuesHookReturn,
    proxy_bidding: ProxyBiddingSettingsHookReturn,
}

/// Auction has not yet started. Show topline info, countdown, proxy-bidding
/// controls, and a space list for setting values (bidding disabled).
#[function_component]
fn AuctionNotStartedContent(props: &AuctionNotStartedContentProps) -> Html {
    let no_op_bid = Callback::from(|_: SpaceId| {});
    html! {
        <div class="space-y-6">
            <AuctionToplineInfo
                auction={props.auction.clone()}
                site_timezone={props.site_timezone.clone()}
                currency={props.currency.clone()}
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
                            on_complete={Option::<Callback<()>>::None}
                        />
                    </div>
                    <p class="text-sm text-neutral-600 dark:text-neutral-400">
                        {"You can set your space values and enable proxy \
                          bidding now to prepare for the auction."}
                    </p>
                    <ConnectionStatusIndicator status={props.connection_status} />
                </div>
            </div>

            <ProxyBiddingControls settings={props.proxy_bidding.clone()} />

            // Space list for setting values (bidding disabled). The auction
            // hasn't started, so no prices or bids exist; gate on spaces +
            // user_values + proxy_bidding being fetched.
            {render_section(
                &props
                    .spaces
                    .zip_ref(&props.user_values.inner)
                    .zip_ref(&props.proxy_bidding.inner),
                "spaces",
                |((spaces, user_values), proxy_bidding_opt), _, errors| html! {
                    <>
                        {stale_data_banner(errors)}
                        <SpaceListForBidding
                            spaces={(*spaces).clone()}
                            prices={Vec::new()}
                            user_values={(*user_values).clone()}
                            proxy_bidding_enabled={proxy_bidding_opt.is_some()}
                            user_bids={HashSet::new()}
                            current_user={props.current_user.clone()}
                            bid_increment={
                                props.auction.auction_details
                                    .auction_params.bid_increment
                            }
                            currency={props.currency.clone()}
                            on_bid={no_op_bid.clone()}
                            on_delete_bid={no_op_bid.clone()}
                            on_update_value={
                                props.user_values.update_value.clone()
                            }
                            on_delete_value={
                                props.user_values.delete_value.clone()
                            }
                            auction_ended={false}
                        />
                    </>
                },
            )}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct AuctionRoundContentProps {
    auction: payloads::responses::Auction,
    site_timezone: Option<String>,
    currency: CurrencySettings,
    last_round: payloads::responses::AuctionRound,
    /// Round id of the round before `last_round`, used to fetch prices.
    /// `None` when `last_round` is round 0 (no previous round exists).
    previous_round_id: Option<payloads::AuctionRoundId>,
    current_user: payloads::responses::UserProfile,
    spaces: Fetch<Vec<payloads::responses::Space>>,
    user_values: UserSpaceValuesHookReturn,
    proxy_bidding: ProxyBiddingSettingsHookReturn,
}

#[function_component]
fn AuctionRoundContent(props: &AuctionRoundContentProps) -> Html {
    let round_id = props.last_round.round_id;
    let auction_id = props.auction.auction_id;

    // Error state for bid actions
    let bid_error = use_state(|| None::<String>);

    let round_prices_hook = use_round_prices(props.previous_round_id);
    let eligibility_hook = use_user_eligibility(round_id);
    let user_bids_hook = use_user_bids(auction_id, round_id);

    let eligibility = eligibility_hook.inner.clone();

    // current_activity is meaningful only once spaces, prices, and user_bids
    // are all loaded — until then, we don't know which spaces the user is
    // actively bidding on. The `Fetch` propagates loading/error state so
    // consumers (UserEligibilityDisplay, SpaceListForBidding) render
    // skeletons during the loading window rather than a misleading 0.
    let current_user_id = props.current_user.user_id;
    let current_activity: Fetch<f64> = props
        .spaces
        .zip_ref(&round_prices_hook.inner)
        .zip_ref(&user_bids_hook.inner)
        .map(|((spaces, prices), user_bid_space_ids)| {
            // sum() on empty iterator returns -0.0; add 0.0 to normalize.
            spaces
                .iter()
                .filter(|space| {
                    let is_high_bidder = prices
                        .iter()
                        .find(|p| p.space_id == space.space_id)
                        .map(|p| p.winner.user_id == current_user_id)
                        .unwrap_or(false);
                    let has_bid = user_bid_space_ids.contains(&space.space_id);
                    is_high_bidder || has_bid
                })
                .map(|space| space.space_details.eligibility_points)
                .sum::<f64>()
                + 0.0
        });

    // Callback for bidding. The user's bids hook refetches itself on the
    // `BidsChanged` SSE event the create_bid transaction emits, so this
    // handler doesn't need to refetch anything on success — it only surfaces
    // errors. Round prices, eligibility, and the rounds list are not affected
    // by an intra-round bid change.
    let on_bid = {
        let round_id = props.last_round.round_id;
        let bid_error = bid_error.clone();

        Callback::from(move |space_id: SpaceId| {
            let round_id = round_id;
            let bid_error = bid_error.clone();

            yew::platform::spawn_local(async move {
                bid_error.set(None);
                let api_client = crate::get_api_client();
                if let Err(e) =
                    api_client.create_bid(&space_id, &round_id).await
                {
                    bid_error.set(Some(format!("Failed to place bid: {}", e)));
                }
            });
        })
    };

    // Callback for deleting a bid. As with `on_bid`, the SSE-driven refetch
    // covers the user's bids and nothing else needs to refresh.
    let on_delete_bid = {
        let round_id = props.last_round.round_id;
        let bid_error = bid_error.clone();

        Callback::from(move |space_id: SpaceId| {
            let round_id = round_id;
            let bid_error = bid_error.clone();

            yew::platform::spawn_local(async move {
                bid_error.set(None);
                let api_client = crate::get_api_client();
                if let Err(e) =
                    api_client.delete_bid(&space_id, &round_id).await
                {
                    bid_error.set(Some(format!("Failed to remove bid: {}", e)));
                }
            });
        })
    };

    // Callback for updating user values
    let on_update_value = props.user_values.update_value.clone();
    let on_delete_value = props.user_values.delete_value.clone();

    html! {
        <div class="space-y-6">
            // Auction info
            <AuctionToplineInfo
                auction={props.auction.clone()}
                site_timezone={props.site_timezone.clone()}
                currency={props.currency.clone()}
            />

            // Current round indicator
            // IMPORTANT: key prop forces remount on round change to avoid stale
            // closure captures. The interval closure captures round_concluded state,
            // and even with use_effect_with(round_end_at), the closure can capture
            // a stale state handle from before the effect runs. Forcing a remount
            // ensures all state is fresh. This is the correct pattern, not a workaround.
            <RoundIndicator
                key={props.last_round.round_id.to_string()}
                round_num={props.last_round.round_details.round_num}
                round_end_at={props.last_round.round_details.end_at}
                auction_end_at={props.auction.end_at}
                on_round_end={Option::<Callback<()>>::None}
            />

            // User eligibility
            <UserEligibilityDisplay
                eligibility_points={eligibility.clone()}
                eligibility_threshold={
                    props.last_round.round_details.eligibility_threshold
                }
                current_activity={current_activity.clone()}
            />

            // Proxy bidding controls
            <ProxyBiddingControls
                settings={props.proxy_bidding.clone()}
            />

            // Bid action error
            {if let Some(error) = &*bid_error {
                html! {
                    <div class="p-3 rounded-md bg-red-50 \
                                dark:bg-red-900/20 border \
                                border-red-200 dark:border-red-800">
                        <p class="text-sm text-red-700 \
                                  dark:text-red-400">
                            {error}
                        </p>
                    </div>
                }
            } else {
                html! {}
            }}

            // Space list for bidding. Gate on the six hooks the list
            // depends on. user_eligibility and current_activity also flow
            // into UserEligibilityDisplay separately (with its own
            // gating); here they're collapsed to plain values via the
            // gate. While the eligibility/activity inputs are still
            // loading the list waits — but those are derived from the
            // same prices/user_bids/spaces hooks the list also needs, so
            // gating is essentially equivalent.
            {render_section(
                &props
                    .spaces
                    .zip_ref(&round_prices_hook.inner)
                    .zip_ref(&user_bids_hook.inner)
                    .zip_ref(&props.user_values.inner)
                    .zip_ref(&eligibility)
                    .zip_ref(&current_activity)
                    .zip_ref(&props.proxy_bidding.inner),
                "spaces",
                |((((((spaces, prices), user_bids), user_values), eligibility_opt), activity), proxy_bidding_opt), _, errors| html! {
                    <>
                        {stale_data_banner(errors)}
                        <SpaceListForBidding
                            spaces={(*spaces).clone()}
                            prices={(*prices).clone()}
                            user_values={(*user_values).clone()}
                            proxy_bidding_enabled={proxy_bidding_opt.is_some()}
                            user_bids={(*user_bids).clone()}
                            current_user={props.current_user.clone()}
                            bid_increment={props.auction.auction_details.auction_params.bid_increment}
                            currency={props.currency.clone()}
                            on_bid={on_bid.clone()}
                            on_delete_bid={on_delete_bid.clone()}
                            on_update_value={on_update_value.clone()}
                            on_delete_value={on_delete_value.clone()}
                            auction_ended={props.auction.end_at.is_some()}
                            auction_started={true}
                            user_eligibility={**eligibility_opt}
                            current_activity={**activity}
                        />
                    </>
                },
            )}
        </div>
    }
}
