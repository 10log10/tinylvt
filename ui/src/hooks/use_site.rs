use payloads::{SiteId, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{
    State, get_api_client,
    hooks::{FetchHookReturn, use_fetch_with_cache},
};

/// Hook to manage single site data with lazy loading and global state caching
///
/// ## Hook Architecture Rationale
///
/// This implements a consistent 3-tier hook hierarchy:
/// 1. `use_communities` - Fetches all communities for the user
/// 2. `use_sites(community_id)` - Fetches all sites for a specific community
/// 3. `use_site(site_id)` - Fetches a single site by ID
///
/// This enables flatter routes (`/sites/:id`) while maintaining efficient data
/// fetching at each granularity level. No `use_community` hook is needed since
/// `use_communities` already loads all user communities.
#[hook]
pub fn use_site(site_id: SiteId) -> FetchHookReturn<responses::Site> {
    let (state, dispatch) = use_store::<State>();

    let get_cached_state = state.clone();
    let should_fetch_state = state.clone();
    let fetch_dispatch = dispatch.clone();

    use_fetch_with_cache(
        site_id,
        move || get_cached_state.get_site(site_id).cloned(),
        move || !should_fetch_state.has_site_loaded(site_id),
        move || {
            let dispatch = fetch_dispatch.clone();
            async move {
                let api_client = get_api_client();
                let site = api_client
                    .get_site(&site_id)
                    .await
                    .map_err(|e| e.to_string())?;
                dispatch.reduce_mut(|s| {
                    s.set_site(site_id, site.clone());
                });
                Ok(site)
            }
        },
    )
}
