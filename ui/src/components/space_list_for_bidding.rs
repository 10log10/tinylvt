use payloads::{RoundSpaceResult, SpaceId, responses};
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use yew::prelude::*;

use crate::hooks::FetchState;

#[derive(Debug, Clone, Copy, PartialEq)]
enum SortField {
    Name,
    Price,
    UserValue,
    Surplus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub spaces: Vec<responses::Space>,
    pub prices: Vec<RoundSpaceResult>,
    pub user_values: HashMap<SpaceId, Decimal>,
    pub proxy_bidding_enabled: bool,
    pub user_bid_space_ids: HashSet<SpaceId>,
    pub current_username: Option<String>,
    pub bid_increment: Decimal,
    pub on_bid: Callback<SpaceId>,
    pub on_delete_bid: Callback<SpaceId>,
    pub on_update_value: Callback<(SpaceId, Decimal)>,
    pub on_delete_value: Callback<SpaceId>,
    #[prop_or_default]
    pub auction_ended: bool,
    #[prop_or_default]
    pub auction_started: bool,
    #[prop_or_default]
    pub user_eligibility: FetchState<Option<f64>>,
}

#[function_component]
pub fn SpaceListForBidding(props: &Props) -> Html {
    let sort_field = use_state(|| SortField::Name);
    let sort_direction = use_state(|| SortDirection::Ascending);
    let filter_no_value = use_state(|| false);

    // Create price and winner lookup
    let price_map: HashMap<SpaceId, RoundSpaceResult> = props
        .prices
        .iter()
        .map(|r| (r.space_id, r.clone()))
        .collect();

    // Calculate current activity: sum of points for spaces where user is
    // high bidder or has placed a bid in this round
    let current_activity: f64 = props
        .spaces
        .iter()
        .filter(|space| {
            let is_high_bidder = props
                .current_username
                .as_ref()
                .and_then(|username| {
                    price_map
                        .get(&space.space_id)
                        .map(|r| &r.winning_username == username)
                })
                .unwrap_or(false);
            let has_bid = props.user_bid_space_ids.contains(&space.space_id);
            is_high_bidder || has_bid
        })
        .map(|space| space.space_details.eligibility_points)
        .sum();

    // Prepare space data
    let mut space_data: Vec<_> = props
        .spaces
        .iter()
        .map(|space| {
            let space_id = space.space_id;
            let result = price_map.get(&space_id);
            let price_opt = result.map(|r| r.value);
            let winning_username = result.map(|r| r.winning_username.clone());
            let user_value = props.user_values.get(&space_id).copied();
            // Calculate surplus using price of 0 if no previous price exists
            let surplus =
                user_value.map(|v| v - price_opt.unwrap_or(Decimal::ZERO));

            (space, price_opt, user_value, surplus, winning_username)
        })
        .collect();

    // Apply filters
    space_data.retain(|(_, _, user_value, _, _)| {
        if *filter_no_value {
            user_value.is_some()
        } else {
            true
        }
    });

    // Sort (None values sort last for Price, UserValue and Surplus)
    space_data.sort_by(|a, b| {
        let comparison = match *sort_field {
            SortField::Name => {
                a.0.space_details.name.cmp(&b.0.space_details.name)
            }
            SortField::Price => match (&a.1, &b.1) {
                (Some(av), Some(bv)) => av.cmp(bv),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            },
            SortField::UserValue => match (&a.2, &b.2) {
                (Some(av), Some(bv)) => av.cmp(bv),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            },
            SortField::Surplus => match (&a.3, &b.3) {
                (Some(av), Some(bv)) => av.cmp(bv),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            },
        };

        match *sort_direction {
            SortDirection::Ascending => comparison,
            SortDirection::Descending => comparison.reverse(),
        }
    });

    let on_sort_click = {
        let sort_field = sort_field.clone();
        let sort_direction = sort_direction.clone();
        let current_field = *sort_field;

        Callback::from(move |new_field: SortField| {
            if current_field == new_field {
                sort_direction.set(match *sort_direction {
                    SortDirection::Ascending => SortDirection::Descending,
                    SortDirection::Descending => SortDirection::Ascending,
                });
            } else {
                sort_field.set(new_field);
                sort_direction.set(SortDirection::Descending);
            }
        })
    };

    let on_filter_toggle = {
        let filter_no_value = filter_no_value.clone();
        Callback::from(move |_| {
            filter_no_value.set(!*filter_no_value);
        })
    };

    html! {
        <div class="space-y-4">
            <div class="flex items-center justify-between">
                <h3 class="text-lg font-semibold text-neutral-900 \
                           dark:text-white">
                    {"Spaces"}
                </h3>
            </div>

            // Filters and Sort
            <div class="flex gap-4 items-center flex-wrap">
                <span class="text-sm font-medium text-neutral-700 \
                             dark:text-neutral-300">
                    {"Sort by:"}
                </span>
                <SortButton
                    label="Name"
                    field={SortField::Name}
                    current_field={*sort_field}
                    current_direction={*sort_direction}
                    on_click={on_sort_click.clone()}
                />
                <SortButton
                    label="Price"
                    field={SortField::Price}
                    current_field={*sort_field}
                    current_direction={*sort_direction}
                    on_click={on_sort_click.clone()}
                />
                <SortButton
                    label="Your Value"
                    field={SortField::UserValue}
                    current_field={*sort_field}
                    current_direction={*sort_direction}
                    on_click={on_sort_click.clone()}
                />
                <SortButton
                    label="Surplus"
                    field={SortField::Surplus}
                    current_field={*sort_field}
                    current_direction={*sort_direction}
                    on_click={on_sort_click.clone()}
                />

                <div class="ml-auto">
                    <label class="flex items-center gap-2 cursor-pointer \
                                  select-none">
                        <input
                            type="checkbox"
                            checked={*filter_no_value}
                            onchange={on_filter_toggle}
                            class="h-4 w-4 text-neutral-600 \
                                   focus:ring-neutral-500 \
                                   border-neutral-300 \
                                   dark:border-neutral-600 rounded"
                        />
                        <span class="text-sm text-neutral-700 \
                                     dark:text-neutral-300">
                            {"Hide spaces with no value"}
                        </span>
                    </label>
                </div>
            </div>

            // Space List
            <div class="space-y-2">
                {if space_data.is_empty() {
                    html! {
                        <div class="text-center py-12">
                            <p class="text-neutral-600 dark:text-neutral-400">
                                {"No spaces match the current filters."}
                            </p>
                        </div>
                    }
                } else {
                    space_data.iter().map(|(space, price, user_value, surplus, winning_username)| {
                        let user_has_bid = props.user_bid_space_ids.contains(&space.space_id);
                        let is_high_bidder = props.current_username.as_ref()
                            .and_then(|username| {
                                winning_username.as_ref().map(|winner| winner == username)
                            })
                            .unwrap_or(false);

                        // Check if bidding on this space would exceed eligibility
                        // Extract Option<f64> from FetchState
                        let eligibility_value = props.user_eligibility.as_ref().cloned().flatten();
                        let would_exceed_eligibility = if let Some(eligibility) = eligibility_value {
                            // If user doesn't have a bid on this space yet, check if adding it would exceed
                            if !user_has_bid && !is_high_bidder {
                                let new_activity = current_activity + space.space_details.eligibility_points;
                                new_activity > eligibility
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        html! {
                            <SpaceRow
                                key={space.space_id.0.to_string()}
                                space={(*space).clone()}
                                price={*price}
                                bid_increment={props.bid_increment}
                                user_value={*user_value}
                                surplus={*surplus}
                                proxy_bidding_enabled={props.proxy_bidding_enabled}
                                user_has_bid={user_has_bid}
                                is_high_bidder={is_high_bidder}
                                on_bid={props.on_bid.clone()}
                                on_delete_bid={props.on_delete_bid.clone()}
                                on_update_value={props.on_update_value.clone()}
                                on_delete_value={props.on_delete_value.clone()}
                                auction_ended={props.auction_ended}
                                auction_started={props.auction_started}
                                winning_username={winning_username.clone()}
                                would_exceed_eligibility={would_exceed_eligibility}
                            />
                        }
                    }).collect::<Html>()
                }}
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct SortButtonProps {
    label: &'static str,
    field: SortField,
    current_field: SortField,
    current_direction: SortDirection,
    on_click: Callback<SortField>,
}

#[function_component]
fn SortButton(props: &SortButtonProps) -> Html {
    let is_active = props.field == props.current_field;

    let onclick = {
        let on_click = props.on_click.clone();
        let field = props.field;
        Callback::from(move |_| {
            on_click.emit(field);
        })
    };

    html! {
        <button
            onclick={onclick}
            class={format!(
                "text-sm px-2 py-1 rounded {}",
                if is_active {
                    "bg-neutral-200 dark:bg-neutral-700 font-medium \
                     text-neutral-900 dark:text-white"
                } else {
                    "text-neutral-600 dark:text-neutral-400 \
                     hover:bg-neutral-100 dark:hover:bg-neutral-800"
                }
            )}
        >
            {props.label}
            {if is_active {
                html! {
                    <span class="ml-1">
                        {match props.current_direction {
                            SortDirection::Ascending => "↑",
                            SortDirection::Descending => "↓",
                        }}
                    </span>
                }
            } else {
                html! {}
            }}
        </button>
    }
}

#[derive(Properties, PartialEq)]
struct SpaceRowProps {
    space: responses::Space,
    price: Option<Decimal>,
    bid_increment: Decimal,
    user_value: Option<Decimal>,
    surplus: Option<Decimal>,
    proxy_bidding_enabled: bool,
    user_has_bid: bool,
    is_high_bidder: bool,
    on_bid: Callback<SpaceId>,
    on_delete_bid: Callback<SpaceId>,
    on_update_value: Callback<(SpaceId, Decimal)>,
    on_delete_value: Callback<SpaceId>,
    auction_ended: bool,
    auction_started: bool,
    winning_username: Option<String>,
    would_exceed_eligibility: bool,
}

#[function_component]
fn SpaceRow(props: &SpaceRowProps) -> Html {
    let space_id = props.space.space_id;
    let is_editing = use_state(|| false);
    let input_value = use_state(String::new);

    // Calculate the bid price (current price + bid increment, or 0 for first bid)
    let bid_price = match props.price {
        Some(price) => price + props.bid_increment,
        None => Decimal::ZERO,
    };

    let on_bid_click = {
        let on_bid = props.on_bid.clone();
        Callback::from(move |_| {
            on_bid.emit(space_id);
        })
    };

    let on_value_click = {
        let is_editing = is_editing.clone();
        let input_value = input_value.clone();
        let user_value = props.user_value;
        Callback::from(move |_| {
            // Set input to current value when starting edit
            let initial_text = match user_value {
                Some(v) => format!("{:.2}", v),
                None => String::new(),
            };
            input_value.set(initial_text);
            is_editing.set(true);
        })
    };

    let on_input_change = {
        let input_value = input_value.clone();
        Callback::from(move |e: yew::InputEvent| {
            let target: web_sys::HtmlInputElement = e.target_unchecked_into();
            input_value.set(target.value());
        })
    };

    let save_value = {
        let is_editing = is_editing.clone();
        let input_value = input_value.clone();
        let on_update_value = props.on_update_value.clone();
        let on_delete_value = props.on_delete_value.clone();
        move || {
            let text = (*input_value).trim();
            if text.is_empty() {
                // Empty input means delete the value (set to None)
                on_delete_value.emit(space_id);
            } else {
                // Try to parse as decimal
                match text.parse::<Decimal>() {
                    Ok(value) if value >= Decimal::ZERO => {
                        on_update_value.emit((space_id, value));
                    }
                    _ => {
                        // Invalid input, revert to original value
                        tracing::warn!(
                            "Invalid value input: '{}'. Must be \
                             non-negative number.",
                            text
                        );
                    }
                }
            }
            is_editing.set(false);
        }
    };

    let on_input_blur = {
        let save_value = save_value.clone();
        Callback::from(move |_| {
            save_value();
        })
    };

    let on_input_keydown = {
        let save_value = save_value.clone();
        let is_editing = is_editing.clone();
        Callback::from(move |e: web_sys::KeyboardEvent| {
            if e.key() == "Enter" {
                e.prevent_default();
                save_value();
            } else if e.key() == "Escape" {
                e.prevent_default();
                // Cancel editing without saving
                is_editing.set(false);
            }
        })
    };

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-4 bg-white dark:bg-neutral-800">
            <div class="grid grid-cols-6 gap-4 items-center">
                <div>
                    <div class="font-medium text-neutral-900 dark:text-white">
                        {&props.space.space_details.name}
                    </div>
                </div>

                <div>
                    <div class="text-xs text-neutral-500 \
                                dark:text-neutral-400">
                        {"Points"}
                    </div>
                    <div class="text-sm font-medium text-neutral-900 \
                                dark:text-white">
                        {format!("{:.1}", props.space.space_details.eligibility_points)}
                    </div>
                </div>

                <div>
                    <div class="text-xs text-neutral-500 \
                                dark:text-neutral-400">
                        {"Price"}
                    </div>
                    <div class="text-sm font-medium text-neutral-900 \
                                dark:text-white">
                        {match props.price {
                            Some(price) => format!("${:.2}", price),
                            None => "--".to_string(),
                        }}
                    </div>
                </div>

                <div>
                    <div class="text-xs text-neutral-500 \
                                dark:text-neutral-400">
                        {"Your Value"}
                    </div>
                    {if *is_editing {
                        html! {
                            <input
                                type="text"
                                value={(*input_value).clone()}
                                oninput={on_input_change}
                                onblur={on_input_blur}
                                onkeydown={on_input_keydown}
                                class="w-20 px-2 py-1 text-sm border \
                                       border-neutral-300 dark:border-neutral-600 \
                                       rounded bg-white dark:bg-neutral-900 \
                                       text-neutral-900 dark:text-white \
                                       focus:outline-none focus:ring-2 \
                                       focus:ring-neutral-500"
                                autofocus={true}
                            />
                        }
                    } else {
                        html! {
                            <div
                                onclick={on_value_click}
                                class="text-sm font-medium text-neutral-900 \
                                       dark:text-white cursor-pointer \
                                       hover:bg-neutral-100 \
                                       dark:hover:bg-neutral-700 px-2 py-1 \
                                       rounded transition-colors"
                            >
                                {match props.user_value {
                                    Some(value) => format!("${:.2}", value),
                                    None => "$--".to_string(),
                                }}
                            </div>
                        }
                    }}
                </div>

                <div>
                    <div class="text-xs text-neutral-500 \
                                dark:text-neutral-400">
                        {"Surplus"}
                    </div>
                    <div class={format!(
                        "text-sm font-medium {}",
                        match props.surplus {
                            Some(s) if s >= Decimal::ZERO => {
                                "text-neutral-900 dark:text-white"
                            }
                            Some(_) => "text-neutral-500 dark:text-neutral-400",
                            None => "text-neutral-500 dark:text-neutral-400",
                        }
                    )}>
                        {match props.surplus {
                            Some(value) => format!("${:.2}", value),
                            None => "$--".to_string(),
                        }}
                    </div>
                </div>

                <div class="flex justify-end">
                    {if props.auction_ended {
                        // Show winner when auction has concluded
                        if let Some(username) = &props.winning_username {
                            html! {
                                <div class="text-right">
                                    <div class="text-xs text-neutral-500 \
                                                dark:text-neutral-400">
                                        {"Winner"}
                                    </div>
                                    <div class="text-sm font-medium \
                                                text-neutral-900 dark:text-white">
                                        {username}
                                    </div>
                                </div>
                            }
                        } else {
                            html! {
                                <span class="text-xs text-neutral-500 \
                                             dark:text-neutral-400">
                                    {"No winner"}
                                </span>
                            }
                        }
                    } else if props.is_high_bidder {
                        // User is currently the high bidder from previous round
                        html! {
                            <span class="text-xs text-neutral-600 \
                                         dark:text-neutral-400 font-medium \
                                         text-right">
                                {"High bidder"}
                            </span>
                        }
                    } else if props.user_has_bid && !props.proxy_bidding_enabled {
                        // When user has bid and proxy bidding is off,
                        // show button to remove bid
                        let on_delete_bid_click = {
                            let on_delete_bid = props.on_delete_bid.clone();
                            Callback::from(move |_| {
                                on_delete_bid.emit(space_id);
                            })
                        };
                        html! {
                            <button
                                onclick={on_delete_bid_click}
                                class="bg-neutral-900 hover:bg-neutral-800 \
                                       dark:bg-neutral-100 \
                                       dark:text-neutral-900 \
                                       dark:hover:bg-neutral-200 text-white \
                                       px-4 py-2 rounded-md text-sm \
                                       font-medium transition-colors"
                            >
                                {format!("Remove bid at ${:.2}", bid_price)}
                            </button>
                        }
                    } else if props.user_has_bid {
                        // When proxy bidding is on and user has bid
                        html! {
                            <span class="text-xs text-neutral-600 \
                                         dark:text-neutral-400 font-medium \
                                         text-right">
                                {format!("Already bid at ${:.2}", bid_price)}
                            </span>
                        }
                    } else if !props.auction_started {
                        // Auction hasn't started yet - no bidding allowed
                        html! {}
                    } else if props.would_exceed_eligibility {
                        // Cannot bid because it would exceed eligibility
                        html! {
                            <span class="text-xs text-neutral-600 \
                                         dark:text-neutral-400 text-right">
                                {"Insufficient eligibility"}
                            </span>
                        }
                    } else if !props.proxy_bidding_enabled {
                        html! {
                            <button
                                onclick={on_bid_click}
                                class="bg-neutral-900 hover:bg-neutral-800 \
                                       dark:bg-neutral-100 \
                                       dark:text-neutral-900 \
                                       dark:hover:bg-neutral-200 text-white \
                                       px-4 py-2 rounded-md text-sm \
                                       font-medium transition-colors"
                            >
                                {format!("Bid at ${:.2}", bid_price)}
                            </button>
                        }
                    } else {
                        html! {
                            <span class="text-xs text-neutral-500 \
                                         dark:text-neutral-400">
                                {"Proxy bidding"}
                            </span>
                        }
                    }}
                </div>
            </div>
        </div>
    }
}
