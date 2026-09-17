use payloads::{
    CurrencySettings, RoundSpaceResult, SpaceCategoryId, SpaceId, responses,
};
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

use crate::components::InlineEdit;
use crate::components::category_select::{category_name, format_points};
use crate::components::user_identity_display::render_user_name;
use crate::hooks::MyCapsMap;
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
    pub bid_increment: payloads::BidIncrement,
    pub currency: CurrencySettings,
    pub on_bid: Callback<SpaceId>,
    pub on_delete_bid: Callback<SpaceId>,
    pub on_update_value: Callback<(SpaceId, Decimal)>,
    pub on_delete_value: Callback<SpaceId>,
    #[prop_or_default]
    pub auction_ended: bool,
    #[prop_or_default]
    pub auction_started: bool,
    /// Inactive members can't bid, so their bid buttons give way to a
    /// note.
    pub member_is_active: bool,
    /// User's eligibility for the current round, already interpreted by the
    /// API against the prior round's threshold. The parent gates the list on
    /// this fetch resolving, so it's a plain value here. Defaults to
    /// `Unlimited` when no auction is running (the parent passes the real
    /// value once it has one).
    #[prop_or(payloads::Eligibility::Unlimited)]
    pub user_eligibility: payloads::Eligibility,
    /// Current activity points. The parent gates on this resolving, so
    /// it's a plain f64. Defaults to 0 when no auction is running.
    #[prop_or_default]
    pub current_activity: f64,
    /// The community's space categories, for category labels and the
    /// category-wide value assignment control. Empty when the community
    /// uses no categories.
    #[prop_or_default]
    pub categories: Vec<responses::SpaceCategory>,
    /// The user's per-category cap buckets in a capped auction; `None`
    /// when the auction is uncapped (no cap gating). A missing bucket
    /// means the user cannot bid on that bucket's spaces.
    #[prop_or_default]
    pub bidder_caps: Option<MyCapsMap>,
    /// Bulk value upsert for the category-wide assignment control.
    #[prop_or_default]
    pub on_update_values: Callback<Vec<(SpaceId, Decimal)>>,
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
            let surplus = user_value.map(|v| {
                v - payloads::current_space_price(
                    price,
                    space.space_details.reserve_price,
                )
            });
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

    let category_names: HashMap<SpaceCategoryId, String> = props
        .categories
        .iter()
        .map(|c| (c.id, c.name.clone()))
        .collect();

    // Active points per cap bucket (current-round bids plus standing
    // wins), mirroring the server's cap check. Only meaningful in capped
    // auctions, but cheap to compute.
    let cap_active: HashMap<Option<SpaceCategoryId>, f64> = {
        let mut active = HashMap::new();
        for space in &props.spaces {
            let is_high_bidder = price_map
                .get(&space.space_id)
                .map(|r| r.winner.user_id == props.current_user.user_id)
                .unwrap_or(false);
            if is_high_bidder || props.user_bids.contains(&space.space_id) {
                *active
                    .entry(space.space_details.category_id)
                    .or_insert(0.0) += space.space_details.eligibility_points;
            }
        }
        active
    };

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

    // Category-wide value assignment: apply one value to every visible
    // space of a bucket — a category, or the uncategorized spaces.
    let mut value_assign_options: Vec<(
        Option<SpaceCategoryId>,
        String,
        Vec<SpaceId>,
    )> = props
        .categories
        .iter()
        .filter_map(|category| {
            let space_ids: Vec<SpaceId> = filtered_spaces
                .iter()
                .filter(|s| s.space_details.category_id == Some(category.id))
                .map(|s| s.space_id)
                .collect();
            (!space_ids.is_empty())
                .then(|| (Some(category.id), category.name.clone(), space_ids))
        })
        .collect();
    let uncategorized: Vec<SpaceId> = filtered_spaces
        .iter()
        .filter(|s| s.space_details.category_id.is_none())
        .map(|s| s.space_id)
        .collect();
    if !uncategorized.is_empty() {
        value_assign_options.push((
            None,
            category_name(None, &props.categories),
            uncategorized,
        ));
    }

    let on_apply_category_value = {
        let on_update_values = props.on_update_values.clone();
        let value_assign_options = value_assign_options.clone();
        Callback::from(
            move |(bucket, value): (Option<SpaceCategoryId>, Decimal)| {
                if let Some((_, _, space_ids)) =
                    value_assign_options.iter().find(|(id, _, _)| *id == bucket)
                {
                    on_update_values.emit(
                        space_ids.iter().map(|id| (*id, value)).collect(),
                    );
                }
            },
        )
    };

    html! {
        <div class="space-y-4">
            <div class="flex items-center justify-between">
                <h3 class="text-lg font-semibold text-neutral-900 \
                           dark:text-white">
                    {"Spaces"}
                </h3>
            </div>

            {bidding_capacity_panel(
                &props.bidder_caps,
                &cap_active,
                &props.categories,
            )}

            {if !props.auction_ended && !value_assign_options.is_empty() {
                html! {
                    <CategoryValueAssign
                        options={
                            value_assign_options
                                .iter()
                                .map(|(id, name, spaces)| {
                                    (*id, name.clone(), spaces.len())
                                })
                                .collect::<Vec<_>>()
                        }
                        currency={props.currency.clone()}
                        on_apply={on_apply_category_value}
                    />
                }
            } else {
                html! {}
            }}

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

                        // Eligibility check. Bidding a space the user
                        // already bid on or is winning adds nothing to their
                        // activity, so those never exceed. Otherwise, adding
                        // this space's points must stay within their
                        // eligibility (Unlimited always passes; a Finite(0.0)
                        // budget only permits zero-point spaces). The API
                        // enforces the real limit; this just gates the button.
                        let would_exceed_eligibility = if user_has_bid
                            || is_high_bidder
                        {
                            false
                        } else {
                            let new_activity = props.current_activity
                                + space.space_details.eligibility_points;
                            !props.user_eligibility.permits(new_activity)
                        };

                        // Cap check, sharing the server's rule via
                        // cap_permits: bidding a space the user already
                        // bid on or is winning adds nothing to the bucket,
                        // otherwise the space's points must fit in the
                        // bucket's remaining cap. A missing bucket means a
                        // cap of 0, which still permits zero-point spaces.
                        let cap_message: Option<AttrValue> = if user_has_bid
                            || is_high_bidder
                            || props.auction_ended
                        {
                            None
                        } else if let Some(caps) = &props.bidder_caps {
                            let bucket = space.space_details.category_id;
                            let active =
                                cap_active.get(&bucket).copied().unwrap_or(0.0);
                            let points =
                                space.space_details.eligibility_points;
                            let cap = caps.get(&bucket).copied();
                            if payloads::cap_permits(cap, active, points) {
                                None
                            } else if cap.is_none() {
                                Some(AttrValue::Static(
                                    "No cap assigned for this category",
                                ))
                            } else {
                                Some(AttrValue::Static(
                                    "Category cap reached",
                                ))
                            }
                        } else {
                            None
                        };

                        let category_label = space
                            .space_details
                            .category_id
                            .and_then(|id| category_names.get(&id).cloned());

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
                                category_label={category_label}
                                cap_message={cap_message}
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
                                member_is_active={props.member_is_active}
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

/// Per-bucket capacity summary for capped auctions: how many of the
/// user's cap points are used (standing wins plus current-round bids) in
/// each bucket they hold a cap for. Hidden for uncapped auctions; shown
/// even for ended or canceled ones, where the caps (and final usage)
/// remain useful information.
fn bidding_capacity_panel(
    bidder_caps: &Option<MyCapsMap>,
    cap_active: &HashMap<Option<SpaceCategoryId>, f64>,
    categories: &[responses::SpaceCategory],
) -> Html {
    let Some(caps) = bidder_caps else {
        return html! {};
    };

    let mut buckets: Vec<(String, f64, f64)> = caps
        .iter()
        .map(|(bucket, cap)| {
            let name = category_name(*bucket, categories);
            let used = cap_active.get(bucket).copied().unwrap_or(0.0);
            (name, used, *cap)
        })
        .collect();
    buckets.sort_by(|a, b| a.0.cmp(&b.0));

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-4 bg-white dark:bg-neutral-800">
            <div class="text-xs text-neutral-500 dark:text-neutral-400 mb-2">
                {"Your bidding capacity (points used of your cap, counting \
                  bids and standing wins)"}
            </div>
            {if buckets.is_empty() {
                html! {
                    <p class="text-sm text-neutral-600 \
                              dark:text-neutral-400">
                        {"You have no caps in this auction, so you cannot \
                          bid. A community leader assigns caps."}
                    </p>
                }
            } else {
                html! {
                    <div class="flex flex-wrap gap-x-6 gap-y-1">
                        {for buckets.iter().map(|(name, used, cap)| html! {
                            <span class="text-sm text-neutral-900 \
                                         dark:text-white">
                                <span class="font-medium">{name}</span>
                                {format!(
                                    ": {} of {} used",
                                    format_points(*used),
                                    format_points(*cap),
                                )}
                            </span>
                        })}
                    </div>
                }
            }}
        </div>
    }
}

