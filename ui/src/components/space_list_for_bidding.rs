use payloads::{CurrencySettings, RoundSpaceResult, SpaceId, responses};
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use web_sys::HtmlElement;
use yew::prelude::*;

use crate::components::InlineEdit;
use crate::components::user_identity_display::render_user_name;
use payloads::responses::{UserIdentity, UserProfile};

/// Per-row resolved data. The list as a whole is gated on prices, bids,
/// and user values being fetched (see the parent's `render_section` over
/// the zipped fetches), so per-cell values are plain types — no skeleton
/// needed.
struct SpaceRowData {
    space: responses::Space,
    price: Option<Decimal>,
    user_value: Option<Decimal>,
    surplus: Option<Decimal>,
    winner: Option<UserIdentity>,
}

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
    /// Round-space results from the previous round (the data that produces
    /// the "Price" column). The parent gates the list on this fetch
    /// resolving, so it's a plain Vec here.
    pub prices: Vec<RoundSpaceResult>,
    /// User's bids in the current round, keyed by space.
    pub user_bids: HashSet<SpaceId>,
    /// User's per-space values (their max willingness-to-pay).
    pub user_values: HashMap<SpaceId, Decimal>,
    pub proxy_bidding_enabled: bool,
    /// The signed-in user. Resolved by the parent's `RequireAuth` gate, so
    /// this is always present. Identity comparisons (e.g., `is_high_bidder`)
    /// use `user_id`.
    pub current_user: UserProfile,
    pub bid_increment: Decimal,
    pub currency: CurrencySettings,
    pub on_bid: Callback<SpaceId>,
    pub on_delete_bid: Callback<SpaceId>,
    pub on_update_value: Callback<(SpaceId, Decimal)>,
    pub on_delete_value: Callback<SpaceId>,
    #[prop_or_default]
    pub auction_ended: bool,
    #[prop_or_default]
    pub auction_started: bool,
    /// User's eligibility for the current round. `None` if no prior
    /// eligibility (round 0) or if the parent didn't pass it (e.g.,
    /// auction not yet started). The parent gates the list on this fetch
    /// resolving, so it's a plain Option here.
    #[prop_or_default]
    pub user_eligibility: Option<f64>,
    /// Current activity points. The parent gates on this resolving, so
    /// it's a plain f64. Defaults to 0 when no auction is running.
    #[prop_or_default]
    pub current_activity: f64,
}

