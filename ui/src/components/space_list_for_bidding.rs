use payloads::{RoundSpaceResult, SpaceId, responses};
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use yew::prelude::*;

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
    pub current_eligibility: f64,
    pub eligibility_threshold: f64,
    pub on_bid: Callback<SpaceId>,
    pub on_update_value: Callback<(SpaceId, Decimal)>,
}

#[function_component]
pub fn SpaceListForBidding(props: &Props) -> Html {
    let sort_field = use_state(|| SortField::Name);
    let sort_direction = use_state(|| SortDirection::Ascending);
    let filter_no_value = use_state(|| true);

    // Create price lookup
    let price_map: HashMap<SpaceId, Decimal> =
        props.prices.iter().map(|r| (r.space_id, r.value)).collect();

    // Prepare space data
    let mut space_data: Vec<_> = props
        .spaces
        .iter()
        .map(|space| {
            let space_id = space.space_id;
            let price =
                price_map.get(&space_id).copied().unwrap_or(Decimal::ZERO);
            let user_value = props.user_values.get(&space_id).copied();
            let surplus = user_value.map(|v| v - price);

            (space, price, user_value, surplus)
        })
        .collect();

    // Apply filters
    space_data.retain(|(_, _, user_value, _)| {
        if *filter_no_value {
            user_value.is_some()
        } else {
            true
        }
    });

    // Sort (None values sort last for UserValue and Surplus)
    space_data.sort_by(|a, b| {
        let comparison = match *sort_field {
            SortField::Name => {
                a.0.space_details.name.cmp(&b.0.space_details.name)
            }
            SortField::Price => a.1.cmp(&b.1),
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

    // Calculate minimum points needed to maintain eligibility
    let min_points_needed =
        props.current_eligibility * props.eligibility_threshold;

    html! {
        <div class="space-y-4">
            <div class="flex items-center justify-between">
                <h3 class="text-lg font-semibold text-neutral-900 \
                           dark:text-white">
                    {"Spaces"}
                </h3>
            </div>

            // Eligibility requirement message
            {if min_points_needed > 0.0 {
                html! {
                    <div class="p-3 bg-neutral-100 dark:bg-neutral-800 \
                                rounded-md border border-neutral-200 \
                                dark:border-neutral-700">
                        <p class="text-sm text-neutral-700 dark:text-neutral-300">
                            {"To maintain your current eligibility, bid on spaces \
                             totaling "}
                            <span class="font-semibold">
                                {format!("{:.1}", min_points_needed)}
                            </span>
                            {" points or more."}
                        </p>
                    </div>
                }
            } else {
                html! {}
            }}

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
                    space_data.iter().map(|(space, price, user_value, surplus)| {
                        let user_has_bid = props.user_bid_space_ids.contains(&space.space_id);
                        html! {
                            <SpaceRow
                                key={space.space_id.0.to_string()}
                                space={(*space).clone()}
                                price={*price}
                                user_value={*user_value}
                                surplus={*surplus}
                                proxy_bidding_enabled={props.proxy_bidding_enabled}
                                user_has_bid={user_has_bid}
                                on_bid={props.on_bid.clone()}
                                on_update_value={props.on_update_value.clone()}
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
    price: Decimal,
    user_value: Option<Decimal>,
    surplus: Option<Decimal>,
    proxy_bidding_enabled: bool,
    user_has_bid: bool,
    on_bid: Callback<SpaceId>,
    on_update_value: Callback<(SpaceId, Decimal)>,
}

#[function_component]
fn SpaceRow(props: &SpaceRowProps) -> Html {
    let space_id = props.space.space_id;

    let on_bid_click = {
        let on_bid = props.on_bid.clone();
        Callback::from(move |_| {
            on_bid.emit(space_id);
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
                        {format!("${:.2}", props.price)}
                    </div>
                </div>

                <div>
                    <div class="text-xs text-neutral-500 \
                                dark:text-neutral-400">
                        {"Your Value"}
                    </div>
                    <div class="text-sm font-medium text-neutral-900 \
                                dark:text-white">
                        {match props.user_value {
                            Some(value) => format!("${:.2}", value),
                            None => "$--".to_string(),
                        }}
                    </div>
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
                    {if props.user_has_bid {
                        html! {
                            <span class="text-xs text-neutral-600 \
                                         dark:text-neutral-400 font-medium">
                                {"Already bid"}
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
                                {"Bid"}
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
