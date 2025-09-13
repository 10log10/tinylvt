use payloads::{ClientError, CommunityId, responses};
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

    SitesHookReturn {
        sites: state.get_sites_for_community(community_id).cloned(),
        is_loading: *is_loading,
        error: (*error).clone(),
        refetch: Callback::from(move |_| refetch.emit(community_id)),
    }
}
