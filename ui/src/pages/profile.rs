use crate::components::{ConfirmationModal, RequireAuth};
use crate::get_api_client;
use crate::hooks::{use_communities, use_logout};
use crate::{AuthState, State};
use payloads::responses::UserProfile;
use payloads::{Role, requests};
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;

#[function_component]
pub fn ProfilePage() -> Html {
    let render_content = Callback::from(|profile: UserProfile| {
        html! { <ProfilePageInner {profile} /> }
    });

    html! {
        <RequireAuth render={render_content} />
    }
}

#[derive(Properties, PartialEq)]
struct ProfilePageInnerProps {
    profile: UserProfile,
}

#[function_component]
fn ProfilePageInner(props: &ProfilePageInnerProps) -> Html {
    let profile = &props.profile;
    let logout = use_logout();
    let (_state, dispatch) = use_store::<State>();

    // Modal and deletion state
    let show_delete_modal = use_state(|| false);
    let is_deleting = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    // Display name editing state
    let is_editing_display_name = use_state(|| false);
    let display_name_input = use_node_ref();
    let display_name_error = use_state(|| None::<String>);
    let is_saving_display_name = use_state(|| false);

    // Use communities hook to get cached communities
    let communities_hook = use_communities();

    // Extract leader communities from the hook data
    let leader_communities: Option<Vec<String>> =
        match communities_hook.data.as_ref() {
            Some(communities) => {
                let leaders: Vec<String> = communities
                    .iter()
                    .filter(|c| c.user_role == Role::Leader)
                    .map(|c| c.name.clone())
                    .collect();
                Some(leaders)
            }
            None => None,
        };

    let username = profile.username.clone();

    let on_open_modal = {
        let show_delete_modal = show_delete_modal.clone();
        let error_message = error_message.clone();
        Callback::from(move |_| {
            error_message.set(None);
            show_delete_modal.set(true);
        })
    };

    let on_close_modal = {
        let show_delete_modal = show_delete_modal.clone();
        Callback::from(move |()| {
            show_delete_modal.set(false);
        })
    };

    let on_delete = {
        let is_deleting = is_deleting.clone();
        let error_message = error_message.clone();
        let logout = logout.clone();

        Callback::from(move |_| {
            let is_deleting = is_deleting.clone();
            let error_message = error_message.clone();
            let logout = logout.clone();

            is_deleting.set(true);
            error_message.set(None);

            wasm_bindgen_futures::spawn_local(async move {
                let client = get_api_client();
                match client.delete_user().await {
                    Ok(_) => {
                        // Clears client state and navigate to landing.
                        logout.emit(());
                    }
                    Err(e) => {
                        error_message.set(Some(e.to_string()));
                        is_deleting.set(false);
                    }
                }
            });
        })
    };

    let on_edit_display_name = {
        let is_editing_display_name = is_editing_display_name.clone();
        let display_name_error = display_name_error.clone();
        Callback::from(move |_: MouseEvent| {
            display_name_error.set(None);
            is_editing_display_name.set(true);
        })
    };

    let on_cancel_edit = {
        let is_editing_display_name = is_editing_display_name.clone();
        let display_name_error = display_name_error.clone();
        Callback::from(move |_: MouseEvent| {
            display_name_error.set(None);
            is_editing_display_name.set(false);
        })
    };

    let on_save_display_name = {
        let display_name_input = display_name_input.clone();
        let is_editing_display_name = is_editing_display_name.clone();
        let display_name_error = display_name_error.clone();
        let is_saving_display_name = is_saving_display_name.clone();
        let dispatch = dispatch.clone();

        Callback::from(move |_: MouseEvent| {
            let input = display_name_input.cast::<HtmlInputElement>();
            let Some(input) = input else { return };

            let value = input.value().trim().to_string();
            let display_name =
                if value.is_empty() { None } else { Some(value) };

            // Validate length
            if let Some(ref name) = display_name
                && name.len() > requests::DISPLAY_NAME_MAX_LEN
            {
                display_name_error.set(Some(format!(
                    "Display name must be at most {} characters",
                    requests::DISPLAY_NAME_MAX_LEN
                )));
                return;
            }

            let is_editing_display_name = is_editing_display_name.clone();
            let display_name_error = display_name_error.clone();
            let is_saving_display_name = is_saving_display_name.clone();
            let dispatch = dispatch.clone();

            is_saving_display_name.set(true);
            display_name_error.set(None);

            wasm_bindgen_futures::spawn_local(async move {
                let client = get_api_client();
                let request = requests::UpdateProfile {
                    display_name: display_name.clone(),
                };

                match client.update_profile(&request).await {
                    Ok(_) => {
                        // Update auth state with new display name
                        dispatch.reduce_mut(|state| {
                            if let AuthState::LoggedIn(ref mut profile) =
                                state.auth_state
                            {
                                profile.display_name = display_name;
                            }
                        });
                        is_editing_display_name.set(false);
                    }
                    Err(e) => {
                        display_name_error.set(Some(e.to_string()));
                    }
                }
                is_saving_display_name.set(false);
            });
        })
    };

    html! {
        <div class="max-w-2xl mx-auto py-8 px-4">
            <h1 class="text-2xl font-bold text-neutral-900 dark:text-neutral-100 mb-8">
                {"Account Settings"}
            </h1>

            // Profile Information Section
            <div class="bg-white dark:bg-neutral-800 rounded-lg shadow-sm border border-neutral-200 dark:border-neutral-700 p-6 mb-8">
                <h2 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                    {"Profile Information"}
                </h2>
                <div class="space-y-3 text-sm">
                    <div class="flex">
                        <span class="w-32 text-neutral-500 dark:text-neutral-400">{"Username"}</span>
                        <span class="text-neutral-900 dark:text-neutral-100">{&profile.username}</span>
                    </div>
                    <div class="flex">
                        <span class="w-32 text-neutral-500 dark:text-neutral-400">{"Email"}</span>
                        <span class="text-neutral-900 dark:text-neutral-100">
                            {&profile.email}
                            {if profile.email_verified {
                                html! {
                                    <span class="ml-2 text-xs text-green-600 dark:text-green-400">{"(verified)"}</span>
                                }
                            } else {
                                html! {
                                    <span class="ml-2 text-xs text-amber-600 dark:text-amber-400">{"(unverified)"}</span>
                                }
                            }}
                        </span>
                    </div>
                    <div class="flex items-start">
                        <span class="w-32 text-neutral-500 dark:text-neutral-400 pt-1">
                            {"Display Name"}
                        </span>
                        <div class="flex-1">
                            if *is_editing_display_name {
                                <div class="space-y-2">
                                    <input
                                        ref={display_name_input.clone()}
                                        type="text"
                                        value={profile.display_name.clone().unwrap_or_default()}
                                        disabled={*is_saving_display_name}
                                        class="w-full px-2 py-1 text-sm border border-neutral-300
                                               dark:border-neutral-600 rounded bg-white dark:bg-neutral-700
                                               text-neutral-900 dark:text-neutral-100
                                               focus:outline-none focus:ring-1 focus:ring-neutral-500
                                               disabled:opacity-50"
                                        placeholder="Enter display name (optional)"
                                    />
                                    if let Some(error) = &*display_name_error {
                                        <p class="text-sm text-red-600 dark:text-red-400">
                                            {error}
                                        </p>
                                    }
                                    <div class="flex gap-2">
                                        <button
                                            onclick={on_save_display_name.clone()}
                                            disabled={*is_saving_display_name}
                                            class="px-3 py-1 text-xs font-medium text-white
                                                   bg-neutral-900 dark:bg-neutral-100
                                                   dark:text-neutral-900 rounded
                                                   hover:bg-neutral-800 dark:hover:bg-neutral-200
                                                   disabled:opacity-50 transition-colors"
                                        >
                                            {if *is_saving_display_name { "Saving..." } else { "Save" }}
                                        </button>
                                        <button
                                            onclick={on_cancel_edit.clone()}
                                            disabled={*is_saving_display_name}
                                            class="px-3 py-1 text-xs font-medium
                                                   text-neutral-700 dark:text-neutral-300
                                                   bg-neutral-100 dark:bg-neutral-700 rounded
                                                   hover:bg-neutral-200 dark:hover:bg-neutral-600
                                                   disabled:opacity-50 transition-colors"
                                        >
                                            {"Cancel"}
                                        </button>
                                    </div>
                                </div>
                            } else {
                                <div class="flex items-center gap-2">
                                    <span class="text-neutral-900 dark:text-neutral-100">
                                        {profile.display_name.as_deref().unwrap_or("Not set")}
                                    </span>
                                    <button
                                        onclick={on_edit_display_name.clone()}
                                        class="text-xs text-neutral-500 hover:text-neutral-700
                                               dark:text-neutral-400 dark:hover:text-neutral-200
                                               underline transition-colors"
                                    >
                                        {"Edit"}
                                    </button>
                                </div>
                            }
                        </div>
                    </div>
                </div>
            </div>

            // Danger Zone Section
            <div class="bg-red-50 dark:bg-red-900/10 rounded-lg border border-red-200 dark:border-red-800 p-6">
                <h2 class="text-lg font-semibold text-red-800 dark:text-red-200 mb-2">
                    {"Danger Zone"}
                </h2>
                <p class="text-sm text-red-700 dark:text-red-300 mb-4">
                    {"Once you delete your account, there is no going back. Please be certain."}
                </p>

                {
                    match &leader_communities {
                        Some(leaders) if !leaders.is_empty() => {
                            let community_list = leaders.join(", ");
                            html! {
                                <>
                                    <div class="mb-4 p-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-700 rounded-md">
                                        <p class="text-sm text-amber-800 dark:text-amber-200">
                                            {"You are the leader of: "}
                                            <span class="font-medium">{community_list}</span>
                                            {". You must transfer leadership or delete the community before you can delete your account."}
                                        </p>
                                    </div>
                                    <button
                                        disabled={true}
                                        class="px-4 py-2 text-sm font-medium text-red-400 dark:text-red-500
                                               bg-neutral-100 dark:bg-neutral-800 border border-neutral-300 dark:border-neutral-600
                                               rounded-md cursor-not-allowed opacity-50"
                                    >
                                        {"Delete Account"}
                                    </button>
                                </>
                            }
                        }
                        _ => {
                            html! {
                                <button
                                    onclick={on_open_modal}
                                    class="px-4 py-2 text-sm font-medium text-red-700 dark:text-red-300
                                           bg-white dark:bg-red-900/20 border border-red-300 dark:border-red-700
                                           rounded-md hover:bg-red-50 dark:hover:bg-red-900/30
                                           transition-colors"
                                >
                                    {"Delete Account"}
                                </button>
                            }
                        }
                    }
                }
            </div>

            // Delete Confirmation Modal
            if *show_delete_modal {
                <ConfirmationModal
                    title="Delete Account"
                    message="This will permanently delete your account and remove all your data."
                    confirm_text="Delete Account"
                    confirmation_value={username.clone()}
                    confirmation_label="your username"
                    on_confirm={on_delete}
                    on_close={on_close_modal}
                    is_loading={*is_deleting}
                    error_message={(*error_message).clone().map(AttrValue::from)}
                />
            }
        </div>
    }
}