#[function_component]
pub fn SpaceListForBidding(props: &Props) -> Html {
    let sort_field = use_state(|| SortField::Name);
    let sort_direction = use_state(|| SortDirection::Ascending);
    let filter_no_value = use_state(|| false);

    // Build a price/winner lookup for O(1) per-row access during the row
    // build below. Both prices and bids are pre-fetched (the parent gates
    // the list on those resolving), so the lookup is over real data.
    let price_map: HashMap<SpaceId, &RoundSpaceResult> =
        props.prices.iter().map(|r| (r.space_id, r)).collect();

    // Filter spaces based on auction status
    let filtered_spaces: Vec<&responses::Space> = if props.auction_ended {
        // For concluded auctions: show spaces with auction history
        // (those with round results or user bids)
        props
            .spaces
            .iter()
            .filter(|space| {
                price_map.contains_key(&space.space_id)
                    || props.user_bids.contains(&space.space_id)
            })
            .collect()
    } else {
        // For in-progress auctions: show only available and not deleted
        props
            .spaces
            .iter()
            .filter(|space| {
                space.deleted_at.is_none() && space.space_details.is_available
            })
            .collect()
    };

    // Per-row resolved data. All inputs are post-gate plain values, so
    // each cell is just an `Option<T>`.
    let mut space_data: Vec<SpaceRowData> = filtered_spaces
        .iter()
        .map(|space| {
            let space_id = space.space_id;
            let result = price_map.get(&space_id);
            let price = result.map(|r| r.value);
            let winner = result.map(|r| r.winner.clone());
            let user_value = props.user_values.get(&space_id).copied();
            // Calculate surplus using a price of 0 if no previous price
            // exists (e.g., first round on this space).
            let surplus =
                user_value.map(|v| v - price.unwrap_or(Decimal::ZERO));
            SpaceRowData {
                space: (*space).clone(),
                price,
                user_value,
                surplus,
                winner,
            }
        })
        .collect();

    // "Hide spaces with no value" filter.
    if *filter_no_value {
        space_data.retain(|row| row.user_value.is_some());
    }

    // Sort. `None` is treated as smaller than any `Some` — semantically a
    // missing user value is worth less than any explicit value. This
    // ordering composes correctly with reverse: ascending puts `None`s at
    // the top, descending puts them at the bottom. (Note: this matches
    // the natural `Ord` impl for `Option<T>` in std.)
    let cmp_option = Option::<Decimal>::cmp;
    space_data.sort_by(|a, b| {
        let comparison = match *sort_field {
            SortField::Name => {
                a.space.space_details.name.cmp(&b.space.space_details.name)
            }
            SortField::Price => cmp_option(&a.price, &b.price),
            SortField::UserValue => cmp_option(&a.user_value, &b.user_value),
            SortField::Surplus => cmp_option(&a.surplus, &b.surplus),
        };

        match *sort_direction {
            SortDirection::Ascending => comparison,
            SortDirection::Descending => comparison.reverse(),
        }
    });

    // NodeRefs for each row's value cell, so Enter
    // can advance focus to the next row
    let value_refs = use_memo(space_data.len(), |n| {
        (0..*n).map(|_| NodeRef::default()).collect::<Vec<_>>()
    });

    let click_next_value = {
        let value_refs = value_refs.clone();
        move |idx: usize| {
            let next = idx + 1;
            if next < value_refs.len()
                && let Some(el) = value_refs[next].cast::<HtmlElement>()
            {
                el.click();
            }
        }
    };

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
            <div class="flex gap-2 sm:gap-4 items-center flex-wrap">
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
                    space_data.iter().enumerate().map(|(idx, row)| {
                        let space = &row.space;
                        let space_id = space.space_id;

                        let user_has_bid = props.user_bids.contains(&space_id);
                        let is_high_bidder = row
                            .winner
                            .as_ref()
                            .map(|w| w.user_id == props.current_user.user_id)
                            .unwrap_or(false);

                        // Eligibility check. `user_eligibility` is None
                        // for round 0 (no prior eligibility yet), in
                        // which case we conservatively allow bidding —
                        // the API enforces the actual limit.
                        let would_exceed_eligibility = match props
                            .user_eligibility
                        {
                            Some(eligibility)
                                if !user_has_bid && !is_high_bidder =>
                            {
                                let new_activity = props.current_activity
                                    + space.space_details.eligibility_points;
                                new_activity > eligibility
                            }
                            _ => false,
                        };

                        let on_value_enter = {
                            let click_next_value = click_next_value.clone();
                            Callback::from(move |()| {
                                click_next_value(idx);
                            })
                        };

                        html! {
                            <SpaceRow
                                key={space_id.0.to_string()}
                                space={space.clone()}
                                price={row.price}
                                bid_increment={props.bid_increment}
                                currency={props.currency.clone()}
                                user_value={row.user_value}
                                surplus={row.surplus}
                                proxy_bidding_enabled={props.proxy_bidding_enabled}
                                user_has_bid={user_has_bid}
                                is_high_bidder={is_high_bidder}
                                on_bid={props.on_bid.clone()}
                                on_delete_bid={props.on_delete_bid.clone()}
                                on_update_value={props.on_update_value.clone()}
                                on_delete_value={props.on_delete_value.clone()}
                                auction_ended={props.auction_ended}
                                auction_started={props.auction_started}
                                winner={row.winner.clone()}
                                would_exceed_eligibility={would_exceed_eligibility}
                                is_deleted={space.deleted_at.is_some()}
                                value_ref={value_refs[idx].clone()}
                                on_value_enter={on_value_enter}
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
    /// Per-row price. `None` if no prior bids on this space.
    price: Option<Decimal>,
    bid_increment: Decimal,
    currency: CurrencySettings,
    /// Per-row user value (their max willingness-to-pay for this space).
    user_value: Option<Decimal>,
    /// Per-row surplus = user_value - price.
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
    winner: Option<UserIdentity>,
    would_exceed_eligibility: bool,
    is_deleted: bool,
    value_ref: NodeRef,
    on_value_enter: Callback<()>,
}

#[function_component]
fn SpaceRow(props: &SpaceRowProps) -> Html {
    let space_id = props.space.space_id;

    // The bid price (current price + bid increment, or 0 for first bid).
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

    let on_value_change = {
        let on_update = props.on_update_value.clone();
        let on_delete = props.on_delete_value.clone();
        Callback::from(move |v: String| {
            if v.is_empty() {
                on_delete.emit(space_id);
            } else if let Ok(d) = v.parse::<Decimal>()
                && d >= Decimal::ZERO
            {
                on_update.emit((space_id, d));
            }
            // Invalid input is silently ignored; InlineEdit reverts to the
            // prior display value on blur.
        })
    };

    let value_str = props
        .user_value
        .map(|v| v.normalize().to_string())
        .unwrap_or_default();

    let display_str = match props.user_value {
        Some(v) => props.currency.format_amount(v),
        None => String::default(),
    };

    html! {
        <div class={format!(
            "border border-neutral-200 dark:border-neutral-700 \
            rounded-lg p-4 bg-white dark:bg-neutral-800{}",
            if props.is_deleted { " opacity-75" } else { "" }
        )}>
            <div class="grid grid-cols-3 md:grid-cols-6 gap-4 items-center">
                <div>
                    <div class="font-medium text-neutral-900 dark:text-white">
                        {&props.space.space_details.name}
                        {if props.is_deleted {
                            html! {
                                <span class="ml-2 text-xs \
                                      text-amber-600 \
                                      dark:text-amber-400">
                                    {"(deleted)"}
                                </span>
                            }
                        } else {
                            html! {}
                        }}
                    </div>
                </div>

                <div>
                    <div class="text-xs text-neutral-500 \
                                dark:text-neutral-400">
                        {"Points"}
                    </div>
                    <div class="text-sm font-medium \
                                text-neutral-900 \
                                dark:text-white">
                        {format!(
                            "{:.1}",
                            props.space
                                .space_details
                                .eligibility_points
                        )}
                    </div>
                </div>

                <div class="text-right md:text-left">
                    <div class="text-xs text-neutral-500 \
                                dark:text-neutral-400">
                        {"Price"}
                    </div>
                    <div class="text-sm font-medium \
                                text-neutral-900 \
                                dark:text-white">
                        {match props.price {
                            Some(price) => html! {
                                {props.currency.format_amount(price)}
                            },
                            None => html! {
                                {props.currency.placeholder_value()}
                            },
                        }}
                    </div>
                </div>

                <div>
                    <div class="text-xs text-neutral-500 \
                                dark:text-neutral-400">
                        {"Your Value"}
                    </div>
                    <InlineEdit
                        value={value_str}
                        display_value={display_str}
                        placeholder={props.currency.placeholder_value()}
                        on_change={on_value_change}
                        on_enter={props.on_value_enter.clone()}
                        container_ref={props.value_ref.clone()}
                        inputmode={AttrValue::Static("decimal")}
                        display_class="w-20 border border-dashed \
                            border-neutral-400 dark:border-neutral-500 \
                            hover:bg-neutral-100 dark:hover:bg-neutral-700"
                        input_class="w-20 font-medium"
                    />
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
                            _ => "text-neutral-500 dark:text-neutral-400",
                        }
                    )}>
                        {match props.surplus {
                            Some(value) => html! {
                                {props.currency.format_amount(value)}
                            },
                            None => html! {
                                {props.currency.placeholder_value()}
                            },
                        }}
                    </div>
                </div>

                <div class="flex justify-end">
                    {if props.auction_ended {
                        // Show winner when auction has concluded
                        if let Some(winner) = &props.winner {
                            html! {
                                <div class="text-right">
                                    <div class="text-xs text-neutral-500 \
                                                dark:text-neutral-400">
                                        {"Winner"}
                                    </div>
                                    <div class="text-sm font-medium \
                                                text-neutral-900 dark:text-white">
                                        {render_user_name(winner)}
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
                                {format!("Remove bid at {}", props.currency.format_amount(bid_price))}
                            </button>
                        }
                    } else if props.user_has_bid {
                        // When proxy bidding is on and user has bid
                        html! {
                            <span class="text-xs text-neutral-600 \
                                         dark:text-neutral-400 font-medium \
                                         text-right">
                                {format!("Already bid at {}", props.currency.format_amount(bid_price))}
                            </span>
                        }
                    } else if props.is_deleted {
                        // Cannot bid on deleted space
                        html! {
                            <div class="text-right">
                                <span class="text-xs text-amber-600 dark:text-amber-400 font-medium">
                                    {"This space has been deleted"}
                                </span>
                            </div>
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
                                {format!("Bid at {}", props.currency.format_amount(bid_price))}
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
