use payloads::{
    ClientError, Role, SiteId, Space, SpaceId,
    requests::{UpdateSpace, UpdateSpaces},
    responses::Space as SpaceResponse,
};
use std::collections::HashMap;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::components::{
    CreateSpaceModal, SitePageWrapper, SiteTabHeader, SiteWithRole,
    site_tab_header::ActiveTab,
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
    let show_create_modal = use_state(|| false);
    let edit_states = use_state(HashMap::<SpaceId, Space>::new);
    let is_saving = use_state(|| false);
    let save_error = use_state(|| None::<String>);

    let on_toggle_edit = {
        let is_editing = is_editing.clone();
        let edit_states = edit_states.clone();
        let spaces = spaces_hook.spaces.clone();
        Callback::from(move |_| {
            if *is_editing {
                // Exiting edit mode - clear changes
                edit_states.set(HashMap::new());
            } else {
                // Entering edit mode - initialize edit states
                if let Some(ref spaces) = spaces {
                    let mut states = HashMap::new();
                    for space in spaces {
                        states.insert(
                            space.space_id,
                            space.space_details.clone(),
                        );
                    }
                    edit_states.set(states);
                }
            }
            is_editing.set(!*is_editing);
        })
    };

    let on_show_create_modal = {
        let show_create_modal = show_create_modal.clone();
        Callback::from(move |_| {
            show_create_modal.set(true);
        })
    };

    let on_close_create_modal = {
        let show_create_modal = show_create_modal.clone();
        Callback::from(move |_| {
            show_create_modal.set(false);
        })
    };

    let on_space_created = {
        let refetch = spaces_hook.refetch.clone();
        Callback::from(move |_| {
            refetch.emit(());
        })
    };

    let on_save_all = {
        let spaces = spaces_hook.spaces.clone();
        let edit_states = edit_states.clone();
        let is_saving = is_saving.clone();
        let save_error = save_error.clone();
        let is_editing = is_editing.clone();
        let refetch = spaces_hook.refetch.clone();
        Callback::from(move |_| {
            let spaces = match &spaces {
                Some(s) => s.clone(),
                None => return,
            };

            let mut updates = Vec::new();
            for space in &spaces {
                if let Some(edit_state) = edit_states.get(&space.space_id)
                    && edit_state != &space.space_details
                {
                    updates.push(UpdateSpace {
                        space_id: space.space_id,
                        space_details: edit_state.clone(),
                    });
                }
            }

            if updates.is_empty() {
                return;
            }

            let is_saving = is_saving.clone();
            let save_error = save_error.clone();
            let refetch = refetch.clone();
            let is_editing = is_editing.clone();
            let edit_states = edit_states.clone();

            yew::platform::spawn_local(async move {
                is_saving.set(true);
                save_error.set(None);

                let api_client = crate::get_api_client();
                let result = api_client
                    .update_spaces(&UpdateSpaces { spaces: updates })
                    .await;

                match result {
                    Ok(_) => {
                        is_editing.set(false);
                        edit_states.set(HashMap::new());
                        refetch.emit(());
                    }
                    Err(ClientError::APIError(_, msg)) => {
                        save_error.set(Some(msg));
                    }
                    Err(ClientError::Network(_)) => {
                        save_error.set(Some(
                            "Network error. Please check your connection."
                                .to_string(),
                        ));
                    }
                }

                is_saving.set(false);
            });
        })
    };

    let spaces_content = if spaces_hook.is_loading {
        html! {
            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400">{"Loading spaces..."}</p>
            </div>
        }
    } else if let Some(error) = &spaces_hook.error {
        html! {
            <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
            </div>
        }
    } else {
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
                                    <button
                                        onclick={on_show_create_modal.clone()}
                                        class="bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors"
                                    >
                                        {"Create First Space"}
                                    </button>
                                }
                            } else {
                                html! {}
                            }}
                        </div>
                    }
                } else {
                    let has_changes = if let Some(spaces) = &spaces_hook.spaces
                    {
                        spaces.iter().any(|space| {
                            if let Some(edit_state) =
                                edit_states.get(&space.space_id)
                            {
                                edit_state != &space.space_details
                            } else {
                                false
                            }
                        })
                    } else {
                        false
                    };

                    html! {
                        <div>
                            <div class="flex justify-between items-center mb-6">
                                <h2 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100">
                                    {"Spaces"}
                                </h2>
                                {if can_edit {
                                    html! {
                                        <div class="flex gap-2">
                                            {if *is_editing {
                                                html! {
                                                    <>
                                                        <button
                                                            onclick={on_toggle_edit.clone()}
                                                            disabled={*is_saving}
                                                            class="py-2 px-4 border border-neutral-300 dark:border-neutral-600
                                                               rounded-md shadow-sm text-sm font-medium text-neutral-700 dark:text-neutral-300
                                                               bg-white dark:bg-neutral-700 hover:bg-neutral-50 dark:hover:bg-neutral-600
                                                               focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                                                               disabled:opacity-50 disabled:cursor-not-allowed
                                                               transition-colors duration-200"
                                                        >
                                                            {"Cancel"}
                                                        </button>
                                                        {if has_changes {
                                                            html! {
                                                                <button
                                                                    onclick={on_save_all}
                                                                    disabled={*is_saving}
                                                                    class="bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                                                                >
                                                                    {if *is_saving { "Saving..." } else { "Save All Changes" }}
                                                                </button>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }}
                                                    </>
                                                }
                                            } else {
                                                html! {
                                                    <>
                                                        <button
                                                            onclick={on_toggle_edit}
                                                            class="py-2 px-4 border border-neutral-300 dark:border-neutral-600
                                                               rounded-md shadow-sm text-sm font-medium text-neutral-700 dark:text-neutral-300
                                                               bg-white dark:bg-neutral-700 hover:bg-neutral-50 dark:hover:bg-neutral-600
                                                               focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                                                               transition-colors duration-200"
                                                        >
                                                            {"Edit Spaces"}
                                                        </button>
                                                        <button
                                                            onclick={on_show_create_modal.clone()}
                                                            class="bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors"
                                                        >
                                                            {"Create New Space"}
                                                        </button>
                                                    </>
                                                }
                                            }}
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }}
                            </div>

                            {if let Some(error) = &*save_error {
                                html! {
                                    <div class="mb-4 p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                                        <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
                                    </div>
                                }
                            } else {
                                html! {}
                            }}

                            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                                {spaces.iter().map(|space| {
                                    let refetch = spaces_hook.refetch.clone();
                                    let edit_states = edit_states.clone();
                                    let space_id = space.space_id;
                                    html! {
                                        <SpaceCard
                                            key={space.space_id.to_string()}
                                            space={space.clone()}
                                            is_editing={*is_editing}
                                            edit_state={edit_states.get(&space_id).cloned()}
                                            on_edit_change={Callback::from(move |updated: Space| {
                                                let mut states = (*edit_states).clone();
                                                states.insert(space_id, updated);
                                                edit_states.set(states);
                                            })}
                                            on_delete={Callback::from(move |_| refetch.emit(()))}
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
    };

    html! {
        <>
            {spaces_content}
            {if *show_create_modal {
                html! {
                    <CreateSpaceModal
                        site_id={props.site_id}
                        on_close={on_close_create_modal}
                        on_space_created={on_space_created}
                    />
                }
            } else {
                html! {}
            }}
        </>
    }
}

#[derive(Properties, PartialEq)]
struct SpaceCardProps {
    space: SpaceResponse,
    is_editing: bool,
    edit_state: Option<Space>,
    on_edit_change: Callback<Space>,
    on_delete: Callback<()>,
}

#[function_component]
fn SpaceCard(props: &SpaceCardProps) -> Html {
    let is_deleting = use_state(|| false);
    let delete_error = use_state(|| None::<String>);

    let on_delete_click = {
        let space = props.space.clone();
        let is_deleting = is_deleting.clone();
        let delete_error = delete_error.clone();
        let on_delete = props.on_delete.clone();

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
            let is_deleting = is_deleting.clone();
            let delete_error = delete_error.clone();
            let on_delete = on_delete.clone();

            yew::platform::spawn_local(async move {
                is_deleting.set(true);
                delete_error.set(None);

                let api_client = crate::get_api_client();
                match api_client.delete_space(&space_id).await {
                    Ok(_) => {
                        on_delete.emit(());
                    }
                    Err(ClientError::APIError(_, msg)) => {
                        delete_error.set(Some(msg));
                    }
                    Err(ClientError::Network(_)) => {
                        delete_error.set(Some(
                            "Network error. Please check your connection."
                                .to_string(),
                        ));
                    }
                }

                is_deleting.set(false);
            });
        })
    };

    if props.is_editing {
        let edit_state = props
            .edit_state
            .as_ref()
            .unwrap_or(&props.space.space_details);

        let on_name_change = {
            let on_edit_change = props.on_edit_change.clone();
            let edit_state = edit_state.clone();
            Callback::from(move |e: InputEvent| {
                let input: HtmlInputElement = e.target_unchecked_into();
                let mut updated = edit_state.clone();
                updated.name = input.value();
                on_edit_change.emit(updated);
            })
        };

        let on_description_change = {
            let on_edit_change = props.on_edit_change.clone();
            let edit_state = edit_state.clone();
            Callback::from(move |e: InputEvent| {
                let input: HtmlInputElement = e.target_unchecked_into();
                let value = input.value();
                let mut updated = edit_state.clone();
                updated.description =
                    if value.is_empty() { None } else { Some(value) };
                on_edit_change.emit(updated);
            })
        };

        let on_eligibility_change = {
            let on_edit_change = props.on_edit_change.clone();
            let edit_state = edit_state.clone();
            Callback::from(move |e: InputEvent| {
                let input: HtmlInputElement = e.target_unchecked_into();
                if let Ok(v) = input.value().parse::<f64>() {
                    let mut updated = edit_state.clone();
                    updated.eligibility_points = v;
                    on_edit_change.emit(updated);
                }
            })
        };

        let on_available_change = {
            let on_edit_change = props.on_edit_change.clone();
            let edit_state = edit_state.clone();
            Callback::from(move |e: Event| {
                let input: HtmlInputElement = e.target_unchecked_into();
                let mut updated = edit_state.clone();
                updated.is_available = input.checked();
                on_edit_change.emit(updated);
            })
        };

        // Edit mode
        html! {
            <div class="bg-white dark:bg-neutral-800 p-6 rounded-lg shadow-md border border-neutral-200 dark:border-neutral-700">
                <div class="space-y-4">
                    {if let Some(error) = &*delete_error {
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
                            type="text"
                            value={edit_state.name.clone()}
                            oninput={on_name_change}
                            disabled={*is_deleting}
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
                            type="text"
                            value={edit_state.description.clone().unwrap_or_default()}
                            oninput={on_description_change}
                            disabled={*is_deleting}
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
                            type="number"
                            step="0.1"
                            value={edit_state.eligibility_points.to_string()}
                            oninput={on_eligibility_change}
                            disabled={*is_deleting}
                            class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                   rounded-md shadow-sm bg-white dark:bg-neutral-700
                                   text-neutral-900 dark:text-neutral-100 text-sm
                                   focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                   disabled:opacity-50 disabled:cursor-not-allowed"
                        />
                    </div>

                    <div class="flex items-center">
                        <input
                            type="checkbox"
                            checked={edit_state.is_available}
                            onchange={on_available_change}
                            disabled={*is_deleting}
                            class="h-4 w-4 text-neutral-600 focus:ring-neutral-500 border-neutral-300 dark:border-neutral-600 rounded disabled:opacity-50"
                        />
                        <label class="ml-2 text-sm font-medium text-neutral-700 dark:text-neutral-300">
                            {"Available"}
                        </label>
                    </div>

                    <div class="pt-4">
                        <button
                            type="button"
                            onclick={on_delete_click}
                            disabled={*is_deleting}
                            class="w-full py-2 px-4 border border-red-300 dark:border-red-600
                                   rounded-md shadow-sm text-sm font-medium text-red-700 dark:text-red-300
                                   bg-red-50 dark:bg-red-900/20 hover:bg-red-100 dark:hover:bg-red-900/30
                                   focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-red-500
                                   disabled:opacity-50 disabled:cursor-not-allowed
                                   transition-colors duration-200"
                        >
                            {if *is_deleting { "Deleting..." } else { "Delete" }}
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
