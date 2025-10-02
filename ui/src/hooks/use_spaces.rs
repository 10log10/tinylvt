use payloads::{ClientError, SiteId, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{State, get_api_client};

/// Hook return type for spaces data
pub struct SpacesHookReturn {
    pub spaces: Option<Vec<responses::Space>>,
    pub is_loading: bool,
    pub error: Option<String>,
    #[allow(dead_code)]
    pub refetch: Callback<()>,
}

/// Hook to manage spaces data with lazy loading and global state caching
///
/// Hook Architecture Rationale:
/// This implements a consistent 3-tier hook hierarchy:
/// 1. `use_communities` - Fetches all communities for the user
/// 2. `use_sites(community_id)` - Fetches all sites for a specific community
/// 3. `use_spaces(site_id)` - Fetches all spaces for a specific site
///
/// This hierarchy allows for efficient data loading and caching at the appropriate
/// granularity. Each level caches its data in the global state, preventing
/// unnecessary re-fetches when components unmount and remount.
#[hook]
pub fn use_spaces(site_id: SiteId) -> SpacesHookReturn {
    let (state, dispatch) = use_store::<State>();
    let is_loading = use_state(|| false);
    let error = use_state(|| None::<String>);

    let refetch = {
        let dispatch = dispatch.clone();
        let is_loading = is_loading.clone();
        let error = error.clone();

        use_callback(site_id, move |site_id, _| {
            let dispatch = dispatch.clone();
            let is_loading = is_loading.clone();
            let error = error.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                match api_client.list_spaces(&site_id).await {
                    Ok(spaces) => {
                        dispatch.reduce_mut(|state| {
                            state.set_spaces_for_site(site_id, spaces);
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

    // Auto-load spaces if not already loaded and user is authenticated
    {
        let refetch = refetch.clone();
        let state = state.clone();
        let is_loading = is_loading.clone();

        use_effect_with(
            (state.auth_state.clone(), site_id),
            move |(_, site_id)| {
                if state.is_authenticated()
                    && !state.has_spaces_loaded_for_site(*site_id)
                    && !*is_loading
                {
                    refetch.emit(*site_id);
                }
            },
        );
    }

    // Consider it "loading" if actively loading OR if we're in initial state
    // (no data, no error yet)
    let spaces = state
        .get_spaces_for_site(site_id)
        .map(|space_refs| space_refs.into_iter().cloned().collect());
    let current_error = (*error).clone();
    let effective_is_loading =
        *is_loading || (spaces.is_none() && current_error.is_none());

    SpacesHookReturn {
        spaces,
        is_loading: effective_is_loading,
        error: current_error,
        refetch: Callback::from(move |_| refetch.emit(site_id)),
    }
}
