use payloads::{
    ClientError, Role, SiteId, requests::UpdateSpace,
    responses::Space as SpaceResponse,
};
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::components::{
    SitePageWrapper, SiteTabHeader, SiteWithRole, site_tab_header::ActiveTab,
};
use crate::hooks::use_spaces;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub site_id: SiteId,
}

#[function_component]
pub fn SiteDetailPage(props: &Props) -> Html {
    let render_content = Callback::from(|site_with_role: SiteWithRole| {
        html! {
            <div>
                <SiteTabHeader site={site_with_role.site.clone()} active_tab={ActiveTab::Spaces} />
                <div class="py-6">
                    <SpacesTab
                        site_id={site_with_role.site.site_id}
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
pub struct SpacesTabProps {
    pub site_id: SiteId,
    pub user_role: Role,
}

#[function_component]
fn SpacesTab(props: &SpacesTabProps) -> Html {
    let spaces_hook = use_spaces(props.site_id);
    let can_edit = props.user_role.is_ge_coleader();
    let is_editing = use_state(|| false);

    let on_toggle_edit = {
        let is_editing = is_editing.clone();
        Callback::from(move |_| {
            is_editing.set(!*is_editing);
        })
    };

    if spaces_hook.is_loading {
        return html! {
            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400">{"Loading spaces..."}</p>
            </div>
        };
    }

    if let Some(error) = &spaces_hook.error {
        return html! {
            <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
            </div>
        };
    }

    match &spaces_hook.spaces {
        Some(spaces) => {
            if spaces.is_empty() {
                html! {
                    <div class="text-center py-12">
                        <p class="text-neutral-600 dark:text-neutral-400 mb-4">
                            {"No spaces have been created for this site yet."}
                        </p>
                        {if can_edit {
                            html! {
                                <button class="bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors">
                                    {"Create First Space"}
                                </button>
                            }
                        } else {
                            html! {}
                        }}
                    </div>
                }
            } else {
                html! {
                    <div>
                        <div class="flex justify-between items-center mb-6">
                            <h2 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100">
                                {"Spaces"}
                            </h2>
                            {if can_edit {
                                html! {
                                    <div class="flex gap-2">
                                        <button
                                            onclick={on_toggle_edit}
                                            class="py-2 px-4 border border-neutral-300 dark:border-neutral-600
                                                   rounded-md shadow-sm text-sm font-medium text-neutral-700 dark:text-neutral-300
                                                   bg-white dark:bg-neutral-700 hover:bg-neutral-50 dark:hover:bg-neutral-600
                                                   focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                                                   transition-colors duration-200"
                                        >
                                            {if *is_editing { "Done Editing" } else { "Edit Spaces" }}
                                        </button>
                                        <button class="bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors">
                                            {"Create New Space"}
                                        </button>
                                    </div>
                                }
                            } else {
                                html! {}
                            }}
                        </div>

                        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                            {spaces.iter().map(|space| {
                                let refetch = spaces_hook.refetch.clone();
                                html! {
                                    <SpaceCard
                                        key={space.space_id.to_string()}
                                        space={space.clone()}
                                        is_editing={*is_editing}
                                        on_updated={Callback::from(move |_| refetch.emit(()))}
                                    />
                                }
                            }).collect::<Html>()}
                        </div>
                    </div>
                }
            }
        }
        None => {
            html! {
                <div class="text-center py-12">
                    <p class="text-neutral-600 dark:text-neutral-400">{"No spaces data available"}</p>
                </div>
            }
        }
    }
}

#[derive(Properties, PartialEq)]
struct SpaceCardProps {
    space: SpaceResponse,
    is_editing: bool,
    on_updated: Callback<()>,
}

#[function_component]
fn SpaceCard(props: &SpaceCardProps) -> Html {
    let name_ref = use_node_ref();
    let description_ref = use_node_ref();
    let eligibility_ref = use_node_ref();
    let available_ref = use_node_ref();

    let is_loading = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    let on_save = {
        let name_ref = name_ref.clone();
        let description_ref = description_ref.clone();
        let eligibility_ref = eligibility_ref.clone();
        let available_ref = available_ref.clone();
        let space = props.space.clone();
        let is_loading = is_loading.clone();
        let error_message = error_message.clone();
        let on_updated = props.on_updated.clone();

        Callback::from(move |_| {
            let name_input = name_ref.cast::<HtmlInputElement>().unwrap();
            let name = name_input.value().trim().to_string();

            if name.is_empty() {
                error_message.set(Some("Name is required".to_string()));
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

            let eligibility_input =
                eligibility_ref.cast::<HtmlInputElement>().unwrap();
            let eligibility_points =
                match eligibility_input.value().parse::<f64>() {
                    Ok(v) => v,
                    Err(_) => {
                        error_message.set(Some(
                            "Invalid eligibility points".to_string(),
                        ));
                        return;
                    }
                };

            let available_input =
                available_ref.cast::<HtmlInputElement>().unwrap();
            let is_available = available_input.checked();

            let updated_space = payloads::Space {
                site_id: space.space_details.site_id,
                name,
                description,
                eligibility_points,
                is_available,
                site_image_id: space.space_details.site_image_id,
            };

            let update_request = UpdateSpace {
                space_id: space.space_id,
                space_details: updated_space,
            };

            let is_loading = is_loading.clone();
            let error_message = error_message.clone();
            let on_updated = on_updated.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error_message.set(None);

                let api_client = crate::get_api_client();
                match api_client.update_space(&update_request).await {
                    Ok(_) => {
                        on_updated.emit(());
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
        let space = props.space.clone();
        let is_loading = is_loading.clone();
        let error_message = error_message.clone();
        let on_updated = props.on_updated.clone();

        Callback::from(move |_| {
            let confirmed = web_sys::window()
                .unwrap()
                .confirm_with_message(&format!(
                    "Are you sure you want to delete the space '{}'? This action cannot be undone.",
                    space.space_details.name
                ))
                .unwrap_or(false);

            if !confirmed {
                return;
            }

            let space_id = space.space_id;
            let is_loading = is_loading.clone();
            let error_message = error_message.clone();
            let on_updated = on_updated.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error_message.set(None);

                let api_client = crate::get_api_client();
                match api_client.delete_space(&space_id).await {
                    Ok(_) => {
                        on_updated.emit(());
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

    if props.is_editing {
        // Edit mode
        html! {
            <div class="bg-white dark:bg-neutral-800 p-6 rounded-lg shadow-md border border-neutral-200 dark:border-neutral-700">
                <div class="space-y-4">
                    {if let Some(error) = &*error_message {
                        html! {
                            <div class="p-3 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                                <p class="text-xs text-red-700 dark:text-red-400">{error}</p>
                            </div>
                        }
                    } else {
                        html! {}
                    }}

                    <div>
                        <label class="block text-xs font-medium text-neutral-700 dark:text-neutral-300 mb-1">
                            {"Name"}
                        </label>
                        <input
                            ref={name_ref}
                            type="text"
                            value={props.space.space_details.name.clone()}
                            disabled={*is_loading}
                            class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                   rounded-md shadow-sm bg-white dark:bg-neutral-700
                                   text-neutral-900 dark:text-neutral-100 text-sm
                                   focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                   disabled:opacity-50 disabled:cursor-not-allowed"
                        />
                    </div>

                    <div>
                        <label class="block text-xs font-medium text-neutral-700 dark:text-neutral-300 mb-1">
                            {"Description"}
                        </label>
                        <input
                            ref={description_ref}
                            type="text"
                            value={props.space.space_details.description.clone().unwrap_or_default()}
                            disabled={*is_loading}
                            class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                   rounded-md shadow-sm bg-white dark:bg-neutral-700
                                   text-neutral-900 dark:text-neutral-100 text-sm
                                   focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                   disabled:opacity-50 disabled:cursor-not-allowed"
                        />
                    </div>

                    <div>
                        <label class="block text-xs font-medium text-neutral-700 dark:text-neutral-300 mb-1">
                            {"Eligibility Points"}
                        </label>
                        <input
                            ref={eligibility_ref}
                            type="number"
                            step="0.1"
                            value={props.space.space_details.eligibility_points.to_string()}
                            disabled={*is_loading}
                            class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                   rounded-md shadow-sm bg-white dark:bg-neutral-700
                                   text-neutral-900 dark:text-neutral-100 text-sm
                                   focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                   disabled:opacity-50 disabled:cursor-not-allowed"
                        />
                    </div>

                    <div class="flex items-center">
                        <input
                            ref={available_ref}
                            type="checkbox"
                            checked={props.space.space_details.is_available}
                            disabled={*is_loading}
                            class="h-4 w-4 text-neutral-600 focus:ring-neutral-500 border-neutral-300 dark:border-neutral-600 rounded disabled:opacity-50"
                        />
                        <label class="ml-2 text-sm font-medium text-neutral-700 dark:text-neutral-300">
                            {"Available"}
                        </label>
                    </div>

                    <div class="pt-4 flex gap-2">
                        <button
                            type="button"
                            onclick={on_save}
                            disabled={*is_loading}
                            class="flex-1 py-2 px-4 border border-transparent
                                   rounded-md shadow-sm text-sm font-medium text-white
                                   bg-neutral-900 hover:bg-neutral-800
                                   dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200
                                   focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                                   disabled:opacity-50 disabled:cursor-not-allowed
                                   transition-colors duration-200"
                        >
                            {if *is_loading { "Saving..." } else { "Save" }}
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
                            {"Delete"}
                        </button>
                    </div>
                </div>
            </div>
        }
    } else {
        // View mode
        html! {
            <div class="bg-white dark:bg-neutral-800 p-6 rounded-lg shadow-md border border-neutral-200 dark:border-neutral-700">
                <div class="space-y-4">
                    <div>
                        <h3 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100">
                            {&props.space.space_details.name}
                        </h3>
                        <div class="h-12">
                            {if let Some(description) = &props.space.space_details.description {
                                html! {
                                    <p class="text-sm text-neutral-600 dark:text-neutral-400 mt-1 line-clamp-3">
                                        {description}
                                    </p>
                                }
                            } else {
                                html! {}
                            }}
                        </div>
                    </div>

                    <div class="text-sm text-neutral-600 dark:text-neutral-400 space-y-1">
                        <p>{"Eligibility Points: "}{props.space.space_details.eligibility_points}</p>
                        <p>{"Status: "}{if props.space.space_details.is_available { "Available" } else { "Unavailable" }}</p>
                        <p>{"Created: "}{props.space.created_at.to_zoned(jiff::tz::TimeZone::system()).strftime("%B %d, %Y").to_string()}</p>
                    </div>
                </div>
            </div>
        }
    }
}
