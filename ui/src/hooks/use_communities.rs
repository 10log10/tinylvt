use payloads::responses;
use yew::prelude::*;
use yewdux::prelude::*;

use crate::hooks::FetchState;
use crate::{State, get_api_client};

/// Hook return type for communities data
pub struct CommunitiesHookReturn {
    pub communities: FetchState<Vec<responses::CommunityWithRole>>,
    pub is_loading: bool,
    pub error: Option<String>,
    pub refetch: Callback<()>,
}

impl CommunitiesHookReturn {
    /// Returns true if this is the initial load (no data, no error, loading)
    pub fn is_initial_loading(&self) -> bool {
        self.is_loading
            && !self.communities.is_fetched()
            && self.error.is_none()
    }
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
                    Err(e) => {
                        error.set(Some(e.to_string()));
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

    CommunitiesHookReturn {
        communities: state.get_communities().clone(),
        is_loading: *is_loading,
        error: (*error).clone(),
        refetch,
    }
}
