use payloads::{CommunityId, requests, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{State, get_api_client};

/// Hook return type for treasury transactions
pub struct TreasuryTransactionsHookReturn {
    pub transactions: Option<Vec<responses::MemberTransaction>>,
    pub is_loading: bool,
    pub error: Option<String>,
    pub refetch: Callback<()>,
}

/// Hook to fetch treasury transaction history with pagination
/// (coleader+ only)
#[hook]
pub fn use_treasury_transactions(
    community_id: CommunityId,
    limit: i64,
    offset: i64,
) -> TreasuryTransactionsHookReturn {
    let (state, _dispatch) = use_store::<State>();
    let is_loading = use_state(|| false);
    let error = use_state(|| None::<String>);
    let transactions = use_state(|| None::<Vec<responses::MemberTransaction>>);

    let refetch = {
        let is_loading = is_loading.clone();
        let error = error.clone();
        let transactions = transactions.clone();

        use_callback(
            (community_id, limit, offset),
            move |(community_id, limit, offset), _| {
                let is_loading = is_loading.clone();
                let error = error.clone();
                let transactions = transactions.clone();

                yew::platform::spawn_local(async move {
                    is_loading.set(true);
                    error.set(None);

                    let api_client = get_api_client();
                    let request = requests::GetTreasuryTransactions {
                        community_id,
                        limit,
                        offset,
                    };

                    match api_client.get_treasury_transactions(&request).await {
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
            (state.auth_state.clone(), community_id, limit, offset),
            move |(_, community_id, limit, offset)| {
                if state.is_authenticated()
                    && transactions_state.is_none()
                    && !*is_loading
                {
                    refetch.emit((*community_id, *limit, *offset));
                }
            },
        );
    }

    let current_transactions = (*transactions).clone();
    let current_error = (*error).clone();
    let effective_is_loading = *is_loading
        || (current_transactions.is_none() && current_error.is_none());

    TreasuryTransactionsHookReturn {
        transactions: current_transactions,
        is_loading: effective_is_loading,
        error: current_error,
        refetch: Callback::from(move |_| {
            refetch.emit((community_id, limit, offset))
        }),
    }
}
