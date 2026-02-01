use payloads::{CommunityId, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{
    State, get_api_client,
    hooks::{FetchHookReturn, use_fetch_with_cache},
};

/// Hook to manage members data with lazy loading and global state caching
#[hook]
pub fn use_members(
    community_id: CommunityId,
) -> FetchHookReturn<Vec<responses::CommunityMember>> {
    let (state, dispatch) = use_store::<State>();

    let get_cached_state = state.clone();
    let should_fetch_state = state.clone();
    let fetch_dispatch = dispatch.clone();

    use_fetch_with_cache(
        community_id,
        move || {
            get_cached_state
                .get_members_for_community(community_id)
                .cloned()
        },
        move || {
            !should_fetch_state.has_members_loaded_for_community(community_id)
        },
        move || {
            let dispatch = fetch_dispatch.clone();
            async move {
                let api_client = get_api_client();
                let members = api_client
                    .get_members(&community_id)
                    .await
                    .map_err(|e| e.to_string())?;
                dispatch.reduce_mut(|s| {
                    s.set_members_for_community(community_id, members.clone());
                });
                Ok(members)
            }
        },
    )
}
