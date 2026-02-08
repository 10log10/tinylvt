use payloads::{CommunityId, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{
    State, get_api_client,
    hooks::{FetchHookReturn, use_fetch_with_cache},
};

/// Hook to manage sites data with lazy loading and global state caching
#[hook]
pub fn use_sites(
    community_id: CommunityId,
) -> FetchHookReturn<Vec<responses::Site>> {
    let (state, dispatch) = use_store::<State>();

    let get_cached_state = state.clone();
    let should_fetch_state = state.clone();
    let fetch_dispatch = dispatch.clone();

    use_fetch_with_cache(
        community_id,
        move || {
            get_cached_state
                .get_sites_for_community(community_id)
                .map(|site_refs| site_refs.into_iter().cloned().collect())
        },
        move || {
            !should_fetch_state.has_sites_loaded_for_community(community_id)
        },
        move || {
            let dispatch = fetch_dispatch.clone();
            async move {
                let api_client = get_api_client();
                let sites = api_client
                    .list_sites(&community_id)
                    .await
                    .map_err(|e| e.to_string())?;
                dispatch.reduce_mut(|s| {
                    s.set_sites_for_community(community_id, sites.clone());
                });
                Ok(sites)
            }
        },
    )
}
