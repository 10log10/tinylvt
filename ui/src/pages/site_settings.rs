use payloads::{
    AuctionParams, ClientError, Role, Site, SiteId, requests::UpdateSite,
    responses::Site as SiteResponse,
};
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::{
    Route,
    components::{
        AuctionParamsEditor, AuctionParamsViewer, SitePageWrapper,
        SiteTabHeader, SiteWithRole, site_tab_header::ActiveTab,
    },
    hooks::use_site,
};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub site_id: SiteId,
}

#[function_component]
pub fn SiteSettingsPage(props: &Props) -> Html {
    let render_content = Callback::from(move |site_with_role: SiteWithRole| {
        html! {
            <div>
                <SiteTabHeader site={site_with_role.site.clone()} active_tab={ActiveTab::Settings} />
                <div class="py-6">
                    <SiteSettingsForm
                        site={site_with_role.site}
                        user_role={site_with_role.user_role}
                    />
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
pub struct SiteSettingsFormProps {
    pub site: SiteResponse,
    pub user_role: Role,
}

#[function_component]
pub fn SiteSettingsForm(props: &SiteSettingsFormProps) -> Html {
    let navigator = use_navigator().unwrap();
    let site_hook = use_site(props.site.site_id);

    // Get user's detected timezone
    let user_timezone = jiff::tz::TimeZone::system()
        .iana_name()
        .unwrap_or("UTC")
        .to_string();

    let name_ref = use_node_ref();
    let description_ref = use_node_ref();
    let timezone_ref = use_node_ref();
    let use_timezone_ref = use_node_ref();

    let is_editing = use_state(|| false);
    let error_message = use_state(|| None::<String>);
    let success_message = use_state(|| None::<String>);
    let is_loading = use_state(|| false);
    let use_timezone = use_state(|| props.site.site_details.timezone.is_some());
    // Note: AuctionParamsEditor relies on this being use_state so that calling
    // .set() with unchanged params still triggers a re-render, which resets
    // invalid input values back to their correct display values
    let auction_params =
        use_state(|| props.site.site_details.default_auction_params.clone());

    let can_edit = props.user_role.is_ge_coleader();

    let on_auction_params_change = {
        let auction_params = auction_params.clone();
        Callback::from(move |new_params: AuctionParams| {
            auction_params.set(new_params);
        })
    };

    let on_update = {
        let name_ref = name_ref.clone();
        let description_ref = description_ref.clone();
        let timezone_ref = timezone_ref.clone();
        let use_timezone_ref = use_timezone_ref.clone();
        let error_message = error_message.clone();
        let success_message = success_message.clone();
        let is_loading = is_loading.clone();
        let is_editing = is_editing.clone();
        let site = props.site.clone();
        let auction_params = auction_params.clone();
        let refetch_site = site_hook.refetch.clone();

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

            // Create updated site object
            let updated_site = Site {
                community_id: site.site_details.community_id,
                name,
                description,
                default_auction_params: (*auction_params).clone(),
                // Keep existing values for MVP fields
                possession_period: site.site_details.possession_period,
                auction_lead_time: site.site_details.auction_lead_time,
                proxy_bidding_lead_time: site
                    .site_details
                    .proxy_bidding_lead_time,
                open_hours: site.site_details.open_hours.clone(),
                auto_schedule: site.site_details.auto_schedule,
                timezone,
                site_image_id: site.site_details.site_image_id,
            };

            let error_message = error_message.clone();
            let success_message = success_message.clone();
            let is_loading = is_loading.clone();
            let is_editing = is_editing.clone();
            let site_id = site.site_id;
            let refetch_site = refetch_site.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error_message.set(None);
                success_message.set(None);

                let update_request = UpdateSite {
                    site_id,
                    site_details: updated_site,
                };

                let api_client = crate::get_api_client();
                match api_client.update_site(&update_request).await {
                    Ok(_) => {
                        success_message.set(Some(
                            "Site updated successfully!".to_string(),
                        ));
                        // Refresh site data
                        refetch_site.emit(());
                        // Exit edit mode
                        is_editing.set(false);
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

    let on_delete = {
        let error_message = error_message.clone();
        let is_loading = is_loading.clone();
        let navigator = navigator.clone();
        let site = props.site.clone();

        Callback::from(move |_| {
            let confirmed = web_sys::window()
                .unwrap()
                .confirm_with_message(&format!(
                    "Are you sure you want to delete the site '{}'? This action cannot be undone.",
                    site.site_details.name
                ))
                .unwrap_or(false);

            if !confirmed {
                return;
            }

            let error_message = error_message.clone();
            let is_loading = is_loading.clone();
            let navigator = navigator.clone();
            let site_id = site.site_id;
            let community_id = site.site_details.community_id;

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error_message.set(None);

                let api_client = crate::get_api_client();
                match api_client.delete_site(&site_id).await {
                    Ok(_) => {
                        navigator
                            .push(&Route::CommunityDetail { id: community_id });
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

    let on_edit = {
        let is_editing = is_editing.clone();
        Callback::from(move |_| {
            is_editing.set(true);
        })
    };

    let on_cancel_edit = {
        let is_editing = is_editing.clone();
        let auction_params = auction_params.clone();
        let error_message = error_message.clone();
        let success_message = success_message.clone();
        let site = props.site.clone();
        Callback::from(move |_| {
            // Reset to original values
            auction_params
                .set(site.site_details.default_auction_params.clone());
            error_message.set(None);
            success_message.set(None);
            is_editing.set(false);
        })
    };

    let on_back = {
        let navigator = navigator.clone();
        let site_id = props.site.site_id;
        Callback::from(move |_| {
            navigator.push(&Route::SiteDetail { id: site_id });
        })
    };

    html! {
        <div class="max-w-4xl mx-auto py-8 px-4">
            <div class="bg-white dark:bg-neutral-800 p-8 rounded-lg shadow-md">
                <div class="mb-8">
                    <div class="flex justify-between items-start mb-2">
                        <div>
                            <h1 class="text-2xl font-bold text-neutral-900 dark:text-neutral-100 mb-2">
                                {"Site Settings"}
                            </h1>
                            <p class="text-neutral-600 dark:text-neutral-400">
                                {if *is_editing {
                                    "Edit site details and auction parameters"
                                } else {
                                    "View site details and auction parameters"
                                }}
                            </p>
                        </div>
                        {if !*is_editing && can_edit {
                            html! {
                                <button
                                    onclick={on_edit}
                                    class="py-2 px-4 border border-transparent
                                           rounded-md shadow-sm text-sm font-medium text-white
                                           bg-neutral-900 hover:bg-neutral-800
                                           dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200
                                           focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                                           transition-colors duration-200"
                                >
                                    {"Edit Settings"}
                                </button>
                            }
                        } else {
                            html! {}
                        }}
                    </div>
                </div>

                if *is_editing {
                    <form onsubmit={on_update} class="space-y-8">
                    if let Some(error) = &*error_message {
                        <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                            <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
                        </div>
                    }

                    if let Some(success) = &*success_message {
                        <div class="p-4 rounded-md bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800">
                            <p class="text-sm text-green-700 dark:text-green-400">{success}</p>
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
                                value={props.site.site_details.name.clone()}
                                required={true}
                                disabled={*is_loading}
                                class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                       rounded-md shadow-sm bg-white dark:bg-neutral-700
                                       text-neutral-900 dark:text-neutral-100
                                       focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                       dark:focus:ring-neutral-400 dark:focus:border-neutral-400
                                       disabled:opacity-50 disabled:cursor-not-allowed"
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
                                value={props.site.site_details.description.clone().unwrap_or_default()}
                                disabled={*is_loading}
                                class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                       rounded-md shadow-sm bg-white dark:bg-neutral-700
                                       text-neutral-900 dark:text-neutral-100
                                       focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                       dark:focus:ring-neutral-400 dark:focus:border-neutral-400
                                       disabled:opacity-50 disabled:cursor-not-allowed"
                                placeholder="Optional description"
                            />
                        </div>
                    </div>

                    // Timezone Section
                    <div class="space-y-4">
                        <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 border-b border-neutral-200 dark:border-neutral-700 pb-2">
                            {"Timezone Settings"}
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
                                    disabled={*is_loading}
                                    class="h-4 w-4 text-neutral-600 focus:ring-neutral-500 border-neutral-300 dark:border-neutral-600 rounded disabled:opacity-50"
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
                                    disabled={!*use_timezone || *is_loading}
                                    class={classes!(
                                        "w-full", "px-3", "py-2", "border", "rounded-md", "shadow-sm",
                                        "focus:outline-none", "focus:ring-2", "focus:ring-neutral-500", "focus:border-neutral-500",
                                        "dark:focus:ring-neutral-400", "dark:focus:border-neutral-400",
                                        if *use_timezone && !*is_loading {
                                            "border-neutral-300 dark:border-neutral-600 bg-white dark:bg-neutral-700 text-neutral-900 dark:text-neutral-100"
                                        } else {
                                            "border-neutral-200 dark:border-neutral-700 bg-neutral-50 dark:bg-neutral-800 text-neutral-400 dark:text-neutral-500 cursor-not-allowed"
                                        }
                                    )}
                                >
                                    {jiff::tz::db().available().map(|tz_name| {
                                        let tz_string = tz_name.to_string();
                                        let is_selected = props.site.site_details.timezone.as_ref()
                                            .map(|tz| tz == &tz_string)
                                            .unwrap_or(tz_string == user_timezone);
                                        html! {
                                            <option value={tz_string.clone()} selected={is_selected}>{tz_string}</option>
                                        }
                                    }).collect::<Html>()}
                                </select>
                            </div>
                        </div>
                    </div>

                    // Auction Parameters Section
                    <div class="space-y-6">
                        <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 border-b border-neutral-200 dark:border-neutral-700 pb-2">
                            {"Default Auction Parameters"}
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
                                onclick={on_cancel_edit}
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

                            {if can_edit {
                                html! {
                                    <button
                                        type="button"
                                        onclick={on_delete}
                                        disabled={*is_loading}
                                        class="py-2 px-4 border border-red-300 dark:border-red-600
                                               rounded-md shadow-sm text-sm font-medium text-red-700 dark:text-red-300
                                               bg-red-50 dark:bg-red-900/20 hover:bg-red-100 dark:hover:bg-red-900/30
                                               focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-red-500
                                               disabled:opacity-50 disabled:cursor-not-allowed
                                               transition-colors duration-200"
                                    >
                                        {"Delete Site"}
                                    </button>
                                }
                            } else {
                                html! {}
                            }}

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
                                    {"Updating Site..."}
                                } else {
                                    {"Save Changes"}
                                }
                            </button>
                        </div>
                    </form>
                } else {
                    // View mode
                    <div class="space-y-8">
                        if let Some(success) = &*success_message {
                            <div class="p-4 rounded-md bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800">
                                <p class="text-sm text-green-700 dark:text-green-400">{success}</p>
                            </div>
                        }

                        // Basic Information Section
                        <div class="space-y-4">
                            <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 border-b border-neutral-200 dark:border-neutral-700 pb-2">
                                {"Basic Information"}
                            </h3>

                            <div>
                                <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                                    {"Site Name"}
                                </label>
                                <p class="text-neutral-900 dark:text-neutral-100">
                                    {&props.site.site_details.name}
                                </p>
                            </div>

                            <div>
                                <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                                    {"Description"}
                                </label>
                                <p class="text-neutral-900 dark:text-neutral-100">
                                    {props.site.site_details.description.as_deref().unwrap_or("No description")}
                                </p>
                            </div>
                        </div>

                        // Timezone Section
                        <div class="space-y-4">
                            <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 border-b border-neutral-200 dark:border-neutral-700 pb-2">
                                {"Timezone Settings"}
                            </h3>

                            <div>
                                <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                                    {"Timezone"}
                                </label>
                                <p class="text-neutral-900 dark:text-neutral-100">
                                    {props.site.site_details.timezone.as_deref().unwrap_or("No timezone set")}
                                </p>
                            </div>
                        </div>

                        // Auction Parameters Section
                        <div class="space-y-6">
                            <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 border-b border-neutral-200 dark:border-neutral-700 pb-2">
                                {"Default Auction Parameters"}
                            </h3>
                            <AuctionParamsViewer
                                auction_params={props.site.site_details.default_auction_params.clone()}
                            />
                        </div>

                        <div class="pt-6 border-t border-neutral-200 dark:border-neutral-700">
                            <button
                                type="button"
                                onclick={on_back}
                                class="py-2 px-4 border border-neutral-300 dark:border-neutral-600
                                       rounded-md shadow-sm text-sm font-medium text-neutral-700 dark:text-neutral-300
                                       bg-white dark:bg-neutral-700 hover:bg-neutral-50 dark:hover:bg-neutral-600
                                       focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                                       transition-colors duration-200"
                            >
                                {"Back to Site"}
                            </button>
                        </div>
                    </div>
                }
            </div>
        </div>
    }
}
