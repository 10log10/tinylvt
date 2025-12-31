use crate::components::{ConfirmationModal, RequireAuth};
use crate::get_api_client;
use crate::hooks::{FetchState, use_communities, use_logout};
use payloads::Role;
use payloads::responses::UserProfile;
use yew::prelude::*;

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

    // Modal and deletion state
    let show_delete_modal = use_state(|| false);
    let is_deleting = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    // Use communities hook to get cached communities
    let communities_hook = use_communities();

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
