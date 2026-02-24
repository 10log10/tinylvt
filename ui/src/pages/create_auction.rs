use jiff::Timestamp;
use payloads::{Auction, AuctionParams, SiteId};
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::{
    Route,
    components::{AuctionParamsEditor, SitePageWrapper, SiteWithRole},
    hooks::{use_auctions, use_push_route},
};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub site_id: SiteId,
}

#[function_component]
pub fn CreateAuctionPage(props: &Props) -> Html {
    let render_content = Callback::from(|site_with_role: SiteWithRole| {
        html! { <CreateAuctionForm site_with_role={site_with_role} /> }
    });

    html! {
        <SitePageWrapper
            site_id={props.site_id}
            children={render_content}
        />
    }
}

#[derive(Properties, PartialEq)]
pub struct CreateAuctionFormProps {
    pub site_with_role: SiteWithRole,
}

#[function_component]
pub fn CreateAuctionForm(props: &CreateAuctionFormProps) -> Html {
    let push_route = use_push_route();
    let auctions_hook = use_auctions(props.site_with_role.site.site_id);
    let site_id = props.site_with_role.site.site_id;
    let site_details = &props.site_with_role.site.site_details;

    let auction_start_ref = use_node_ref();
    let possession_start_ref = use_node_ref();
    let possession_end_ref = use_node_ref();
    let use_site_timezone_ref = use_node_ref();

    let error_message = use_state(|| None::<String>);
    let is_loading = use_state(|| false);

    // Default to site timezone for auction start if available
    let use_site_timezone_for_auction =
        use_state(|| site_details.timezone.is_some());

    // Default to site's default auction params
    let auction_params = use_state(|| {
        props
            .site_with_role
            .site
            .site_details
            .default_auction_params
            .clone()
    });

    let on_auction_params_change = {
        let auction_params = auction_params.clone();
        Callback::from(move |updated: AuctionParams| {
            auction_params.set(updated);
        })
    };

    let on_auction_timezone_toggle = {
        let use_site_timezone_for_auction =
            use_site_timezone_for_auction.clone();
        Callback::from(move |e: Event| {
            let target = e.target().unwrap();
            let input = target.dyn_into::<web_sys::HtmlInputElement>().unwrap();
            use_site_timezone_for_auction.set(input.checked());
        })
    };

    let on_submit = {
        let auction_start_ref = auction_start_ref.clone();
        let possession_start_ref = possession_start_ref.clone();
        let possession_end_ref = possession_end_ref.clone();
        let use_site_timezone_ref = use_site_timezone_ref.clone();
        let auction_params = auction_params.clone();
        let error_message = error_message.clone();
        let is_loading = is_loading.clone();
        let push_route = push_route.clone();
        let refetch_auctions = auctions_hook.refetch.clone();
        let site_timezone = site_details.timezone.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let auction_start_input =
                auction_start_ref.cast::<HtmlInputElement>().unwrap();
            let auction_start_str = auction_start_input.value();

            let possession_start_input =
                possession_start_ref.cast::<HtmlInputElement>().unwrap();
            let possession_start_str = possession_start_input.value();

            let possession_end_input =
                possession_end_ref.cast::<HtmlInputElement>().unwrap();
            let possession_end_str = possession_end_input.value();

            let use_site_tz_checkbox =
                use_site_timezone_ref.cast::<HtmlInputElement>().unwrap();
            let use_site_tz_for_auction = use_site_tz_checkbox.checked();

            // Determine timezone for auction start
            let auction_timezone = if use_site_tz_for_auction {
                site_timezone.as_deref()
            } else {
                None
            };

            // Possession times always use site timezone if available
            let possession_timezone = site_timezone.as_deref();

            // Parse datetime-local strings to Timestamps
            let auction_start = match parse_datetime_local(
                &auction_start_str,
                auction_timezone,
            ) {
                Ok(ts) => ts,
                Err(e) => {
                    error_message.set(Some(format!(
                        "Invalid auction start time: {}",
                        e
                    )));
                    return;
                }
            };

            let possession_start = match parse_datetime_local(
                &possession_start_str,
                possession_timezone,
            ) {
                Ok(ts) => ts,
                Err(e) => {
                    error_message.set(Some(format!(
                        "Invalid possession start time: {}",
                        e
                    )));
                    return;
                }
            };

            let possession_end = match parse_datetime_local(
                &possession_end_str,
                possession_timezone,
            ) {
                Ok(ts) => ts,
                Err(e) => {
                    error_message.set(Some(format!(
                        "Invalid possession end time: {}",
                        e
                    )));
                    return;
                }
            };

            // Validate times
            let now = Timestamp::now();

            if auction_start <= now {
                error_message.set(Some(
                    "Auction start time must be in the future".to_string(),
                ));
                return;
            }

            if possession_start >= possession_end {
                error_message.set(Some(
                    "Possession start must be before possession end"
                        .to_string(),
                ));
                return;
            }

            if auction_start > possession_start {
                error_message.set(Some(
                    "Auction start must be at or before possession start"
                        .to_string(),
                ));
                return;
            }

            let auction = Auction {
                site_id,
                possession_start_at: possession_start,
                possession_end_at: possession_end,
                start_at: auction_start,
                auction_params: (*auction_params).clone(),
            };

            let error_message = error_message.clone();
            let is_loading = is_loading.clone();
            let push_route = push_route.clone();
            let refetch_auctions = refetch_auctions.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error_message.set(None);

                let api_client = crate::get_api_client();
                match api_client.create_auction(&auction).await {
                    Ok(_auction_id) => {
                        // Refresh auctions in global state
                        refetch_auctions.emit(());
                        // Navigate to auctions page
                        push_route.emit(Route::SiteAuctions { id: site_id });
                    }
                    Err(e) => {
                        error_message.set(Some(e.to_string()));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    let on_cancel = {
        let push_route = push_route.clone();
        Callback::from(move |_| {
            push_route.emit(Route::SiteOverview { id: site_id });
        })
    };

    // Determine if we should show timezone toggle for auction start
    let user_tz = jiff::tz::TimeZone::system();
    let user_tz_name = user_tz.iana_name().unwrap_or("UTC");
    let show_timezone_toggle = site_details
        .timezone
        .as_deref()
        .map(|site_tz| site_tz != user_tz_name)
        .unwrap_or(false);

    html! {
        <div class="max-w-2xl mx-auto py-8 px-4">
            <div class="bg-white dark:bg-neutral-800 p-8 rounded-lg shadow-md">
                <div class="mb-8 text-center">
                    <h1 class="text-2xl font-bold text-neutral-900 dark:text-neutral-100 mb-2">
                        {"Create New Auction"}
                    </h1>
                    <p class="text-neutral-600 dark:text-neutral-400">
                        {"Schedule a new auction for "}{&props.site_with_role.site.site_details.name}
                    </p>
                </div>

                <form onsubmit={on_submit} class="space-y-6">
                    {if let Some(error) = &*error_message {
                        html! {
                            <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                                <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
                            </div>
                        }
                    } else {
                        html! {}
                    }}

                    // Possession Period Section
                    <div class="space-y-4">
                        <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 border-b border-neutral-200 dark:border-neutral-700 pb-2">
                            {"Possession Period"}
                        </h3>

                        <div>
                            <label for="possession-start" class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                                {"Start Time *"}
                            </label>
                            <input
                                ref={possession_start_ref}
                                type="datetime-local"
                                id="possession-start"
                                name="possession_start"
                                required={true}
                                class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                       rounded-md shadow-sm bg-white dark:bg-neutral-700
                                       text-neutral-900 dark:text-neutral-100
                                       focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                       dark:focus:ring-neutral-400 dark:focus:border-neutral-400"
                            />
                            <p class="text-xs text-neutral-500 dark:text-neutral-400 mt-1">
                                {"When possession period begins"}
                            </p>
                        </div>

                        <div>
                            <label for="possession-end" class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                                {"End Time *"}
                            </label>
                            <input
                                ref={possession_end_ref}
                                type="datetime-local"
                                id="possession-end"
                                name="possession_end"
                                required={true}
                                class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                       rounded-md shadow-sm bg-white dark:bg-neutral-700
                                       text-neutral-900 dark:text-neutral-100
                                       focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                       dark:focus:ring-neutral-400 dark:focus:border-neutral-400"
                            />
                            <p class="text-xs text-neutral-500 dark:text-neutral-400 mt-1">
                                {"When possession period ends"}
                            </p>
                        </div>

                        <p class="text-xs text-neutral-500 dark:text-neutral-400 bg-neutral-50 dark:bg-neutral-900 p-3 rounded border border-neutral-200 dark:border-neutral-700">
                            {if let Some(tz) = &site_details.timezone {
                                format!("Possession times are always interpreted in the site's timezone ({}), as they pertain to physical coordination at the site location.", tz)
                            } else {
                                "No site timezone set - possession times will be interpreted in your local timezone.".to_string()
                            }}
                        </p>
                    </div>

                    // Auction Start Section
                    <div class="space-y-4">
                        <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 border-b border-neutral-200 dark:border-neutral-700 pb-2">
                            {"Auction Start"}
                        </h3>

                        <div>
                            <label for="auction-start" class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                                {"Start Time *"}
                            </label>
                            <input
                                ref={auction_start_ref}
                                type="datetime-local"
                                id="auction-start"
                                name="auction_start"
                                required={true}
                                class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                       rounded-md shadow-sm bg-white dark:bg-neutral-700
                                       text-neutral-900 dark:text-neutral-100
                                       focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                       dark:focus:ring-neutral-400 dark:focus:border-neutral-400"
                            />
                            <p class="text-xs text-neutral-500 dark:text-neutral-400 mt-1">
                                {"When the auction will begin"}
                            </p>
                        </div>

                        {if show_timezone_toggle {
                            let site_tz = site_details.timezone.as_ref().unwrap();
                            html! {
                                <div class="space-y-3 pt-2">
                                    <div class="flex items-center">
                                        <input
                                            ref={use_site_timezone_ref.clone()}
                                            type="checkbox"
                                            id="use-site-timezone-auction"
                                            name="use_site_timezone_auction"
                                            checked={*use_site_timezone_for_auction}
                                            onchange={on_auction_timezone_toggle.clone()}
                                            class="h-4 w-4 text-neutral-600 focus:ring-neutral-500 border-neutral-300 dark:border-neutral-600 rounded"
                                        />
                                        <label for="use-site-timezone-auction" class="ml-2 text-sm font-medium text-neutral-700 dark:text-neutral-300">
                                            {format!("Interpret in site timezone ({})", site_tz)}
                                        </label>
                                    </div>
                                    {if *use_site_timezone_for_auction {
                                        html! {
                                            <p class="text-xs text-neutral-600 dark:text-neutral-400 bg-neutral-50 dark:bg-neutral-900 p-2 rounded">
                                                {format!("Time will be interpreted in {} timezone", site_tz)}
                                            </p>
                                        }
                                    } else {
                                        html! {
                                            <p class="text-xs text-neutral-600 dark:text-neutral-400 bg-neutral-50 dark:bg-neutral-900 p-2 rounded">
                                                {format!("Time will be interpreted in your local timezone ({})", user_tz_name)}
                                            </p>
                                        }
                                    }}
                                </div>
                            }
                        } else {
                            html! {
                                <>
                                    // Hidden checkbox to maintain form structure
                                    <input
                                        ref={use_site_timezone_ref.clone()}
                                        type="checkbox"
                                        checked={*use_site_timezone_for_auction}
                                        class="hidden"
                                    />
                                    <p class="text-xs text-neutral-500 dark:text-neutral-400 bg-neutral-50 dark:bg-neutral-900 p-3 rounded border border-neutral-200 dark:border-neutral-700">
                                        {if site_details.timezone.is_some() {
                                            format!("Your timezone matches the site timezone ({})", user_tz_name)
                                        } else {
                                            format!("Time will be interpreted in your local timezone ({})", user_tz_name)
                                        }}
                                    </p>
                                </>
                            }
                        }}
                    </div>

                    // Auction Parameters Section
                    <div class="space-y-4">
                        <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 border-b border-neutral-200 dark:border-neutral-700 pb-2">
                            {"Auction Parameters"}
                        </h3>

                        <AuctionParamsEditor
                            auction_params={(*auction_params).clone()}
                            on_change={on_auction_params_change}
                            disabled={*is_loading}
                        />
                    </div>

                    <div class="flex space-x-3 pt-6 border-t border-neutral-200 dark:border-neutral-700">
                        <button
                            type="button"
                            onclick={on_cancel}
                            disabled={*is_loading}
                            class="flex-1 py-2 px-4 border border-neutral-300 dark:border-neutral-600
                                   rounded-md shadow-sm text-sm font-medium text-neutral-700 dark:text-neutral-300
                                   bg-white dark:bg-neutral-700 hover:bg-neutral-50 dark:hover:bg-neutral-600
                                   focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                                   disabled:opacity-50 disabled:cursor-not-allowed
                                   transition-colors duration-200"
                        >
                            {"Cancel"}
                        </button>

                        <button
                            type="submit"
                            disabled={*is_loading}
                            class="flex-1 flex justify-center py-2 px-4 border border-transparent
                                   rounded-md shadow-sm text-sm font-medium text-white
                                   bg-neutral-900 hover:bg-neutral-800
                                   dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200
                                   focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                                   disabled:opacity-50 disabled:cursor-not-allowed
                                   transition-colors duration-200"
                        >
                            {if *is_loading {
                                "Creating Auction..."
                            } else {
                                "Create Auction"
                            }}
                        </button>
                    </div>
                </form>
            </div>
        </div>
    }
}

/// Parse datetime-local input string (YYYY-MM-DDTHH:MM) to Timestamp
/// If timezone is provided, use it; otherwise use system timezone
fn parse_datetime_local(
    s: &str,
    timezone: Option<&str>,
) -> Result<Timestamp, String> {
    // datetime-local format: "2024-01-15T14:30"
    // Parse using jiff's civil datetime
    let civil_dt = jiff::civil::DateTime::strptime("%Y-%m-%dT%H:%M", s)
        .map_err(|e| format!("Failed to parse datetime: {}", e))?;

    // Convert to timestamp in specified or system timezone
    let tz = if let Some(tz_name) = timezone {
        jiff::tz::TimeZone::get(tz_name)
            .map_err(|e| format!("Invalid timezone '{}': {}", tz_name, e))?
    } else {
        jiff::tz::TimeZone::system()
    };

    civil_dt
        .to_zoned(tz)
        .map_err(|e| format!("Failed to convert to zoned datetime: {}", e))
        .map(|zdt| zdt.timestamp())
}