/// The select value encoding the uncategorized bucket ("" is the
/// unselected placeholder, so None needs its own sentinel).
const UNCATEGORIZED_OPTION: &str = "uncategorized";

#[derive(Properties, PartialEq)]
struct CategoryValueAssignProps {
    /// (bucket, name, number of spaces it would apply to); the None
    /// bucket is the uncategorized spaces.
    options: Vec<(Option<SpaceCategoryId>, String, usize)>,
    currency: CurrencySettings,
    on_apply: Callback<(Option<SpaceCategoryId>, Decimal)>,
}

/// Compact control to set one value on every listed space of a bucket at
/// once — identical spaces in a category are interchangeable, so one
/// value for all of them is the common case.
#[function_component]
fn CategoryValueAssign(props: &CategoryValueAssignProps) -> Html {
    let select_ref = use_node_ref();
    let value_ref = use_node_ref();
    let selected = use_state(|| None::<Option<SpaceCategoryId>>);

    let on_select_change = {
        let selected = selected.clone();
        Callback::from(move |e: Event| {
            let select =
                e.target().unwrap().dyn_into::<HtmlSelectElement>().unwrap();
            let value = select.value();
            selected.set(match value.as_str() {
                "" => None,
                UNCATEGORIZED_OPTION => Some(None),
                id => id.parse().ok().map(|id| Some(SpaceCategoryId(id))),
            });
        })
    };

    let on_submit = {
        let value_ref = value_ref.clone();
        let selected = selected.clone();
        let on_apply = props.on_apply.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let Some(bucket) = *selected else {
                return;
            };
            let input = value_ref.cast::<HtmlInputElement>().unwrap();
            let Ok(value) = input.value().parse::<Decimal>() else {
                return;
            };
            on_apply.emit((bucket, value));
        })
    };

    let count_for_selected = selected.and_then(|bucket| {
        props
            .options
            .iter()
            .find(|(option_bucket, _, _)| *option_bucket == bucket)
            .map(|(_, _, count)| *count)
    });

    html! {
        <form
            onsubmit={on_submit}
            class="flex gap-2 items-center flex-wrap text-sm"
        >
            <span class="text-neutral-700 dark:text-neutral-300">
                {"Set one value for all spaces in"}
            </span>
            <select
                ref={select_ref}
                onchange={on_select_change}
                class="px-2 py-1.5 border border-neutral-300 \
                       dark:border-neutral-600 rounded-md bg-white \
                       dark:bg-neutral-700 text-neutral-900 \
                       dark:text-neutral-100 text-sm focus:outline-none \
                       focus:ring-2 focus:ring-neutral-500"
            >
                <option value="" selected={selected.is_none()}>
                    {"Choose category"}
                </option>
                {for props.options.iter().map(|(bucket, name, count)| {
                    let value = match bucket {
                        Some(id) => id.to_string(),
                        None => UNCATEGORIZED_OPTION.to_string(),
                    };
                    html! {
                        <option value={value}>
                            {format!("{} ({} spaces)", name, count)}
                        </option>
                    }
                })}
            </select>
            <input
                ref={value_ref}
                type="text"
                inputmode="decimal"
                placeholder={props.currency.placeholder_value()}
                class="w-24 px-2 py-1.5 border border-neutral-300 \
                       dark:border-neutral-600 rounded-md bg-white \
                       dark:bg-neutral-700 text-neutral-900 \
                       dark:text-neutral-100 text-sm focus:outline-none \
                       focus:ring-2 focus:ring-neutral-500"
            />
            <button
                type="submit"
                disabled={selected.is_none()}
                class="py-1.5 px-3 rounded-md text-sm font-medium text-white \
                       bg-neutral-900 hover:bg-neutral-800 \
                       dark:bg-neutral-100 dark:text-neutral-900 \
                       dark:hover:bg-neutral-200 disabled:opacity-50 \
                       disabled:cursor-not-allowed transition-colors \
                       duration-200"
            >
                {match count_for_selected {
                    Some(count) => format!("Apply to {} spaces", count),
                    None => "Apply".to_string(),
                }}
            </button>
        </form>
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
    bid_increment: payloads::BidIncrement,
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
    member_is_active: bool,
    winner: Option<UserIdentity>,
    would_exceed_eligibility: bool,
    is_deleted: bool,
    value_ref: NodeRef,
    on_value_enter: Callback<()>,
    /// Name of the space's category, shown under the space name.
    #[prop_or_default]
    category_label: Option<String>,
    /// Set when the user's cap blocks bidding on this space; the message
    /// replaces the bid button.
    #[prop_or_default]
    cap_message: Option<AttrValue>,
}

