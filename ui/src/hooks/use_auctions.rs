use payloads::{SiteId, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{
    State, get_api_client,
    hooks::{FetchHookReturn, use_fetch_with_cache},
};

/// Hook to manage auctions data with global state caching.
///
/// Unlike the per-auction detail hook, the site auction list isn't
/// SSE-subscribed (its scope is every auction in the site, not a single
/// auction id), so lifecycle changes made elsewhere — canceling or deleting
/// an auction on its detail page — aren't pushed here. To avoid showing a
/// stale list after such a change, this refetches on every mount rather than
/// only on a cold cache (`should_fetch` is always `true`).
///
/// Because we always refetch, the yewdux cache no longer exists to *save* a
/// request. It's kept for one reason: rendering the previously-cached list
/// immediately via `get_cached` while the refetch runs in the background, so
/// revisiting the auctions tab never flashes a loading spinner.
#[hook]
pub fn use_auctions(
    site_id: SiteId,
) -> FetchHookReturn<Vec<responses::Auction>> {
    let (state, dispatch) = use_store::<State>();

    let get_cached_state = state.clone();
    let fetch_dispatch = dispatch.clone();

    use_fetch_with_cache(
        site_id,
        move || {
            get_cached_state
                .get_auctions_for_site(site_id)
                .map(|auction_refs| auction_refs.into_iter().cloned().collect())
        },
        || true,
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
