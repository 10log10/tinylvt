use jiff::Timestamp;
use payloads::{SiteId, responses};
use yew::prelude::*;

use crate::components::{
    SitePageWrapper, SiteTabHeader, SiteWithRole, site_tab_header::ActiveTab,
};
use crate::hooks::use_auctions;
use crate::utils::time::{format_zoned_timestamp, localize_timestamp};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub site_id: SiteId,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum AuctionStatus {
    Upcoming,
    Ongoing,
    Finished,
}

impl AuctionStatus {
    fn from_auction(auction: &responses::Auction) -> Self {
        let now = Timestamp::now();

        if auction.end_at.is_some() {
            Self::Finished
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
            Self::Finished => "Finished",
        }
    }

    fn badge_classes(&self) -> &'static str {
        match self {
            Self::Upcoming => {
                "bg-neutral-100 text-neutral-800 dark:bg-neutral-800 dark:text-neutral-200"
            }
            Self::Ongoing => {
                "bg-neutral-800 text-white dark:bg-neutral-200 dark:text-neutral-900"
            }
            Self::Finished => {
                "bg-neutral-300 text-neutral-600 dark:bg-neutral-600 dark:text-neutral-400"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SortField {
    AuctionStart,
    AuctionEnd,
    PossessionStart,
    PossessionEnd,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SortDirection {
    Ascending,
    Descending,
}

#[function_component]
pub fn SiteAuctionsPage(props: &Props) -> Html {
    let render_content = Callback::from(|site_with_role: SiteWithRole| {
        html! {
            <div>
                <SiteTabHeader
                    site={site_with_role.site.clone()}
                    active_tab={ActiveTab::Auctions}
                />
                <div class="py-6">
                    <AuctionsTab site={site_with_role.site} />
                </div>
            </div>
        }
    });

    html! {
        <SitePageWrapper
            site_id={props.site_id}
            children={render_content}
        />
    }
}

#[derive(Properties, PartialEq)]
pub struct AuctionsTabProps {
    pub site: responses::Site,
}

#[function_component]
fn AuctionsTab(props: &AuctionsTabProps) -> Html {
    let site_id = props.site.site_id;
    let auctions_hook = use_auctions(site_id);
    let sort_field = use_state(|| SortField::AuctionStart);
    let sort_direction = use_state(|| SortDirection::Descending);
    let filter_upcoming = use_state(|| true);
    let filter_ongoing = use_state(|| true);
    let filter_finished = use_state(|| true);

    let on_sort_field_change = {
        let sort_field = sort_field.clone();
        let sort_direction = sort_direction.clone();
        let current_field = *sort_field;

        Callback::from(move |new_field: SortField| {
            if current_field == new_field {
                // Toggle direction if clicking same field
                sort_direction.set(match *sort_direction {
                    SortDirection::Ascending => SortDirection::Descending,
                    SortDirection::Descending => SortDirection::Ascending,
                });
            } else {
                // Set new field with default descending
                sort_field.set(new_field);
                sort_direction.set(SortDirection::Descending);
            }
        })
    };

    let on_filter_upcoming_toggle = {
        let filter_upcoming = filter_upcoming.clone();
        Callback::from(move |_| {
            filter_upcoming.set(!*filter_upcoming);
        })
    };

    let on_filter_ongoing_toggle = {
        let filter_ongoing = filter_ongoing.clone();
        Callback::from(move |_| {
            filter_ongoing.set(!*filter_ongoing);
        })
    };

    let on_filter_finished_toggle = {
        let filter_finished = filter_finished.clone();
        Callback::from(move |_| {
            filter_finished.set(!*filter_finished);
        })
    };

    let auctions_content = if auctions_hook.is_loading {
        html! {
            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400">
                    {"Loading auctions..."}
                </p>
            </div>
        }
    } else if let Some(error) = &auctions_hook.error {
        html! {
            <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
            </div>
        }
    } else {
        match &auctions_hook.auctions {
            Some(auctions) => {
                if auctions.is_empty() {
                    html! {
                        <div class="text-center py-12">
                            <p class="text-neutral-600 dark:text-neutral-400 mb-4">
                                {"No auctions have been created for this site yet."}
                            </p>
                        </div>
                    }
                } else {
                    // Filter auctions by status
                    let mut filtered_auctions: Vec<_> = auctions
                        .iter()
                        .filter(|auction| {
                            let status = AuctionStatus::from_auction(auction);
                            match status {
                                AuctionStatus::Upcoming => *filter_upcoming,
                                AuctionStatus::Ongoing => *filter_ongoing,
                                AuctionStatus::Finished => *filter_finished,
                            }
                        })
                        .collect();

                    // Sort auctions
                    filtered_auctions.sort_by(|a, b| {
                        let comparison = match *sort_field {
                            SortField::AuctionStart => a
                                .auction_details
                                .start_at
                                .cmp(&b.auction_details.start_at),
                            SortField::AuctionEnd => {
                                match (a.end_at, b.end_at) {
                                    (Some(a_end), Some(b_end)) => {
                                        a_end.cmp(&b_end)
                                    }
                                    (Some(_), None) => std::cmp::Ordering::Less,
                                    (None, Some(_)) => {
                                        std::cmp::Ordering::Greater
                                    }
                                    (None, None) => a
                                        .auction_details
                                        .start_at
                                        .cmp(&b.auction_details.start_at),
                                }
                            }
                            SortField::PossessionStart => a
                                .auction_details
                                .possession_start_at
                                .cmp(&b.auction_details.possession_start_at),
                            SortField::PossessionEnd => a
                                .auction_details
                                .possession_end_at
                                .cmp(&b.auction_details.possession_end_at),
                        };

                        match *sort_direction {
                            SortDirection::Ascending => comparison,
                            SortDirection::Descending => comparison.reverse(),
                        }
                    });

                    html! {
                        <div>
                            <div class="flex justify-between items-center mb-6">
                                <h2 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100">
                                    {"Auctions"}
                                </h2>
                            </div>

                            // Filters and Sorting Controls
                            <div class="mb-6 space-y-4">
                                // Filter Controls
                                <div class="flex gap-4 items-center">
                                    <span class="text-sm font-medium text-neutral-700 dark:text-neutral-300">
                                        {"Show:"}
                                    </span>
                                    {[
                                        ("Upcoming", *filter_upcoming, on_filter_upcoming_toggle),
                                        ("Ongoing", *filter_ongoing, on_filter_ongoing_toggle),
                                        ("Finished", *filter_finished, on_filter_finished_toggle),
                                    ].iter().map(|(label, checked, callback)| {
                                        html! {
                                            <label class="flex items-center gap-2 cursor-pointer select-none">
                                                <input
                                                    type="checkbox"
                                                    checked={*checked}
                                                    onchange={callback.clone()}
                                                    class="h-4 w-4 text-neutral-600 focus:ring-neutral-500 border-neutral-300 dark:border-neutral-600 rounded"
                                                />
                                                <span class="text-sm text-neutral-700 dark:text-neutral-300">
                                                    {*label}
                                                </span>
                                            </label>
                                        }
                                    }).collect::<Html>()}
                                </div>

                                // Sort Controls
                                <div class="flex gap-4 items-center">
                                    <span class="text-sm font-medium text-neutral-700 dark:text-neutral-300">
                                        {"Sort by:"}
                                    </span>
                                    <SortButton
                                        label="Auction Start"
                                        field={SortField::AuctionStart}
                                        current_field={*sort_field}
                                        current_direction={*sort_direction}
                                        on_click={on_sort_field_change.clone()}
                                    />
                                    <SortButton
                                        label="Auction End"
                                        field={SortField::AuctionEnd}
                                        current_field={*sort_field}
                                        current_direction={*sort_direction}
                                        on_click={on_sort_field_change.clone()}
                                    />
                                    <SortButton
                                        label="Possession Start"
                                        field={SortField::PossessionStart}
                                        current_field={*sort_field}
                                        current_direction={*sort_direction}
                                        on_click={on_sort_field_change.clone()}
                                    />
                                    <SortButton
                                        label="Possession End"
                                        field={SortField::PossessionEnd}
                                        current_field={*sort_field}
                                        current_direction={*sort_direction}
                                        on_click={on_sort_field_change.clone()}
                                    />
                                </div>
                            </div>

                            // Auctions List
                            {if filtered_auctions.is_empty() {
                                html! {
                                    <div class="text-center py-12">
                                        <p class="text-neutral-600 dark:text-neutral-400">
                                            {"No auctions match the selected filters."}
                                        </p>
                                    </div>
                                }
                            } else {
                                html! {
                                    <div class="space-y-4">
                                        {filtered_auctions.iter().map(|auction| {
                                            html! {
                                                <AuctionCard
                                                    key={auction.auction_id.to_string()}
                                                    auction={(*auction).clone()}
                                                    site={props.site.clone()}
                                                />
                                            }
                                        }).collect::<Html>()}
                                    </div>
                                }
                            }}
                        </div>
                    }
                }
            }
            None => {
                html! {
                    <div class="text-center py-12">
                        <p class="text-neutral-600 dark:text-neutral-400">
                            {"No auctions data available"}
                        </p>
                    </div>
                }
            }
        }
    };

    html! {
        <>{auctions_content}</>
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
    let on_click = {
        let field = props.field;
        let on_click = props.on_click.clone();
        Callback::from(move |_| {
            on_click.emit(field);
        })
    };

    let arrow = if is_active {
        match props.current_direction {
            SortDirection::Ascending => " ↑",
            SortDirection::Descending => " ↓",
        }
    } else {
        ""
    };

    let classes = if is_active {
        "px-3 py-1 text-sm font-medium bg-neutral-900 text-white dark:bg-neutral-100 dark:text-neutral-900 rounded-md cursor-pointer hover:bg-neutral-800 dark:hover:bg-neutral-200 transition-colors"
    } else {
        "px-3 py-1 text-sm font-medium bg-neutral-200 text-neutral-700 dark:bg-neutral-700 dark:text-neutral-300 rounded-md cursor-pointer hover:bg-neutral-300 dark:hover:bg-neutral-600 transition-colors"
    };

    html! {
        <button onclick={on_click} class={classes}>
            {props.label}{arrow}
        </button>
    }
}

#[derive(Properties, PartialEq)]
struct AuctionCardProps {
    auction: responses::Auction,
    site: responses::Site,
}

#[function_component]
fn AuctionCard(props: &AuctionCardProps) -> Html {
    let auction_details = &props.auction.auction_details;
    let site_details = &props.site.site_details;
    let site_timezone = site_details.timezone.as_deref();
    let status = AuctionStatus::from_auction(&props.auction);

    // Format timestamps
    let possession_start = format_zoned_timestamp(&localize_timestamp(
        auction_details.possession_start_at,
        site_timezone,
    ));

    let possession_end = format_zoned_timestamp(&localize_timestamp(
        auction_details.possession_end_at,
        site_timezone,
    ));

    let auction_start = format_zoned_timestamp(&localize_timestamp(
        auction_details.start_at,
        site_timezone,
    ));

    let auction_end = props.auction.end_at.map(|end_at| {
        format_zoned_timestamp(&localize_timestamp(end_at, site_timezone))
    });

    html! {
        <div class="bg-white dark:bg-neutral-800 p-6 rounded-lg shadow-md border border-neutral-200 dark:border-neutral-700">
            <div class="flex justify-between items-start mb-4">
                <div class="flex-1">
                    <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100">
                        {"Auction for "}{&site_details.name}
                    </h3>
                </div>
                <span class={format!("px-3 py-1 rounded-full text-xs font-medium {}", status.badge_classes())}>
                    {status.label()}
                </span>
            </div>

            <div class="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm">
                <div>
                    <h4 class="font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                        {"Possession Period"}
                    </h4>
                    <div class="space-y-1 text-neutral-600 dark:text-neutral-400">
                        <p>{"Start: "}{possession_start}</p>
                        <p>{"End: "}{possession_end}</p>
                    </div>
                </div>

                <div>
                    <h4 class="font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                        {"Auction Times"}
                    </h4>
                    <div class="space-y-1 text-neutral-600 dark:text-neutral-400">
                        <p>{"Start: "}{auction_start}</p>
                        {if let Some(end) = auction_end {
                            html! {<p>{"End: "}{end}</p>}
                        } else {
                            html! {<p>{"End: In progress"}</p>}
                        }}
                    </div>
                </div>
            </div>
        </div>
    }
}
