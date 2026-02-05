use payloads::{CommunityId, Role, responses::CommunityWithRole};
use yew::prelude::*;

use crate::components::{
    ActiveStatusToggle, ActiveTab, CommunityPageWrapper, CommunityTabHeader,
    EditCreditLimitModal,
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

    members_hook.render("members", |members, is_loading, error| {
        html! {
            <div class="relative">
                <div class="flex justify-between items-center mb-6">
                    <h2 class="text-xl font-semibold text-neutral-900 \
                               dark:text-neutral-100">
                        {"Community Members"}
                    </h2>
                    // Contextual loading indicator during refetch (inline)
                    {if is_loading {
                        html! {
                            <span class="text-xs text-neutral-500 \
                                        dark:text-neutral-400 italic">
                                {"Refreshing..."}
                            </span>
                        }
                    } else {
                        html! {}
                    }}
                </div>

                // Refetch error display
                {if let Some(err) = error {
                    html! {
                        <div class="mb-4 p-4 rounded-md bg-red-50 \
                                    dark:bg-red-900/20 border border-red-200 \
                                    dark:border-red-800">
                            <p class="text-sm text-red-700 dark:text-red-400">
                                {"Error refreshing: "}{err}
                            </p>
                        </div>
                    }
                } else {
                    html! {}
                }}

                // Member list (always visible when data exists)
                {if members.is_empty() {
                    html! {
                        <div class="text-center py-12">
                            <p class="text-neutral-600 dark:text-neutral-400">
                                {"No members found in this community."}
                            </p>
                        </div>
                    }
                } else {
                    html! {
                        <div class="space-y-3">
                            {members.iter().map(|member| {
                                let on_update = members_hook.refetch.clone();
                                html! {
                                    <MemberRow
                                        key={member.user.user_id.to_string()}
                                        member={member.clone()}
                                        community={props.community.clone()}
                                        on_update={on_update}
                                    />
                                }
                            }).collect::<Html>()}
                        </div>
                    }
                }}
            </div>
        }
    })
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

#[derive(Properties, PartialEq)]
struct MemberRowProps {
    pub member: payloads::responses::CommunityMember,
    pub community: CommunityWithRole,
    pub on_update: Callback<()>,
}

#[function_component]
fn MemberRow(props: &MemberRowProps) -> Html {
    let member = &props.member;
    let community = &props.community;

    // Modal state
    let show_edit_modal = use_state(|| false);

    // Check if credit limits are supported and user can edit them
    let can_edit_credit_limit = community.user_role.is_ge_moderator()
        && matches!(
            community.community.currency.mode_config,
            payloads::CurrencyModeConfig::DistributedClearing(_)
                | payloads::CurrencyModeConfig::DeferredPayment(_)
        );

    // Check if user can edit active status
    let can_edit_active_status = community.user_role.is_ge_moderator()
        && matches!(
            community.community.currency.mode_config,
            payloads::CurrencyModeConfig::PointsAllocation(_)
                | payloads::CurrencyModeConfig::DistributedClearing(_)
        );

    let on_edit_click = {
        let show_edit_modal = show_edit_modal.clone();
        Callback::from(move |_: web_sys::MouseEvent| {
            show_edit_modal.set(true);
        })
    };

    let on_modal_close = {
        let show_edit_modal = show_edit_modal.clone();
        Callback::from(move |_: ()| {
            show_edit_modal.set(false);
        })
    };

    let on_modal_success = {
        let show_edit_modal = show_edit_modal.clone();
        Callback::from(move |_: ()| {
            show_edit_modal.set(false);
            // Could refetch members here if needed, but balance won't change
            // when editing credit limit
        })
    };

    html! {
        <div class="bg-white dark:bg-neutral-800 p-4 rounded-lg border border-neutral-200 dark:border-neutral-700">
            <div class="flex justify-between items-center">
                <div class="flex items-center space-x-3">
                    {render_user_avatar(&member.user, None, None)}
                    <div>
                        <p class="font-medium text-neutral-900 dark:text-neutral-100">
                            {render_user_name(&member.user)}
                        </p>
                    </div>
                </div>
                <div class="flex items-center gap-4">
                    // Balance display
                    {
                        if let Some(balance) = member.balance {
                            html! {
                                <div class="text-right">
                                    <div class="text-xs text-neutral-600 dark:text-neutral-400">
                                        {"Balance"}
                                    </div>
                                    <div class="font-medium text-neutral-900 dark:text-neutral-100">
                                        {community.community.currency.format_amount(balance)}
                                    </div>
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }

                    // Active status toggle
                    {
                        if can_edit_active_status {
                            html! {
                                <ActiveStatusToggle
                                    community_id={community.id}
                                    member_user_id={member.user.user_id}
                                    current_status={member.is_active}
                                    on_success={props.on_update.clone()}
                                    disabled={false}
                                />
                            }
                        } else {
                            html! {}
                        }
                    }

                    // Edit credit limit button
                    {
                        if can_edit_credit_limit {
                            html! {
                                <button
                                    onclick={on_edit_click}
                                    class="px-3 py-1 text-sm border border-neutral-300 dark:border-neutral-600 rounded hover:bg-neutral-50 dark:hover:bg-neutral-700 text-neutral-700 dark:text-neutral-300"
                                >
                                    {"Edit Credit Limit"}
                                </button>
                            }
                        } else {
                            html! {}
                        }
                    }

                    <RoleBadge role={member.role} />
                </div>
            </div>

            // Edit credit limit modal
            {
                if *show_edit_modal {
                    html! {
                        <EditCreditLimitModal
                            member={member.user.clone()}
                            community_id={community.id}
                            currency={community.community.currency.clone()}
                            on_close={on_modal_close}
                            on_success={on_modal_success}
                        />
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}
