use payloads::{CommunityId, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{State, get_api_client};

/// Hook return type for sites data
pub struct SitesHookReturn {
    pub sites: Option<Vec<responses::Site>>,
    pub is_loading: bool,
    pub error: Option<String>,
    #[allow(dead_code)]
    pub refetch: Callback<()>,
}

impl SitesHookReturn {
    /// Returns true if this is the initial load (no data, no error, loading)
    pub fn is_initial_loading(&self) -> bool {
        self.is_loading && self.sites.is_none() && self.error.is_none()
    }
}

/// Hook to manage sites data with lazy loading and global state caching
#[hook]
pub fn use_sites(community_id: CommunityId) -> SitesHookReturn {
    let (state, dispatch) = use_store::<State>();
    let is_loading = use_state(|| false);
    let error = use_state(|| None::<String>);

    let refetch = {
        let dispatch = dispatch.clone();
        let is_loading = is_loading.clone();
        let error = error.clone();

        use_callback(community_id, move |community_id, _| {
            let dispatch = dispatch.clone();
            let is_loading = is_loading.clone();
            let error = error.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                match api_client.list_sites(&community_id).await {
                    Ok(sites) => {
                        dispatch.reduce_mut(|state| {
                            state.set_sites_for_community(community_id, sites);
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

    // Auto-load sites if not already loaded and user is authenticated
    {
        let refetch = refetch.clone();
        let state = state.clone();
        let is_loading = is_loading.clone();

        use_effect_with(
            (state.auth_state.clone(), community_id),
            move |(_, community_id)| {
                if state.is_authenticated()
                    && !state.has_sites_loaded_for_community(*community_id)
                    && !*is_loading
                {
                    refetch.emit(*community_id);
                }
            },
        );
    }

    // Consider it "loading" if actively loading OR if we're in initial state
    // (no data, no error yet)
    let sites = state
        .get_sites_for_community(community_id)
        .map(|site_refs| site_refs.into_iter().cloned().collect());
    let current_error = (*error).clone();
    let effective_is_loading =
        *is_loading || (sites.is_none() && current_error.is_none());

    SitesHookReturn {
        sites,
        is_loading: effective_is_loading,
        error: current_error,
        refetch: Callback::from(move |_| refetch.emit(community_id)),
    }
}