/// Small trailing label that appears next to a negative price/reserve to
/// remind the reader that "negative" means the winner is compensated for
/// taking on the space. On mobile the trailer stacks under the price so
/// the column heading still sits over the number rather than over the
/// (longer) trailer text. Returns empty html for non-negative amounts.
fn negative_explainer(amount: Decimal) -> Html {
    if amount < Decimal::ZERO {
        html! {
            <span class="block md:inline md:ml-1 text-xs font-normal \
                         text-neutral-500 dark:text-neutral-400">
                {"(winner gets paid)"}
            </span>
        }
    } else {
        html! {}
    }
}

#[function_component]
fn SpaceRow(props: &SpaceRowProps) -> Html {
    let space_id = props.space.space_id;
    let reserve_price = props.space.space_details.reserve_price;

    let bid_price = payloads::next_bid_amount(
        props.price,
        props.bid_increment,
        reserve_price,
    );

    // The price shown in the row's "Price" column. Falls back to the
    // reserve when no prior round has produced a price, so the user sees
    // the effective starting price rather than a "$--" placeholder.
    let displayed_price =
        payloads::current_space_price(props.price, reserve_price);

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
            } else if let Ok(d) = v.parse::<Decimal>() {
                // Negative values are valid for chore semantics — "I'll
                // accept down to -$5" means "this is worth at least -$5
                // to me." The backend enforces the gating rules.
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
                    {if let Some(category) = &props.category_label {
                        html! {
                            <div class="text-xs text-neutral-500 \
                                        dark:text-neutral-400">
                                {category}
                            </div>
                        }
                    } else {
                        html! {}
                    }}
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
                        {props.currency.format_amount(displayed_price)}
                        {negative_explainer(displayed_price)}
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
                    } else if !props.member_is_active {
                        html! {
                            <span class="text-xs text-neutral-600 \
                                         dark:text-neutral-400 text-right">
                                {"Inactive members can't bid"}
                            </span>
                        }
                    } else if let Some(message) = &props.cap_message {
                        // The user's per-category cap blocks this bid
                        html! {
                            <span class="text-xs text-neutral-600 \
                                         dark:text-neutral-400 text-right">
                                {message}
                            </span>
                        }
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
            {sign_mismatch_warning(
                reserve_price.0,
                props.user_value,
                &props.currency,
            )}
        </div>
    }
}

