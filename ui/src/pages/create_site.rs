use payloads::{
    ActivityRuleParams, AuctionParams, ClientError, CommunityId, Site,
    responses::CommunityWithRole,
};
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::{Route, components::CommunityPageWrapper, hooks::use_sites};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
}

#[function_component]
pub fn CreateSitePage(props: &Props) -> Html {
    let render_content = Callback::from(|community: CommunityWithRole| {
        html! { <CreateSiteForm community={community} /> }
    });

    html! {
        <CommunityPageWrapper
            community_id={props.community_id}
            children={render_content}
        />
    }
}

#[derive(Properties, PartialEq)]
pub struct CreateSiteFormProps {
    pub community: CommunityWithRole,
}

#[function_component]
pub fn CreateSiteForm(props: &CreateSiteFormProps) -> Html {
    let navigator = use_navigator().unwrap();
    let sites_hook = use_sites(props.community.id);
    let community_id = props.community.id;

    // Get user's detected timezone
    let user_timezone = jiff::tz::TimeZone::system()
        .iana_name()
        .unwrap_or("UTC")
        .to_string();

    let name_ref = use_node_ref();
    let description_ref = use_node_ref();
    let timezone_ref = use_node_ref();
    let use_timezone_ref = use_node_ref();

    let error_message = use_state(|| None::<String>);
    let is_loading = use_state(|| false);
    let use_timezone = use_state(|| true); // Default to enabled

    let on_submit = {
        let name_ref = name_ref.clone();
        let description_ref = description_ref.clone();
        let timezone_ref = timezone_ref.clone();
        let use_timezone_ref = use_timezone_ref.clone();
        let error_message = error_message.clone();
        let is_loading = is_loading.clone();
        let navigator = navigator.clone();
        let refetch_sites = sites_hook.refetch.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let name_input = name_ref.cast::<HtmlInputElement>().unwrap();
            let name = name_input.value().trim().to_string();

            if name.is_empty() {
                error_message.set(Some("Please enter a site name".to_string()));
                return;
            }

            let description_input =
                description_ref.cast::<HtmlInputElement>().unwrap();
            let description_value = description_input.value();
            let description = description_value.trim();
            let description = if description.is_empty() {
                None
            } else {
                Some(description.to_string())
            };

            let use_timezone_checkbox =
                use_timezone_ref.cast::<HtmlInputElement>().unwrap();
            let timezone = if use_timezone_checkbox.checked() {
                let timezone_select =
                    timezone_ref.cast::<HtmlSelectElement>().unwrap();
                Some(timezone_select.value())
            } else {
                None
            };

            // Create site object with sensible defaults
            let site = Site {
                community_id,
                name,
                description,
                default_auction_params: AuctionParams {
                    round_duration: jiff::Span::new().minutes(5),
                    bid_increment: rust_decimal::Decimal::new(100, 2), // $1.00
                    activity_rule_params: ActivityRuleParams {
                        eligibility_progression: vec![
                            (0, 0.5),
                            (10, 0.75),
                            (20, 0.9),
                            (30, 1.0),
                        ],
                    },
                },
                // Default values for MVP - auctions will be manually created
                possession_period: jiff::Span::new().days(7), // Default 7 days
                auction_lead_time: jiff::Span::new().hours(24), // Default 24 hours
                proxy_bidding_lead_time: jiff::Span::new().hours(12), // Default 12 hours
                open_hours: None,
                auto_schedule: false, // MVP uses manual auction creation
                timezone,
                site_image_id: None,
            };

            let error_message = error_message.clone();
            let is_loading = is_loading.clone();
            let navigator = navigator.clone();
            let refetch_sites = refetch_sites.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error_message.set(None);

                let api_client = crate::get_api_client();
                match api_client.create_site(&site).await {
                    Ok(site_id) => {
                        // Refresh sites in global state
                        refetch_sites.emit(());
                        // Navigate to site detail page
                        navigator.push(&Route::SiteDetail { id: site_id });
                    }
                    Err(ClientError::APIError(_, msg)) => {
                        error_message.set(Some(msg));
                    }
                    Err(ClientError::Network(_)) => {
                        error_message.set(Some(
                            "Network error. Please check your connection."
                                .to_string(),
                        ));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    let on_timezone_toggle = {
        let use_timezone = use_timezone.clone();
        Callback::from(move |e: Event| {
            let target = e.target().unwrap();
            let input = target.dyn_into::<web_sys::HtmlInputElement>().unwrap();
            use_timezone.set(input.checked());
        })
    };

    let on_cancel = {
        let navigator = navigator.clone();
        Callback::from(move |_| {
            navigator.push(&Route::CommunityDetail { id: community_id });
        })
    };

    html! {
        <div class="max-w-2xl mx-auto py-8 px-4">
            <div class="bg-white dark:bg-neutral-800 p-8 rounded-lg shadow-md">
                <div class="mb-8 text-center">
                    <h1 class="text-2xl font-bold text-neutral-900 dark:text-neutral-100 mb-2">
                        {"Create New Site"}
                    </h1>
                    <p class="text-neutral-600 dark:text-neutral-400">
                        {"Set up a new site for your community."}
                    </p>
                </div>

                <form onsubmit={on_submit} class="space-y-6">
                    if let Some(error) = &*error_message {
                        <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                            <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
                        </div>
                    }

                    // Basic Information Section
                    <div class="space-y-4">
                        <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 border-b border-neutral-200 dark:border-neutral-700 pb-2">
                            {"Basic Information"}
                        </h3>

                        <div>
                            <label for="site-name" class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                                {"Site Name *"}
                            </label>
                            <input
                                ref={name_ref}
                                type="text"
                                id="site-name"
                                name="name"
                                required={true}
                                class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                       rounded-md shadow-sm bg-white dark:bg-neutral-700 
                                       text-neutral-900 dark:text-neutral-100
                                       focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                       dark:focus:ring-neutral-400 dark:focus:border-neutral-400"
                                placeholder="Enter site name"
                            />
                        </div>

                        <div>
                            <label for="site-description" class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                                {"Description"}
                            </label>
                            <input
                                ref={description_ref}
                                type="text"
                                id="site-description"
                                name="description"
                                class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                       rounded-md shadow-sm bg-white dark:bg-neutral-700 
                                       text-neutral-900 dark:text-neutral-100
                                       focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                       dark:focus:ring-neutral-400 dark:focus:border-neutral-400"
                                placeholder="Optional description"
                            />
                        </div>
                    </div>


                    // Optional Settings Section
                    <div class="space-y-4">
                        <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 border-b border-neutral-200 dark:border-neutral-700 pb-2">
                            {"Optional Settings"}
                        </h3>

                        <div class="space-y-3">
                            <div class="flex items-center">
                                <input
                                    ref={use_timezone_ref}
                                    type="checkbox"
                                    id="use-timezone"
                                    name="use_timezone"
                                    checked={*use_timezone}
                                    onchange={on_timezone_toggle}
                                    class="h-4 w-4 text-neutral-600 focus:ring-neutral-500 border-neutral-300 dark:border-neutral-600 rounded"
                                />
                                <label for="use-timezone" class="ml-2 text-sm font-medium text-neutral-700 dark:text-neutral-300">
                                    {"Set a timezone for this site"}
                                </label>
                            </div>

                            <div>
                                <label for="timezone" class={classes!("block", "text-sm", "font-medium", "mb-2", if *use_timezone { "text-neutral-700 dark:text-neutral-300" } else { "text-neutral-400 dark:text-neutral-500" })}>
                                    {"Timezone"}
                                </label>
                                <select
                                    ref={timezone_ref}
                                    id="timezone"
                                    name="timezone"
                                    disabled={!*use_timezone}
                                    class={classes!(
                                        "w-full", "px-3", "py-2", "border", "rounded-md", "shadow-sm",
                                        "focus:outline-none", "focus:ring-2", "focus:ring-neutral-500", "focus:border-neutral-500",
                                        "dark:focus:ring-neutral-400", "dark:focus:border-neutral-400",
                                        if *use_timezone {
                                            "border-neutral-300 dark:border-neutral-600 bg-white dark:bg-neutral-700 text-neutral-900 dark:text-neutral-100"
                                        } else {
                                            "border-neutral-200 dark:border-neutral-700 bg-neutral-50 dark:bg-neutral-800 text-neutral-400 dark:text-neutral-500 cursor-not-allowed"
                                        }
                                    )}
                                >
                                    {jiff::tz::db().available().map(|tz_name| {
                                        let tz_string = tz_name.to_string();
                                        let is_selected = tz_string == user_timezone;
                                        html! {
                                            <option value={tz_string.clone()} selected={is_selected}>{tz_string}</option>
                                        }
                                    }).collect::<Html>()}
                                </select>
                                <p class={classes!("text-xs", "mt-1", if *use_timezone { "text-neutral-500 dark:text-neutral-400" } else { "text-neutral-400 dark:text-neutral-500" })}>
                                    {"Timezone for this site (defaults to your detected timezone)"}
                                </p>
                            </div>
                        </div>
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
                            if *is_loading {
                                {"Creating Site..."}
                            } else {
                                {"Create Site"}
                            }
                        </button>
                    </div>
                </form>
            </div>
        </div>
    }
}
