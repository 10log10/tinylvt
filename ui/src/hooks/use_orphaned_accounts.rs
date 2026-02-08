use payloads::{CommunityId, responses};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{FetchHookReturn, use_fetch};

/// Hook to manage orphaned accounts data with lazy loading
#[hook]
pub fn use_orphaned_accounts(
    community_id: CommunityId,
) -> FetchHookReturn<responses::OrphanedAccountsList> {
    use_fetch(community_id, move || async move {
        let api_client = get_api_client();
        api_client
            .get_orphaned_accounts(&community_id)
            .await
            .map_err(|e| e.to_string())
    })
}
