use payloads::{CommunityId, responses::CommunityWithRole};
use yew::prelude::*;

use crate::components::{ActiveTab, CommunityPageWrapper, CommunityTabHeader};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
}

#[function_component]
pub fn CommunityMembersPage(props: &Props) -> Html {
    let render_content = Callback::from(|community: CommunityWithRole| {
        html! {
            <div>
                <CommunityTabHeader community={community.clone()} active_tab={ActiveTab::Members} />
                <div class="py-6">
                    <MembersContent community_id={community.id} />
                </div>
            </div>
        }
    });

    html! {
        <CommunityPageWrapper
            community_id={props.community_id}
            children={render_content}
        />
    }
}

#[derive(Properties, PartialEq)]
pub struct MembersContentProps {
    pub community_id: CommunityId,
}

#[function_component]
fn MembersContent(props: &MembersContentProps) -> Html {
    // TODO: Implement members list
    html! {
        <div class="text-center py-12">
            <p class="text-neutral-600 dark:text-neutral-400 mb-4">
                {"Members will be displayed here"}
            </p>
            <p class="text-sm text-neutral-500 dark:text-neutral-500">
                {"Community ID: "}{props.community_id.to_string()}
            </p>
        </div>
    }
}
