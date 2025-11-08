use payloads::{CommunityId, responses};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::FetchState;

/// Hook return type for issued invites data
pub struct IssuedInvitesHookReturn {
    pub invites: FetchState<Vec<responses::IssuedCommunityInvite>>,
    pub is_loading: bool,
    pub error: Option<String>,
    #[allow(dead_code)]
    pub refetch: Callback<()>,
}

impl IssuedInvitesHookReturn {
    /// Returns true if this is the initial load (no data, no error, loading)
    #[allow(dead_code)]
    pub fn is_initial_loading(&self) -> bool {
        self.is_loading && !self.invites.is_fetched() && self.error.is_none()
    }
}

/// Hook to manage issued invites data with lazy loading
#[hook]
pub fn use_issued_invites(
    community_id: CommunityId,
) -> IssuedInvitesHookReturn {
    let invites = use_state(|| FetchState::NotFetched);
    let is_loading = use_state(|| true);
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
                        invites.set(FetchState::Fetched(issued_invites));
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

    // Auto-load invites on component mount and when community_id changes
    {
        let refetch = refetch.clone();

        use_effect_with(community_id, move |community_id| {
            refetch.emit(*community_id);
        });
    }

    IssuedInvitesHookReturn {
        invites: (*invites).clone(),
        is_loading: *is_loading,
        error: (*error).clone(),
        refetch: Callback::from(move |_| refetch.emit(community_id)),
    }
}
