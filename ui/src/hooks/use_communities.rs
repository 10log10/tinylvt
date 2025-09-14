use payloads::{ClientError, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{State, get_api_client};

/// Hook return type for communities data
pub struct CommunitiesHookReturn {
    pub communities: Option<Vec<responses::CommunityWithRole>>,
    pub is_loading: bool,
    pub error: Option<String>,
    pub refetch: Callback<()>,
}

/// Hook to manage communities data with lazy loading and global state caching
#[hook]
pub fn use_communities() -> CommunitiesHookReturn {
    let (state, dispatch) = use_store::<State>();
    let is_loading = use_state(|| false);
    let error = use_state(|| None::<String>);

    let refetch = {
        let dispatch = dispatch.clone();
        let is_loading = is_loading.clone();
        let error = error.clone();

        use_callback((), move |_, _| {
            let dispatch = dispatch.clone();
            let is_loading = is_loading.clone();
            let error = error.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                match api_client.get_communities().await {
                    Ok(communities) => {
                        dispatch.reduce_mut(|state| {
                            state.set_communities(communities);
                        });
                        error.set(None);
                    }
                    Err(ClientError::APIError(_, msg)) => {
                        error.set(Some(msg));
                    }
                    Err(ClientError::Network(_)) => {
                        error.set(Some(
                            "Network error. Please check your connection."
                                .to_string(),
                        ));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    // Auto-load communities if not already loaded and user is authenticated
    {
        let refetch = refetch.clone();
        let state = state.clone();
        let is_loading = is_loading.clone();

        use_effect_with(state.auth_state.clone(), move |_| {
            if state.is_authenticated()
                && !state.has_communities_loaded()
                && !*is_loading
            {
                refetch.emit(());
            }
        });
    }

    // Consider it "loading" if actively loading OR if we're in initial state
    // (no data, no error yet)
    let communities = state.get_communities().clone();
    let current_error = (*error).clone();
    let effective_is_loading =
        *is_loading || (communities.is_none() && current_error.is_none());

    CommunitiesHookReturn {
        communities,
        is_loading: effective_is_loading,
        error: current_error,
        refetch,
    }
}
