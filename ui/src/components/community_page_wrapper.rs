use payloads::{CommunityId, responses::CommunityWithRole};
use yew::prelude::*;

use crate::components::RequireAuth;
use crate::hooks::{render_section, use_communities};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    pub children: Callback<CommunityWithRole, Html>,
}

#[function_component]
pub fn CommunityPageWrapper(props: &Props) -> Html {
    html! {
        <RequireAuth>
            <CommunityPageWrapperInner
                community_id={props.community_id}
                children={props.children.clone()}
            />
        </RequireAuth>
    }
}

// Inner component that only renders when authenticated
#[function_component]
fn CommunityPageWrapperInner(props: &Props) -> Html {
    let communities_hook = use_communities();

    render_section(
        &communities_hook.inner,
        "community",
        |communities, _is_loading, _errors| match communities
            .iter()
            .find(|c| c.id == props.community_id)
        {
            Some(community) => html! {
                <div>
                    {props.children.emit(community.clone())}
                </div>
            },
            None => html! {
                <div class="text-center py-12">
                    <p class="text-neutral-600 dark:text-neutral-400">
                        {"Community not found"}
                    </p>
                </div>
            },
        },
    )
}