/// Renders a verbose warning under a bidding row when the user's stated
/// value looks inconsistent with the space's chore/normal classification.
/// Three flavors:
/// - Reserve is a chore but user typed a positive value (they'd pay to take the
///   chore).
/// - Reserve is non-negative but user typed a negative value (they'd only
///   accept the space if paid; on a non-chore reserve this means they won't
///   bid).
/// - Reserve is a chore but user typed zero (they'd take the chore for free --
///   legal but worth confirming).
///
/// A zero reserve is treated as non-negative (no chore semantics).
/// Returns empty html otherwise.
fn sign_mismatch_warning(
    reserve_price: Decimal,
    user_value: Option<Decimal>,
    currency: &CurrencySettings,
) -> Html {
    let Some(value) = user_value else {
        return html! {};
    };
    let reserve_is_chore = reserve_price < Decimal::ZERO;

    let message = if reserve_is_chore && value > Decimal::ZERO {
        format!(
            "You've said you'd pay {} to take this chore. Confirm you want \
             to pay rather than be paid.",
            currency.format_amount(value),
        )
    } else if reserve_is_chore && value == Decimal::ZERO {
        "You've said you'd take this chore for free. Confirm you don't \
         want to be paid for it."
            .to_string()
    } else if !reserve_is_chore && value < Decimal::ZERO {
        format!(
            "You've said you'd only accept {} to win this space. This \
             space charges its winner, so a negative value means you \
             won't bid here unless the reserve is changed to a chore.",
            currency.format_amount(value.abs()),
        )
    } else {
        return html! {};
    };

    html! {
        <div class="mt-3 text-xs text-amber-700 dark:text-amber-400 \
                    bg-amber-50 dark:bg-amber-900/20 border \
                    border-amber-200 dark:border-amber-800 rounded p-2">
            {message}
        </div>
    }
}
