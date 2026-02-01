use payloads::responses;
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{
    State, get_api_client,
    hooks::{FetchHookReturn, use_fetch_with_cache},
};

/// Hook to manage communities data with lazy loading and global state caching
#[hook]
pub fn use_communities() -> FetchHookReturn<Vec<responses::CommunityWithRole>> {
    let (state, dispatch) = use_store::<State>();

    let get_cached_state = state.clone();
    let should_fetch_state = state.clone();
    let fetch_dispatch = dispatch.clone();

    use_fetch_with_cache(
        (),
        move || {
            // get_communities returns FetchState, we need to convert to Option
            match get_cached_state.get_communities() {
                crate::hooks::FetchState::Fetched(communities) => {
                    Some(communities.clone())
                }
                crate::hooks::FetchState::NotFetched => None,
            }
        },
        move || !should_fetch_state.has_communities_loaded(),
        move || {
            let dispatch = fetch_dispatch.clone();
            async move {
                let api_client = get_api_client();
                let communities = api_client
                    .get_communities()
                    .await
                    .map_err(|e| e.to_string())?;
                dispatch.reduce_mut(|s| {
                    s.set_communities(communities.clone());
                });
                Ok(communities)
            }
        },
    )
}
