use payloads::{Account, CommunityId, requests};
use yew::prelude::*;

use crate::{
    get_api_client,
    hooks::{FetchHookReturn, use_fetch},
};

/// Hook to fetch treasury account info (coleader+ only)
#[hook]
pub fn use_treasury_account(
    community_id: CommunityId,
) -> FetchHookReturn<Account> {
    use_fetch(community_id, move || async move {
        let api_client = get_api_client();
        let request = requests::GetTreasuryAccount { community_id };

        api_client
            .get_treasury_account(&request)
            .await
            .map_err(|e| e.to_string())
    })
}
