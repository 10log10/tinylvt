use payloads::{
    AuctionParams, Role, Site, SiteId, requests::UpdateSite,
    responses::Site as SiteResponse,
};
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::{
    Route,
    components::{
        AuctionParamsEditor, AuctionParamsViewer, ConfirmationModal,
        SitePageWrapper, SiteTabHeader, SiteWithRole,
        site_tab_header::ActiveTab,
    },
    hooks::{use_auctions, use_site},
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
    let auctions_hook = use_auctions(props.site.site_id);

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

    // State for confirmation modal (permanent delete)
    let show_delete_modal = use_state(|| false);
    let is_deleting = use_state(|| false);
    let delete_error_message = use_state(|| None::<String>);

    // State for confirmation modal (soft delete)
    let show_soft_delete_modal = use_state(|| false);
    let is_soft_deleting = use_state(|| false);
    let soft_delete_error_message = use_state(|| None::<String>);

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
                    Err(e) => {
                        error_message.set(Some(e.to_string()));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    // Soft delete handler - called when site is not yet deleted
    // Soft delete handler - called from confirmation modal
    let on_soft_delete_confirm = {
        let soft_delete_error_message = soft_delete_error_message.clone();
        let is_soft_deleting = is_soft_deleting.clone();
        let show_soft_delete_modal = show_soft_delete_modal.clone();
        let success_message = success_message.clone();
        let site_id = props.site.site_id;
        let refetch_site = site_hook.refetch.clone();
        let refetch_auctions = auctions_hook.refetch.clone();

        Callback::from(move |_| {
            let soft_delete_error_message = soft_delete_error_message.clone();
            let is_soft_deleting = is_soft_deleting.clone();
            let show_soft_delete_modal = show_soft_delete_modal.clone();
            let success_message = success_message.clone();
            let refetch_site = refetch_site.clone();
            let refetch_auctions = refetch_auctions.clone();

            yew::platform::spawn_local(async move {
                is_soft_deleting.set(true);
                soft_delete_error_message.set(None);

                let api_client = crate::get_api_client();
                match api_client.soft_delete_site(&site_id).await {
                    Ok(_) => {
                        // Close modal and show success message
                        show_soft_delete_modal.set(false);
                        success_message.set(Some(
                            "Site has been deleted. You can permanently delete it if needed.".to_string(),
                        ));
                        // Refresh site data to show updated deleted_at
                        refetch_site.emit(());
                        // Refetch auctions in case any were canceled
                        refetch_auctions.emit(());
                    }
                    Err(e) => {
                        soft_delete_error_message.set(Some(e.to_string()));
                        is_soft_deleting.set(false);
                    }
                }
            });
        })
    };

    // Restore handler - called when site is soft-deleted
    let on_restore = {
        let error_message = error_message.clone();
        let success_message = success_message.clone();
        let is_loading = is_loading.clone();
        let site_id = props.site.site_id;
        let refetch_site = site_hook.refetch.clone();

        Callback::from(move |_| {
            let error_message = error_message.clone();
            let success_message = success_message.clone();
            let is_loading = is_loading.clone();
            let refetch_site = refetch_site.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error_message.set(None);
                success_message.set(None);

                let api_client = crate::get_api_client();
                match api_client.restore_site(&site_id).await {
                    Ok(_) => {
                        success_message.set(Some(
                            "Site has been restored successfully.".to_string(),
                        ));
                        // Refresh site data to show updated deleted_at
                        refetch_site.emit(());
                    }
                    Err(e) => {
                        error_message.set(Some(e.to_string()));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    // Hard delete handler - called from confirmation modal
    let on_hard_delete = {
        let delete_error_message = delete_error_message.clone();
        let is_deleting = is_deleting.clone();
        let navigator = navigator.clone();
        let site_id = props.site.site_id;
        let community_id = props.site.site_details.community_id;

        Callback::from(move |_| {
            let delete_error_message = delete_error_message.clone();
            let is_deleting = is_deleting.clone();
            let navigator = navigator.clone();

            yew::platform::spawn_local(async move {
                is_deleting.set(true);
                delete_error_message.set(None);

                let api_client = crate::get_api_client();
                match api_client.delete_site(&site_id).await {
                    Ok(_) => {
                        navigator
                            .push(&Route::CommunityDetail { id: community_id });
                    }
                    Err(e) => {
                        delete_error_message.set(Some(e.to_string()));
                        is_deleting.set(false);
                    }
                }
            });
        })
    };

    // Delete button handler - shows appropriate confirmation modal
    let on_delete = {
        let site = props.site.clone();
        let show_delete_modal = show_delete_modal.clone();
        let show_soft_delete_modal = show_soft_delete_modal.clone();

        Callback::from(move |_| {
            if site.deleted_at.is_some() {
                // Already soft-deleted, show confirmation modal for permanent delete
                show_delete_modal.set(true);
            } else {
                // Not deleted yet, show confirmation modal for soft delete
                show_soft_delete_modal.set(true);
            }
        })
    };

    let on_close_modal = {
        let show_delete_modal = show_delete_modal.clone();
        let delete_error_message = delete_error_message.clone();

        Callback::from(move |_| {
            show_delete_modal.set(false);
            delete_error_message.set(None);
        })
    };

    let on_close_soft_delete_modal = {
        let show_soft_delete_modal = show_soft_delete_modal.clone();
        let soft_delete_error_message = soft_delete_error_message.clone();

        Callback::from(move |_| {
            show_soft_delete_modal.set(false);
            soft_delete_error_message.set(None);
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
            navigator.push(&Route::SiteAuctions { id: site_id });
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

                    if props.site.deleted_at.is_some() {
                        <div class="p-4 rounded-md bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800">
                            <p class="text-sm text-amber-700 dark:text-amber-400">
                                {"This site has been deleted. You can still edit it or permanently delete it using the button below."}
                            </p>
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
                                let is_deleted = props.site.deleted_at.is_some();

                                if is_deleted {
                                    // Show both Restore and Permanently Delete buttons
                                    html! {
                                        <>
                                            <button
                                                type="button"
                                                onclick={on_restore.clone()}
                                                disabled={*is_loading}
                                                class="py-2 px-4 border border-green-300 dark:border-green-600
                                                       rounded-md shadow-sm text-sm font-medium text-green-700 dark:text-green-300
                                                       bg-green-50 dark:bg-green-900/20 hover:bg-green-100 dark:hover:bg-green-900/30
                                                       focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500
                                                       disabled:opacity-50 disabled:cursor-not-allowed
                                                       transition-colors duration-200"
                                            >
                                                {"Restore Site"}
                                            </button>
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
                                                {"Permanently Delete Site"}
                                            </button>
                                        </>
                                    }
                                } else {
                                    // Show only Delete Site button
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

            // Confirmation modal for permanent delete
            {if *show_delete_modal {
                html! {
                    <ConfirmationModal
                        title="Permanently Delete Site"
                        message="This will permanently delete the site and remove all spaces and auctions associated with it. This action cannot be undone."
                        confirm_text="Permanently Delete"
                        confirmation_value={props.site.site_details.name.clone()}
                        confirmation_label="the site name"
                        on_confirm={on_hard_delete}
                        on_close={on_close_modal}
                        is_loading={*is_deleting}
                        error_message={(*delete_error_message).clone().map(AttrValue::from)}
                    />
                }
            } else {
                html! {}
            }}

            // Confirmation modal for soft delete
            {if *show_soft_delete_modal {
                html! {
                    <ConfirmationModal
                        title="Delete Site"
                        message="This will delete the site and cancel any active auctions. The site can be restored later if needed."
                        confirm_text="Delete Site"
                        confirmation_value={props.site.site_details.name.clone()}
                        confirmation_label="the site name"
                        on_confirm={on_soft_delete_confirm}
                        on_close={on_close_soft_delete_modal}
                        is_loading={*is_soft_deleting}
                        error_message={(*soft_delete_error_message).clone().map(AttrValue::from)}
                    />
                }
            } else {
                html! {}
            }}
        </div>
    }
}
