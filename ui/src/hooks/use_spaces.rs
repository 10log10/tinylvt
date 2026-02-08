use payloads::{SiteId, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{
    State, get_api_client,
    hooks::{FetchHookReturn, use_fetch_with_cache},
};

/// Hook to manage spaces data with lazy loading and global state caching
///
/// Hook Architecture Rationale:
/// This implements a consistent 3-tier hook hierarchy:
/// 1. `use_communities` - Fetches all communities for the user
/// 2. `use_sites(community_id)` - Fetches all sites for a specific community
/// 3. `use_spaces(site_id)` - Fetches all spaces for a specific site
///
/// This hierarchy allows for efficient data loading and caching at the
/// appropriate granularity. Each level caches its data in the global state,
/// preventing unnecessary re-fetches when components unmount and remount.
#[hook]
pub fn use_spaces(site_id: SiteId) -> FetchHookReturn<Vec<responses::Space>> {
    let (state, dispatch) = use_store::<State>();

    let get_cached_state = state.clone();
    let should_fetch_state = state.clone();
    let fetch_dispatch = dispatch.clone();

    use_fetch_with_cache(
        site_id,
        move || {
            get_cached_state
                .get_spaces_for_site(site_id)
                .map(|space_refs| space_refs.into_iter().cloned().collect())
        },
        move || !should_fetch_state.has_spaces_loaded_for_site(site_id),
        move || {
            let dispatch = fetch_dispatch.clone();
            async move {
                let api_client = get_api_client();
                let spaces = api_client
                    .list_spaces(&site_id)
                    .await
                    .map_err(|e| e.to_string())?;
                dispatch.reduce_mut(|s| {
                    s.set_spaces_for_site(site_id, spaces.clone());
                });
                Ok(spaces)
            }
        },
    )
}
