use payloads::{SiteId, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{
    State, get_api_client,
    hooks::{FetchHookReturn, use_fetch_with_cache},
};

/// Hook to manage auctions data with lazy loading and global state caching
///
/// This follows the same pattern as use_spaces and use_sites, providing
/// efficient data loading and caching for auctions at the site level.
#[hook]
pub fn use_auctions(
    site_id: SiteId,
) -> FetchHookReturn<Vec<responses::Auction>> {
    let (state, dispatch) = use_store::<State>();

    let get_cached_state = state.clone();
    let should_fetch_state = state.clone();
    let fetch_dispatch = dispatch.clone();

    use_fetch_with_cache(
        site_id,
        move || {
            get_cached_state
                .get_auctions_for_site(site_id)
                .map(|auction_refs| auction_refs.into_iter().cloned().collect())
        },
        move || !should_fetch_state.has_auctions_loaded_for_site(site_id),
        move || {
            let dispatch = fetch_dispatch.clone();
            async move {
                let api_client = get_api_client();
                let auctions = api_client
                    .list_auctions(&site_id)
                    .await
                    .map_err(|e| e.to_string())?;
                dispatch.reduce_mut(|s| {
                    s.set_auctions_for_site(site_id, auctions.clone());
                });
                Ok(auctions)
            }
        },
    )
}
