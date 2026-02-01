use payloads::{CommunityId, UserId, requests, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{State, get_api_client};

/// Hook return type for member transactions
pub struct MemberTransactionsHookReturn {
    pub transactions: Option<Vec<responses::MemberTransaction>>,
    pub is_loading: bool,
    pub error: Option<String>,
    pub refetch: Callback<()>,
}

impl MemberTransactionsHookReturn {
    /// Returns true if this is the initial load
    pub fn is_initial_loading(&self) -> bool {
        self.is_loading && self.transactions.is_none() && self.error.is_none()
    }
}

/// Hook to fetch member transaction history with pagination
/// If member_user_id is None, fetches for the current user
#[hook]
pub fn use_member_transactions(
    community_id: CommunityId,
    member_user_id: Option<UserId>,
    limit: i64,
    offset: i64,
) -> MemberTransactionsHookReturn {
    let (state, _dispatch) = use_store::<State>();
    let is_loading = use_state(|| false);
    let error = use_state(|| None::<String>);
    let transactions = use_state(|| None::<Vec<responses::MemberTransaction>>);

    let refetch = {
        let is_loading = is_loading.clone();
        let error = error.clone();
        let transactions = transactions.clone();

        use_callback(
            (community_id, member_user_id, limit, offset),
            move |(community_id, member_user_id, limit, offset), _| {
                let is_loading = is_loading.clone();
                let error = error.clone();
                let transactions = transactions.clone();

                yew::platform::spawn_local(async move {
                    is_loading.set(true);
                    error.set(None);

                    let api_client = get_api_client();
                    let request = requests::GetMemberTransactions {
                        community_id,
                        member_user_id,
                        limit,
                        offset,
                    };

                    match api_client.get_member_transactions(&request).await {
                        Ok(txns) => {
                            transactions.set(Some(txns));
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

    // Auto-load transactions if authenticated
    {
        let refetch = refetch.clone();
        let state = state.clone();
        let is_loading = is_loading.clone();
        let transactions_state = transactions.clone();

        use_effect_with(
            (
                state.auth_state.clone(),
                community_id,
                member_user_id,
                limit,
                offset,
            ),
            move |(_, community_id, member_user_id, limit, offset)| {
                if state.is_authenticated()
                    && transactions_state.is_none()
                    && !*is_loading
                {
                    refetch.emit((
                        *community_id,
                        *member_user_id,
                        *limit,
                        *offset,
                    ));
                }
            },
        );
    }

    let current_transactions = (*transactions).clone();
    let current_error = (*error).clone();
    let effective_is_loading = *is_loading
        || (current_transactions.is_none() && current_error.is_none());

    MemberTransactionsHookReturn {
        transactions: current_transactions,
        is_loading: effective_is_loading,
        error: current_error,
        refetch: Callback::from(move |_| {
            refetch.emit((community_id, member_user_id, limit, offset))
        }),
    }
}
