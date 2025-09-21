use payloads::{ClientError, CommunityId, responses};
use yew::prelude::*;

use crate::get_api_client;

/// Hook return type for issued invites data
pub struct IssuedInvitesHookReturn {
    pub invites: Option<Vec<responses::IssuedCommunityInvite>>,
    pub is_loading: bool,
    pub error: Option<String>,
    #[allow(dead_code)]
    pub refetch: Callback<()>,
}

/// Hook to manage issued invites data with lazy loading
#[hook]
pub fn use_issued_invites(
    community_id: CommunityId,
) -> IssuedInvitesHookReturn {
    let invites = use_state(|| None::<Vec<responses::IssuedCommunityInvite>>);
    let is_loading = use_state(|| false);
    let error = use_state(|| None::<String>);

    let refetch = {
        let invites = invites.clone();
        let is_loading = is_loading.clone();
        let error = error.clone();

        use_callback(community_id, move |community_id, _| {
            let invites = invites.clone();
            let is_loading = is_loading.clone();
            let error = error.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                match api_client.get_issued_invites(&community_id).await {
                    Ok(issued_invites) => {
                        invites.set(Some(issued_invites));
                        error.set(None);
                    }
                    Err(ClientError::APIError(_, msg)) => {
                        error.set(Some(msg));
                    }
                    Err(ClientError::Network(_)) => {
                        error.set(Some(
                            "Network error. Please check your connection."
                                .to_string(),
                        ));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    // Auto-load invites on component mount
    {
        let refetch = refetch.clone();
        let invites = invites.clone();
        let is_loading = is_loading.clone();

        use_effect_with(community_id, move |community_id| {
            if invites.is_none() && !*is_loading {
                refetch.emit(*community_id);
            }
        });
    }

    // Consider it "loading" if actively loading OR if we're in initial state
    // (no data, no error yet)
    let invites_data = (*invites).clone();
    let current_error = (*error).clone();
    let effective_is_loading =
        *is_loading || (invites_data.is_none() && current_error.is_none());

    IssuedInvitesHookReturn {
        invites: invites_data,
        is_loading: effective_is_loading,
        error: current_error,
        refetch: Callback::from(move |_| refetch.emit(community_id)),
    }
}
