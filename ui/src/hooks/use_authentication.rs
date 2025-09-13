use yew::prelude::*;
use yewdux::prelude::*;

use crate::{AuthState, State, get_api_client};

/// Hook to automatically check authentication status on startup
#[hook]
pub fn use_authentication() {
    let (_state, dispatch) = use_store::<State>();

    // Check authentication status on app startup
    use_effect_with((), {
        let dispatch = dispatch.clone();
        move |_| {
            yew::platform::spawn_local(async move {
                let api_client = get_api_client();
                match api_client.login_check().await {
                    Ok(true) => {
                        // User has valid session, get their profile
                        match api_client.user_profile().await {
                            Ok(profile) => {
                                dispatch.reduce_mut(|state| {
                                    state.auth_state =
                                        AuthState::LoggedIn(profile);
                                });
                            }
                            Err(_) => {
                                dispatch.reduce_mut(|state| {
                                    state.logout();
                                });
                            }
                        }
                    }
                    Ok(false) => {
                        dispatch.reduce_mut(|state| {
                            state.logout();
                        });
                    }
                    Err(_) => {
                        // Network error or other issue, assume logged out
                        dispatch.reduce_mut(|state| {
                            state.logout();
                        });
                    }
                }
            });
        }
    });
}
