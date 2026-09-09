use payloads::{CommunityId, responses};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{FetchHookReturn, use_fetch};

/// Hook to fetch a community's space categories, sorted by name
/// (server-side). Used by the space forms' category selector, the cap
/// editor, and the bidding view's per-category capacity display.
#[hook]
pub fn use_space_categories(
    community_id: CommunityId,
) -> FetchHookReturn<Vec<responses::SpaceCategory>> {
    use_fetch(community_id, move || async move {
        let api_client = get_api_client();
        api_client
            .list_space_categories(&community_id)
            .await
            .map_err(|e| e.to_string())
    })
}
