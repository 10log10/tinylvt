use payloads::{AuctionId, CurrencySettings, responses::UserProfile};
use yew::prelude::*;

use crate::components::{
    AuctionContext, AuctionPageWrapper, AuctionTabHeader,
    ConnectionStatusIndicator, TimestampDisplay, auction_tab_header::ActiveTab,
};
use crate::hooks::{
    render_section, stale_data_banner, use_auction_round_results,
    use_auction_rounds, use_auction_user_bids, use_spaces,
};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub auction_id: AuctionId,
}

#[function_component]
pub fn AuctionRoundsPage(props: &Props) -> Html {
    let render_content = Callback::from(|ctx: AuctionContext| {
        html! {
            <div>
                <AuctionTabHeader
                    auction={ctx.auction.clone()}
                    active_tab={ActiveTab::Rounds}
                />
                <div class="py-6">
                    <RoundsPageContent
                        auction={ctx.auction.clone()}
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
struct RoundsPageContentProps {
    auction: payloads::responses::Auction,
    currency: CurrencySettings,
    current_user: UserProfile,
}

#[function_component]
fn RoundsPageContent(props: &RoundsPageContentProps) -> Html {
    let auction_id = props.auction.auction_id;
    let rounds_hook = use_auction_rounds(auction_id);

    render_section(&rounds_hook.inner, "rounds", {
        let auction = props.auction.clone();
        let currency = props.currency.clone();
        let current_user = props.current_user.clone();
        move |rounds, _is_loading, errors| {
            html! {
                <>
                    {stale_data_banner(errors)}
                    <RoundsContent
                        auction={auction.clone()}
                        rounds={rounds.clone()}
                        currency={currency.clone()}
                        current_user={current_user.clone()}
                    />
                </>
            }
        }
    })
}

#[derive(Properties, PartialEq)]
struct RoundsContentProps {
    auction: payloads::responses::Auction,
    rounds: Vec<payloads::responses::AuctionRound>,
    currency: CurrencySettings,
    current_user: UserProfile,
}

#[function_component]
fn RoundsContent(props: &RoundsContentProps) -> Html {
    let rounds = &props.rounds;
    let auction_id = props.auction.auction_id;
    let site_id = props.auction.auction_details.site_id;

    // Fetch all user bids and round results for the auction
    let user_bids_hook = use_auction_user_bids(auction_id, rounds.clone());
    let round_results_hook =
        use_auction_round_results(auction_id, rounds.clone());
    let spaces_hook = use_spaces(site_id);

    // The subscribed fetch hooks share an EventSource per auction; reading
    // connection_status from any of them returns the same live status.
    let connection_status = user_bids_hook.connection_status;

    if rounds.is_empty() {
        return html! {
            <>
                <ConnectionStatusIndicator status={connection_status} />
                <div class="text-center py-12">
                    <p class="text-neutral-600 dark:text-neutral-400">
                        {"This auction has not started yet. No rounds have \
                         been created."}
                    </p>
                </div>
            </>
        };
    }

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

            <ConnectionStatusIndicator status={connection_status} />

            {render_section(
                &user_bids_hook
                    .inner
                    .zip_ref(&round_results_hook.inner)
                    .zip_ref(&spaces_hook.inner),
                "rounds",
                |((bids_by_round, results_by_round), spaces), _, errors| html! {
                    <>
                        {stale_data_banner(errors)}

                        <div class="space-y-4">
                            {sorted_rounds.iter().map(|round| {
                                let round_id = round.round_id;
                                let round_num = round.round_details.round_num;
                                let user_bids_for_round =
                                    bids_by_round.get(&round_id).cloned();
                                // Previous round's results, used to compute
                                // the displayed bid values in this round. If
                                // that previous round's results fetch errored
                                // we still pass the Err through so the bid
                                // section can fall back to "—" rather than
                                // showing a misleading 0.
                                let previous_round_results = if round_num > 0 {
                                    sorted_rounds.iter()
                                        .find(|r| r.round_details.round_num == round_num - 1)
                                        .and_then(|prev_round| {
                                            results_by_round.get(&prev_round.round_id).cloned()
                                        })
                                } else {
                                    None
                                };
                                html! {
                                    <RoundCard
                                        key={round_id.0.to_string()}
                                        round={round.clone()}
                                        bid_increment={props.auction.auction_details.auction_params.bid_increment}
                                        currency={props.currency.clone()}
                                        user_bids={user_bids_for_round}
                                        previous_round_results={previous_round_results}
                                        current_user={props.current_user.clone()}
                                        spaces={(*spaces).clone()}
                                    />
                                }
                            }).collect::<Html>()}
                        </div>
                    </>
                },
            )}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct RoundCardProps {
    round: payloads::responses::AuctionRound,
    bid_increment: rust_decimal::Decimal,
    currency: CurrencySettings,
    /// `None` means no bids hook entry yet (parent still loading); `Some(Ok)`
    /// is the user's bid space ids; `Some(Err)` carries the per-round fetch
    /// error so the bids section renders an inline error.
    user_bids: Option<Result<Vec<payloads::SpaceId>, String>>,
    /// Previous round's results — drives both the high-bidder section and
    /// the bid value computation in this round's bids list. Same shape as
    /// `user_bids`.
    previous_round_results:
        Option<Result<Vec<payloads::RoundSpaceResult>, String>>,
    current_user: UserProfile,
    spaces: Vec<payloads::responses::Space>,
}

#[function_component]
fn RoundCard(props: &RoundCardProps) -> Html {
    let round = &props.round;
    let round_num = round.round_details.round_num;

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-6 hover:shadow-md transition-shadow">
            <div class="grid grid-cols-1 md:grid-cols-3 gap-4 md:gap-6">
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
                        {render_bids_section(props)}
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
                        {render_high_bidder_section(props)}
                    </div>
                </div>
            </div>
        </div>
    }
}

fn render_bids_section(props: &RoundCardProps) -> Html {
    let bid_space_ids = match &props.user_bids {
        Some(Ok(ids)) => ids,
        Some(Err(msg)) => {
            return error_text(&format!("Couldn't load bids: {msg}"));
        }
        None => return placeholder_text("No bids placed"),
    };

    if bid_space_ids.is_empty() {
        return placeholder_text("No bids placed");
    }

    html! {
        <ul class="space-y-1">
            {bid_space_ids.iter().filter_map(|space_id| {
                let space = props.spaces.iter()
                    .find(|s| s.space_id == *space_id)?;
                let value_label = compute_bid_value_label(
                    space_id,
                    &props.previous_round_results,
                    props.bid_increment,
                    &props.currency,
                );
                Some(html! {
                    <li class="text-sm text-neutral-900 dark:text-neutral-100">
                        {format!("{}: {}", space.space_details.name, value_label)}
                    </li>
                })
            }).collect::<Html>()}
        </ul>
    }
}

/// Bid value = previous round's result for the space + bid increment, or
/// 0 if there is no previous result. `None` means there is no previous
/// round at all (round 0); `Some(Ok)` with no row for this space means the
/// previous round had no bid on it (so it opens at 0 again this round).
/// `Some(Err)` is the only case where we can't compute a value, and
/// renders as `"value unavailable"`.
///
/// This relies on the parent gating render on `use_auction_round_results`
/// resolving, so an in-flight fetch never reaches this function as `None`.
fn compute_bid_value_label(
    space_id: &payloads::SpaceId,
    previous_round_results: &Option<
        Result<Vec<payloads::RoundSpaceResult>, String>,
    >,
    bid_increment: rust_decimal::Decimal,
    currency: &CurrencySettings,
) -> String {
    match previous_round_results {
        Some(Ok(results)) => {
            let value = results
                .iter()
                .find(|r| r.space_id == *space_id)
                .map(|prev| prev.value + bid_increment)
                .unwrap_or(rust_decimal::Decimal::ZERO);
            currency.format_amount(value)
        }
        Some(Err(_)) => "value unavailable".to_string(),
        None => currency.format_amount(rust_decimal::Decimal::ZERO),
    }
}

fn render_high_bidder_section(props: &RoundCardProps) -> Html {
    // `None` means there is no previous round (round 0), so there can't be
    // a high bidder yet — same rendering as "previous round had no winners
    // for this user". Same gating-on-render assumption as
    // `compute_bid_value_label`.
    let results = match &props.previous_round_results {
        Some(Ok(r)) => r,
        Some(Err(msg)) => {
            return error_text(&format!("Couldn't load round results: {msg}"));
        }
        None => return placeholder_text("None"),
    };

    let high_bidder_spaces: Vec<_> = results
        .iter()
        .filter(|r| r.winner.user_id == props.current_user.user_id)
        .collect();

    if high_bidder_spaces.is_empty() {
        return placeholder_text("None");
    }

    html! {
        <ul class="space-y-1">
            {high_bidder_spaces.iter().filter_map(|result| {
                let space = props.spaces.iter()
                    .find(|s| s.space_id == result.space_id)?;
                Some(html! {
                    <li class="text-sm text-neutral-900 dark:text-neutral-100">
                        {format!(
                            "{}: {}",
                            space.space_details.name,
                            props.currency.format_amount(result.value),
                        )}
                    </li>
                })
            }).collect::<Html>()}
        </ul>
    }
}

fn placeholder_text(text: &str) -> Html {
    html! {
        <p class="text-sm text-neutral-600 dark:text-neutral-400">
            {text.to_string()}
        </p>
    }
}

fn error_text(text: &str) -> Html {
    html! {
        <p class="text-sm text-red-600 dark:text-red-400">
            {text.to_string()}
        </p>
    }
}
