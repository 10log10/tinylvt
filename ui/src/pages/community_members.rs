use payloads::{CommunityId, Role, responses::CommunityWithRole};
use yew::prelude::*;

use crate::components::{
    ActiveTab, CommunityPageWrapper, CommunityTabHeader,
    user_identity_display::{render_user_avatar, render_user_name},
};
use crate::hooks::use_members;

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
                    <MembersContent community={community.clone()} />
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
    pub community: CommunityWithRole,
}

#[function_component]
fn MembersContent(props: &MembersContentProps) -> Html {
    let members_hook = use_members(props.community.id);

    if members_hook.is_loading {
        return html! {
            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400">{"Loading members..."}</p>
            </div>
        };
    }

    if let Some(error) = &members_hook.error {
        return html! {
            <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
            </div>
        };
    }

    match &members_hook.members {
        Some(members) => {
            if members.is_empty() {
                html! {
                    <div class="text-center py-12">
                        <p class="text-neutral-600 dark:text-neutral-400">
                            {"No members found in this community."}
                        </p>
                    </div>
                }
            } else {
                html! {
                    <div>
                        <div class="flex justify-between items-center mb-6">
                            <h2 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100">
                                {"Community Members"}
                            </h2>
                            // Invite Member button moved to Invites tab
                        </div>

                        <div class="space-y-3">
                            {members.iter().map(|member| {
                                html! {
                                    <div key={member.user.user_id.to_string()} class="bg-white dark:bg-neutral-800 p-4 rounded-lg border border-neutral-200 dark:border-neutral-700">
                                        <div class="flex justify-between items-center">
                                            <div class="flex items-center space-x-3">
                                                {render_user_avatar(
                                                    &member.user,
                                                    None,
                                                    None
                                                )}
                                                <div>
                                                    <p class="font-medium text-neutral-900 dark:text-neutral-100">
                                                        {render_user_name(&member.user)}
                                                    </p>
                                                </div>
                                            </div>
                                            <div>
                                                <RoleBadge role={member.role} />
                                            </div>
                                        </div>
                                    </div>
                                }
                            }).collect::<Html>()}
                        </div>
                    </div>
                }
            }
        }
        None => {
            html! {
                <div class="text-center py-12">
                    <p class="text-neutral-600 dark:text-neutral-400">{"No members data available"}</p>
                </div>
            }
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct RoleBadgeProps {
    pub role: Role,
}

#[function_component]
fn RoleBadge(props: &RoleBadgeProps) -> Html {
    let (text, classes) = match props.role {
        Role::Leader => (
            "Leader",
            "bg-neutral-900 text-white dark:bg-neutral-100 dark:text-neutral-900",
        ),
        Role::Coleader => (
            "Coleader",
            "bg-neutral-700 text-white dark:bg-neutral-300 dark:text-neutral-900",
        ),
        Role::Moderator => (
            "Moderator",
            "bg-neutral-500 text-white dark:bg-neutral-400 dark:text-neutral-900",
        ),
        Role::Member => (
            "Member",
            "bg-neutral-200 text-neutral-800 dark:bg-neutral-600 dark:text-neutral-200",
        ),
    };

    html! {
        <span class={format!("px-2 py-1 text-xs font-medium rounded-full {}", classes)}>
            {text}
        </span>
    }
}
