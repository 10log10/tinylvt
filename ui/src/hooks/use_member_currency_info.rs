use payloads::{CommunityId, UserId, requests, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{State, get_api_client};

/// Hook return type for member currency info
pub struct MemberCurrencyInfoHookReturn {
    pub info: Option<responses::MemberCurrencyInfo>,
    pub is_loading: bool,
    pub error: Option<String>,
    pub refetch: Callback<()>,
}

impl MemberCurrencyInfoHookReturn {
    /// Returns true if this is the initial load
    pub fn is_initial_loading(&self) -> bool {
        self.is_loading && self.info.is_none() && self.error.is_none()
    }
}

/// Hook to fetch member currency info (balance, credit limit, etc.)
/// If member_user_id is None, fetches for the current user
#[hook]
pub fn use_member_currency_info(
    community_id: CommunityId,
    member_user_id: Option<UserId>,
) -> MemberCurrencyInfoHookReturn {
    let (state, _dispatch) = use_store::<State>();
    let is_loading = use_state(|| false);
    let error = use_state(|| None::<String>);
    let info = use_state(|| None::<responses::MemberCurrencyInfo>);

    let refetch = {
        let is_loading = is_loading.clone();
        let error = error.clone();
        let info = info.clone();

        use_callback(
            (community_id, member_user_id),
            move |(community_id, member_user_id), _| {
                let is_loading = is_loading.clone();
                let error = error.clone();
                let info = info.clone();

                yew::platform::spawn_local(async move {
                    is_loading.set(true);
                    error.set(None);

                    let api_client = get_api_client();
                    let request = requests::GetMemberCurrencyInfo {
                        community_id,
                        member_user_id,
                    };

                    match api_client.get_member_currency_info(&request).await {
                        Ok(currency_info) => {
                            info.set(Some(currency_info));
                            error.set(None);
                        }
                        Err(e) => {
                            error.set(Some(e.to_string()));
                        }
                    }

                    is_loading.set(false);
                });
            },
        )
    };

    // Auto-load currency info if authenticated
    {
        let refetch = refetch.clone();
        let state = state.clone();
        let is_loading = is_loading.clone();
        let info_state = info.clone();

        use_effect_with(
            (state.auth_state.clone(), community_id, member_user_id),
            move |(_, community_id, member_user_id)| {
                if state.is_authenticated()
                    && info_state.is_none()
                    && !*is_loading
                {
                    refetch.emit((*community_id, *member_user_id));
                }
            },
        );
    }

    let current_info = (*info).clone();
    let current_error = (*error).clone();
    let effective_is_loading =
        *is_loading || (current_info.is_none() && current_error.is_none());

    MemberCurrencyInfoHookReturn {
        info: current_info,
        is_loading: effective_is_loading,
        error: current_error,
        refetch: Callback::from(move |_| {
            refetch.emit((community_id, member_user_id))
        }),
    }
}
