use wasm_bindgen::JsCast;

use crate::hooks::{FetchState, use_communities};
use crate::{AuthState, Route, State, get_api_client};
use payloads::Role;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

#[function_component]
pub fn ProfilePage() -> Html {
    let (state, dispatch) = use_store::<State>();
    let navigator = use_navigator().unwrap();

    // Modal and deletion state
    let show_delete_modal = use_state(|| false);
    let delete_confirmation = use_state(String::new);
    let is_deleting = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    // Use communities hook to get cached communities
    let communities_hook = use_communities();

    // Backdrop click handling for modal
    let backdrop_ref = use_node_ref();

    // Extract leader communities from the hook data
    let leader_communities: Option<Vec<String>> =
        match &communities_hook.communities {
            FetchState::Fetched(communities) => {
                let leaders: Vec<String> = communities
                    .iter()
                    .filter(|c| c.user_role == Role::Leader)
                    .map(|c| c.name.clone())
                    .collect();
                Some(leaders)
            }
            _ => None,
        };

    // Redirect to landing page if not logged in
    let profile = match &state.auth_state {
        AuthState::LoggedIn(profile) => profile.clone(),
        AuthState::LoggedOut => {
            navigator.push(&Route::Landing);
            return html! {};
        }
        AuthState::Unknown => {
            return html! {
                <div class="text-center space-y-4">
                    <div class="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-neutral-900 dark:border-neutral-100"></div>
                    <p class="text-neutral-600 dark:text-neutral-400">{"Loading..."}</p>
                </div>
            };
        }
    };

    let username = profile.username.clone();
    let can_delete = *delete_confirmation == username;

    let on_open_modal = {
        let show_delete_modal = show_delete_modal.clone();
        let delete_confirmation = delete_confirmation.clone();
        let error_message = error_message.clone();
        Callback::from(move |_| {
            delete_confirmation.set(String::new());
            error_message.set(None);
            show_delete_modal.set(true);
        })
    };

    let on_close_modal = {
        let show_delete_modal = show_delete_modal.clone();
        Callback::from(move |_: MouseEvent| {
            show_delete_modal.set(false);
        })
    };

    let on_backdrop_click = {
        let show_delete_modal = show_delete_modal.clone();
        let backdrop_ref = backdrop_ref.clone();
        Callback::from(move |e: MouseEvent| {
            // Only close if clicking the backdrop itself, not its children
            if let Some(backdrop_element) =
                backdrop_ref.cast::<web_sys::Element>()
                && let Some(target) = e.target()
                && target.dyn_ref::<web_sys::Element>()
                    == Some(&backdrop_element)
            {
                show_delete_modal.set(false);
            }
        })
    };

    let on_confirmation_input = {
        let delete_confirmation = delete_confirmation.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            delete_confirmation.set(input.value());
        })
    };

    let on_delete = {
        let is_deleting = is_deleting.clone();
        let error_message = error_message.clone();
        let dispatch = dispatch.clone();
        let navigator = navigator.clone();

        Callback::from(move |_| {
            let is_deleting = is_deleting.clone();
            let error_message = error_message.clone();
            let dispatch = dispatch.clone();
            let navigator = navigator.clone();

            is_deleting.set(true);
            error_message.set(None);

            wasm_bindgen_futures::spawn_local(async move {
                let client = get_api_client();
                match client.delete_user().await {
                    Ok(_) => {
                        dispatch.reduce_mut(|state| {
                            state.auth_state = AuthState::LoggedOut;
                        });
                        navigator.push(&Route::Landing);
                    }
                    Err(e) => {
                        error_message.set(Some(e.to_string()));
                        is_deleting.set(false);
                    }
                }
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
                    if let Some(display_name) = &profile.display_name {
                        <div class="flex">
                            <span class="w-32 text-neutral-500 dark:text-neutral-400">{"Display Name"}</span>
                            <span class="text-neutral-900 dark:text-neutral-100">{display_name}</span>
                        </div>
                    }
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
                <div
                    ref={backdrop_ref.clone()}
                    onclick={on_backdrop_click.clone()}
                    class="fixed inset-0 bg-neutral-900 bg-opacity-50 z-50 flex items-center justify-center p-4"
                >
                    <div class="bg-white dark:bg-neutral-800 rounded-lg shadow-xl max-w-md w-full p-6">
                        <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                            {"Delete Account"}
                        </h3>

                        <div class="space-y-4">
                            <p class="text-sm text-neutral-600 dark:text-neutral-400">
                                {"This action "}
                                <span class="font-semibold text-red-600 dark:text-red-400">{"cannot be undone"}</span>
                                {". This will permanently delete your account and remove all your data."}
                            </p>

                            <p class="text-sm text-neutral-600 dark:text-neutral-400">
                                {"Please type "}
                                <span class="font-mono font-semibold text-neutral-900 dark:text-neutral-100">{&username}</span>
                                {" to confirm."}
                            </p>

                            <input
                                type="text"
                                value={(*delete_confirmation).clone()}
                                oninput={on_confirmation_input}
                                placeholder="Enter your username"
                                disabled={*is_deleting}
                                class="w-full px-3 py-2 text-sm border border-neutral-300 dark:border-neutral-600
                                       rounded-md bg-white dark:bg-neutral-700
                                       text-neutral-900 dark:text-neutral-100
                                       placeholder-neutral-400 dark:placeholder-neutral-500
                                       focus:outline-none focus:ring-2 focus:ring-red-500 focus:border-red-500
                                       disabled:opacity-50 disabled:cursor-not-allowed"
                            />

                            if let Some(error) = &*error_message {
                                <div class="text-sm text-red-600 dark:text-red-400">
                                    {error}
                                </div>
                            }
                        </div>

                        <div class="flex justify-end gap-3 mt-6">
                            <button
                                onclick={on_close_modal}
                                disabled={*is_deleting}
                                class="px-4 py-2 text-sm font-medium text-neutral-700 dark:text-neutral-300
                                       bg-white dark:bg-neutral-700 border border-neutral-300 dark:border-neutral-600
                                       rounded-md hover:bg-neutral-50 dark:hover:bg-neutral-600
                                       disabled:opacity-50 disabled:cursor-not-allowed
                                       transition-colors"
                            >
                                {"Cancel"}
                            </button>
                            <button
                                onclick={on_delete}
                                disabled={!can_delete || *is_deleting}
                                class="px-4 py-2 text-sm font-medium text-white
                                       bg-red-600 hover:bg-red-700 dark:bg-red-700 dark:hover:bg-red-600
                                       rounded-md disabled:opacity-50 disabled:cursor-not-allowed
                                       transition-colors"
                            >
                                {if *is_deleting { "Deleting..." } else { "Delete Account" }}
                            </button>
                        </div>
                    </div>
                </div>
            }
        </div>
    }
}
