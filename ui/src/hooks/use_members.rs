use payloads::{ClientError, CommunityId, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{State, get_api_client};

/// Hook return type for members data
pub struct MembersHookReturn {
    pub members: Option<Vec<responses::CommunityMember>>,
    pub is_loading: bool,
    pub error: Option<String>,
    #[allow(dead_code)]
    pub refetch: Callback<()>,
}

/// Hook to manage members data with lazy loading and global state caching
#[hook]
pub fn use_members(community_id: CommunityId) -> MembersHookReturn {
    let (state, dispatch) = use_store::<State>();
    let is_loading = use_state(|| false);
    let error = use_state(|| None::<String>);

    let refetch = {
        let dispatch = dispatch.clone();
        let is_loading = is_loading.clone();
        let error = error.clone();

        use_callback(community_id, move |community_id, _| {
            let dispatch = dispatch.clone();
            let is_loading = is_loading.clone();
            let error = error.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                match api_client.get_members(&community_id).await {
                    Ok(members) => {
                        dispatch.reduce_mut(|state| {
                            state.set_members_for_community(
                                community_id,
                                members,
                            );
                        });
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

    // Auto-load members if not already loaded and user is authenticated
    {
        let refetch = refetch.clone();
        let state = state.clone();
        let is_loading = is_loading.clone();

        use_effect_with(
            (state.auth_state.clone(), community_id),
            move |(_, community_id)| {
                if state.is_authenticated()
                    && !state.has_members_loaded_for_community(*community_id)
                    && !*is_loading
                {
                    refetch.emit(*community_id);
                }
            },
        );
    }

    // Consider it "loading" if actively loading OR if we're in initial state
    // (no data, no error yet)
    let members = state.get_members_for_community(community_id).cloned();
    let current_error = (*error).clone();
    let effective_is_loading =
        *is_loading || (members.is_none() && current_error.is_none());

    MembersHookReturn {
        members,
        is_loading: effective_is_loading,
        error: current_error,
        refetch: Callback::from(move |_| refetch.emit(community_id)),
    }
}
