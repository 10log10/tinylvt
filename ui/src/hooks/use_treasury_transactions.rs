use payloads::{CommunityId, requests, responses};
use yew::prelude::*;

use crate::{
    get_api_client,
    hooks::{FetchHookReturn, use_fetch},
};

/// Hook to fetch treasury transaction history with pagination
/// (coleader+ only)
#[hook]
pub fn use_treasury_transactions(
    community_id: CommunityId,
    limit: i64,
    offset: i64,
) -> FetchHookReturn<Vec<responses::MemberTransaction>> {
    use_fetch((community_id, limit, offset), move || async move {
        let api_client = get_api_client();
        let request = requests::GetTreasuryTransactions {
            community_id,
            limit,
            offset,
        };

        api_client
            .get_treasury_transactions(&request)
            .await
            .map_err(|e| e.to_string())
    })
}
