use payloads::{CommunityId, Role, responses::CommunityWithRole};
use yew::prelude::*;

use crate::components::{
    ActiveStatusToggle, ActiveTab, ChangeRoleModal, CommunityPageWrapper,
    CommunityTabHeader, EditCreditLimitModal, MenuItem, OverflowMenu,
    RemoveMemberModal,
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
                <CommunityTabHeader
                    community={community.clone()}
                    active_tab={ActiveTab::Members}
                />
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
            "bg-neutral-200 text-neutral-800 dark:bg-neutral-600 \
             dark:text-neutral-200",
        ),
    };

    html! {
        <span class={format!(
            "px-2 py-1 text-xs font-medium rounded-full {}",
            classes
        )}>
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

    // Modal states
    let show_edit_modal = use_state(|| false);
    let show_remove_modal = use_state(|| false);
    let show_change_role_modal = use_state(|| false);

    // Check if credit limits are supported and user can edit them
    let can_edit_credit_limit = community.user_role.can_edit_credit_limit()
        && matches!(
            community.community.currency.mode_config,
            payloads::CurrencyModeConfig::DistributedClearing(_)
                | payloads::CurrencyModeConfig::DeferredPayment(_)
        );

    // Check if user can edit active status
    let can_edit_active_status = community.user_role.can_change_active_status()
        && matches!(
            community.community.currency.mode_config,
            payloads::CurrencyModeConfig::PointsAllocation(_)
                | payloads::CurrencyModeConfig::DistributedClearing(_)
        );

    // Check if user can remove this member
    let can_remove = community.user_role.can_remove_role(&member.role);

    // Check if user can change this member's role (any valid change exists)
    let can_change_role = [Role::Member, Role::Moderator, Role::Coleader]
        .iter()
        .any(|new_role| {
            community.user_role.can_change_role(&member.role, new_role)
        });

    // Build overflow menu items
    let menu_items = {
        let mut items = Vec::new();

        if can_edit_credit_limit {
            let show_edit_modal = show_edit_modal.clone();
            items.push(MenuItem {
                label: "Edit Credit Limit".into(),
                on_click: Callback::from(move |_| show_edit_modal.set(true)),
                danger: false,
            });
        }

        if can_change_role {
            let show_change_role_modal = show_change_role_modal.clone();
            items.push(MenuItem {
                label: "Change Role".into(),
                on_click: Callback::from(move |_| {
                    show_change_role_modal.set(true)
                }),
                danger: false,
            });
        }

        if can_remove {
            let show_remove_modal = show_remove_modal.clone();
            items.push(MenuItem {
                label: "Remove Member".into(),
                on_click: Callback::from(move |_| show_remove_modal.set(true)),
                danger: true,
            });
        }

        items
    };

    let on_edit_modal_close = {
        let show_edit_modal = show_edit_modal.clone();
        Callback::from(move |_: ()| {
            show_edit_modal.set(false);
        })
    };

    let on_edit_modal_success = {
        let show_edit_modal = show_edit_modal.clone();
        Callback::from(move |_: ()| {
            show_edit_modal.set(false);
        })
    };

    let on_remove_modal_close = {
        let show_remove_modal = show_remove_modal.clone();
        Callback::from(move |_: ()| {
            show_remove_modal.set(false);
        })
    };

    let on_remove_success = {
        let show_remove_modal = show_remove_modal.clone();
        let on_update = props.on_update.clone();
        Callback::from(move |_: ()| {
            show_remove_modal.set(false);
            on_update.emit(());
        })
    };

    let on_change_role_modal_close = {
        let show_change_role_modal = show_change_role_modal.clone();
        Callback::from(move |_: ()| {
            show_change_role_modal.set(false);
        })
    };

    let on_change_role_success = {
        let show_change_role_modal = show_change_role_modal.clone();
        let on_update = props.on_update.clone();
        Callback::from(move |_: ()| {
            show_change_role_modal.set(false);
            on_update.emit(());
        })
    };

    html! {
        <div class="bg-white dark:bg-neutral-800 p-4 rounded-lg border \
                    border-neutral-200 dark:border-neutral-700">
            <div class="flex flex-wrap justify-between items-center gap-3">
                <div class="flex items-center space-x-3">
                    {render_user_avatar(&member.user, None, None)}
                    <div>
                        <p class="font-medium text-neutral-900 \
                                  dark:text-neutral-100">
                            {render_user_name(&member.user)}
                        </p>
                    </div>
                </div>
                <div class="flex items-center gap-3 flex-wrap">
                    // Balance display
                    {if let Some(balance) = member.balance {
                        html! {
                            <div class="text-right">
                                <div class="text-xs text-neutral-600 \
                                            dark:text-neutral-400">
                                    {"Balance"}
                                </div>
                                <div class="font-medium text-neutral-900 \
                                            dark:text-neutral-100">
                                    {community.community.currency.format_amount(balance)}
                                </div>
                            </div>
                        }
                    } else {
                        html! {}
                    }}

                    // Active status toggle
                    {if can_edit_active_status {
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
                    }}

                    <RoleBadge role={member.role} />

                    // Overflow menu for admin actions
                    <OverflowMenu items={menu_items} />
                </div>
            </div>

            // Edit credit limit modal
            {if *show_edit_modal {
                html! {
                    <EditCreditLimitModal
                        member={member.user.clone()}
                        community_id={community.id}
                        currency={community.community.currency.clone()}
                        on_close={on_edit_modal_close}
                        on_success={on_edit_modal_success}
                    />
                }
            } else {
                html! {}
            }}

            // Remove member modal
            {if *show_remove_modal {
                html! {
                    <RemoveMemberModal
                        community_id={community.id}
                        member={member.clone()}
                        on_success={on_remove_success}
                        on_close={on_remove_modal_close}
                    />
                }
            } else {
                html! {}
            }}

            // Change role modal
            {if *show_change_role_modal {
                html! {
                    <ChangeRoleModal
                        community_id={community.id}
                        member={member.clone()}
                        actor_role={community.user_role}
                        on_success={on_change_role_success}
                        on_close={on_change_role_modal_close}
                    />
                }
            } else {
                html! {}
            }}
        </div>
    }
}
