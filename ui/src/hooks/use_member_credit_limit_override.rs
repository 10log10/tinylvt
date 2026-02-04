use payloads::{CommunityId, UserId, requests, responses};
use yew::prelude::*;

use crate::{
    get_api_client,
    hooks::{FetchHookReturn, use_fetch},
};

/// Hook to fetch a member's credit limit (moderator+ only)
#[hook]
pub fn use_member_credit_limit_override(
    community_id: CommunityId,
    member_user_id: UserId,
) -> FetchHookReturn<responses::MemberCreditLimitOverride> {
    use_fetch((community_id, member_user_id), move || async move {
        let api_client = get_api_client();
        let request = requests::GetMemberCreditLimitOverride {
            community_id,
            member_user_id,
        };

        api_client
            .get_member_credit_limit_override(&request)
            .await
            .map_err(|e| e.to_string())
    })
}
