use payloads::{CommunityId, UserId, requests, responses};
use yew::prelude::*;

use crate::{
    get_api_client,
    hooks::{FetchHookReturn, use_fetch},
};

/// Hook to fetch member transaction history with pagination
/// If member_user_id is None, fetches for the current user
#[hook]
pub fn use_member_transactions(
    community_id: CommunityId,
    member_user_id: Option<UserId>,
    limit: i64,
    offset: i64,
) -> FetchHookReturn<Vec<responses::MemberTransaction>> {
    use_fetch(
        (community_id, member_user_id, limit, offset),
        move || async move {
            let api_client = get_api_client();
            let request = requests::GetMemberTransactions {
                community_id,
                member_user_id,
                limit,
                offset,
            };

            api_client
                .get_member_transactions(&request)
                .await
                .map_err(|e| e.to_string())
        },
    )
}
