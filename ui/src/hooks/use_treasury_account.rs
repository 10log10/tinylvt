use payloads::{Account, CommunityId, requests};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{State, get_api_client};

/// Hook return type for treasury account
pub struct TreasuryAccountHookReturn {
    pub account: Option<Account>,
    pub is_loading: bool,
    pub error: Option<String>,
    pub refetch: Callback<()>,
}

/// Hook to fetch treasury account info (coleader+ only)
#[hook]
pub fn use_treasury_account(
    community_id: CommunityId,
) -> TreasuryAccountHookReturn {
    let (state, _dispatch) = use_store::<State>();
    let is_loading = use_state(|| false);
    let error = use_state(|| None::<String>);
    let account = use_state(|| None::<Account>);

    let refetch = {
        let is_loading = is_loading.clone();
        let error = error.clone();
        let account = account.clone();

        use_callback(community_id, move |community_id, _| {
            let is_loading = is_loading.clone();
            let error = error.clone();
            let account = account.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                let request = requests::GetTreasuryAccount { community_id };

                match api_client.get_treasury_account(&request).await {
                    Ok(treasury_account) => {
                        account.set(Some(treasury_account));
                        error.set(None);
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    // Auto-load treasury account if authenticated
    {
        let refetch = refetch.clone();
        let state = state.clone();
        let is_loading = is_loading.clone();
        let account_state = account.clone();

        use_effect_with(
            (state.auth_state.clone(), community_id),
            move |(_, community_id)| {
                if state.is_authenticated()
                    && account_state.is_none()
                    && !*is_loading
                {
                    refetch.emit(*community_id);
                }
            },
        );
    }

    let current_account = (*account).clone();
    let current_error = (*error).clone();
    let effective_is_loading =
        *is_loading || (current_account.is_none() && current_error.is_none());

    TreasuryAccountHookReturn {
        account: current_account,
        is_loading: effective_is_loading,
        error: current_error,
        refetch: Callback::from(move |_| refetch.emit(community_id)),
    }
}
