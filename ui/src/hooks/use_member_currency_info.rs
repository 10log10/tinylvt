use payloads::{CommunityId, UserId, requests, responses};
use yew::prelude::*;

use crate::{
    get_api_client,
    hooks::{FetchHookReturn, use_fetch},
};

/// Hook to fetch member currency info (balance, credit limit, etc.)
/// If member_user_id is None, fetches for the current user
#[hook]
pub fn use_member_currency_info(
    community_id: CommunityId,
    member_user_id: Option<UserId>,
) -> FetchHookReturn<responses::MemberCurrencyInfo> {
    use_fetch((community_id, member_user_id), move || async move {
        let api_client = get_api_client();
        let request = requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id,
        };

        api_client
            .get_member_currency_info(&request)
            .await
            .map_err(|e| e.to_string())
    })
}
