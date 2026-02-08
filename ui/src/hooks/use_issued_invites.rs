use payloads::{CommunityId, responses};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{FetchHookReturn, use_fetch};

/// Hook to manage issued invites data with lazy loading
#[hook]
pub fn use_issued_invites(
    community_id: CommunityId,
) -> FetchHookReturn<Vec<responses::IssuedCommunityInvite>> {
    use_fetch(community_id, move || async move {
        let api_client = get_api_client();
        api_client
            .get_issued_invites(&community_id)
            .await
            .map_err(|e| e.to_string())
    })
}
