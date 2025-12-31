use payloads::{InviteId, responses};
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

use crate::components::AuthForm;
use crate::contexts::use_toast;
use crate::{AuthState, Route, State};

#[derive(Properties, PartialEq)]
pub struct AcceptInvitePageProps {
    pub invite_id: InviteId,
}

#[function_component]
pub fn AcceptInvitePage(props: &AcceptInvitePageProps) -> Html {
    let navigator = use_navigator().unwrap();
    let (state, dispatch) = use_store::<State>();
    let toast = use_toast();

    let invite_id = props.invite_id;
    let is_accepting = use_state(|| false);
    let error_message = use_state(|| None::<String>);
    let community_name = use_state(|| None::<String>);
    let is_loading_name = use_state(|| true);

    // Fetch community name when component loads
    {
        let community_name = community_name.clone();
        let is_loading_name = is_loading_name.clone();
        let error_message = error_message.clone();

        use_effect_with(invite_id, move |invite_id| {
            let community_name = community_name.clone();
            let is_loading_name = is_loading_name.clone();
            let error_message = error_message.clone();
            let invite_id = *invite_id; // Copy the InviteId since it implements Copy

            yew::platform::spawn_local(async move {
                let api_client = crate::get_api_client();
                match api_client.get_invite_community_name(&invite_id).await {
                    Ok(name) => {
                        community_name.set(Some(name));
                        is_loading_name.set(false);
                    }
                    Err(err) => {
                        error_message.set(Some(format!(
                            "Failed to load community name: {}",
                            err
                        )));
                        is_loading_name.set(false);
                    }
                }
            });
        });
    }

    // Handle accepting the invite (closure that takes no arguments)
    let accept_invite = {
        let is_accepting = is_accepting.clone();
        let error_message = error_message.clone();
        let navigator = navigator.clone();
        let toast = toast.clone();
        let dispatch = dispatch.clone();

        move || {
            let is_accepting = is_accepting.clone();
            let error_message = error_message.clone();
            let navigator = navigator.clone();
            let toast = toast.clone();
            let dispatch = dispatch.clone();

            yew::platform::spawn_local(async move {
                is_accepting.set(true);
                error_message.set(None);

                let api_client = crate::get_api_client();
                match api_client.accept_invite(&invite_id).await {
                    Ok(()) => {
                        toast.success("Successfully joined community!");

                        // Clear communities cache to force refresh
                        dispatch.reduce_mut(|state| {
                            state.clear_communities();
                        });

                        // Navigate to communities page
                        navigator.push(&Route::Communities);
                    }
                    Err(e) => {
                        error_message.set(Some(e.to_string()));
                    }
                }

                is_accepting.set(false);
            });
        }
    };

    // Show loading if either auth or community name are loading
    if matches!(state.auth_state, AuthState::Unknown) || *is_loading_name {
        return html! {
            <div class="flex items-center justify-center min-h-[60vh]">
                <div class="text-center">
                    <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-neutral-600 dark:border-neutral-400 mx-auto mb-4"></div>
                    <p class="text-neutral-600 dark:text-neutral-400">{"Loading..."}</p>
                </div>
            </div>
        };
    }

    match &state.auth_state {
        AuthState::Unknown => {
            // This case is handled above in the loading check
            unreachable!()
        }
        AuthState::LoggedOut => {
            // User needs to login - show auth form inline
            html! {
                <div class="flex items-center justify-center min-h-[60vh]">
                    <AuthForm
                        login_title="Community Invite"
                        login_description={
                            AttrValue::from(
                                if let Some(name) = &*community_name {
                                    format!("You've been invited to join {} on TinyLVT. Please sign in to accept this invitation.", name)
                                } else {
                                    "You've been invited to join a community on TinyLVT. Please sign in to accept this invitation.".to_string()
                                }
                            )
                        }
                        login_submit_text="Sign in to Accept Invite"
                        signup_title="Create Account for Invite"
                        signup_description={
                            AttrValue::from(
                                if let Some(name) = &*community_name {
                                    format!("You've been invited to join {} on TinyLVT. Create an account to accept this invitation.", name)
                                } else {
                                    "You've been invited to join a community on TinyLVT. Create an account to accept this invitation.".to_string()
                                }
                            )
                        }
                        signup_submit_text="Create Account"
                        on_success={Callback::noop()}
                    />
                </div>
            }
        }
        AuthState::LoggedIn(_) => {
            // User is authenticated, show invite acceptance UI
            html! {
                <div class="flex items-center justify-center min-h-[60vh]">
                    <div class="max-w-md w-full bg-white dark:bg-neutral-800 p-8 rounded-lg shadow-md">
                        <div class="mb-8 text-center">
                            <h1 class="text-2xl font-bold text-neutral-900 dark:text-neutral-100 mb-2">
                                {"Accept Community Invite"}
                            </h1>
                            <p class="text-neutral-600 dark:text-neutral-400">
                                {
                                    if let Some(name) = &*community_name {
                                        format!("You've been invited to join {} on TinyLVT.", name)
                                    } else {
                                        "You've been invited to join a community on TinyLVT.".to_string()
                                    }
                                }
                            </p>
                        </div>

                        if let Some(error) = &*error_message {
                            <div class="mb-6 p-4 rounded-md bg-red-50 dark:bg-red-900 border border-red-200 dark:border-red-800">
                                <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
                            </div>
                        }

                        <div class="space-y-4">
                            <button
                                onclick={{
                                    let accept_invite = accept_invite.clone();
                                    Callback::from(move |_| {
                                        accept_invite();
                                    })
                                }}
                                disabled={*is_accepting}
                                class="w-full flex justify-center py-2 px-4 border border-transparent
                                       rounded-md shadow-sm text-sm font-medium text-white
                                       bg-neutral-900 hover:bg-neutral-800 
                                       dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200
                                       focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                                       disabled:opacity-50 disabled:cursor-not-allowed
                                       transition-colors duration-200"
                            >
                                if *is_accepting {
                                    {"Accepting..."}
                                } else {
                                    {"Accept Invite"}
                                }
                            </button>

                            <button
                                onclick={Callback::from(move |_| {
                                    navigator.push(&Route::Communities);
                                })}
                                class="w-full flex justify-center py-2 px-4 border border-neutral-300 dark:border-neutral-600
                                       rounded-md shadow-sm text-sm font-medium text-neutral-700 dark:text-neutral-300
                                       bg-white dark:bg-neutral-700 hover:bg-neutral-50 dark:hover:bg-neutral-600
                                       focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                                       transition-colors duration-200"
                            >
                                {"Cancel"}
                            </button>
                        </div>
                    </div>
                </div>
            }
        }
    }
}
