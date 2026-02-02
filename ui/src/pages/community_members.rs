use payloads::{CommunityId, Role, responses::CommunityWithRole};
use yew::prelude::*;

use crate::components::{
    ActiveTab, CommunityPageWrapper, CommunityTabHeader, EditCreditLimitModal,
    user_identity_display::{render_user_avatar, render_user_name},
};
use crate::hooks::{use_member_currency_info, use_members};

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

    match members_hook.data.as_ref() {
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
                                    <MemberRow
                                        key={member.user.user_id.to_string()}
                                        member={member.clone()}
                                        community={props.community.clone()}
                                    />
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

#[derive(Properties, PartialEq)]
struct MemberRowProps {
    pub member: payloads::responses::CommunityMember,
    pub community: CommunityWithRole,
}

#[function_component]
fn MemberRow(props: &MemberRowProps) -> Html {
    let member = &props.member;
    let community = &props.community;

    // Modal state
    let show_edit_modal = use_state(|| false);

    // Check if we should show balance
    // Show if balances_visible_to_members OR user is coleader+
    let show_balance = community.community.currency.balances_visible_to_members
        || community.user_role.is_ge_coleader();

    // Fetch member currency info if we should show balance
    let currency_info = use_member_currency_info(
        community.id,
        if show_balance {
            Some(member.user.user_id)
        } else {
            None
        },
    );

    // Check if credit limits are supported and user can edit them
    let can_edit_credit_limit = community.user_role.is_ge_moderator()
        && matches!(
            community.community.currency.mode_config,
            payloads::CurrencyModeConfig::DistributedClearing(_)
                | payloads::CurrencyModeConfig::DeferredPayment(_)
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
        let refetch = currency_info.refetch.clone();
        Callback::from(move |_: ()| {
            show_edit_modal.set(false);
            refetch.emit(());
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
                        if show_balance {
                            html! {
                                <div class="text-right">
                                    <div class="text-xs text-neutral-600 dark:text-neutral-400">
                                        {"Balance"}
                                    </div>
                                    <div class="font-medium text-neutral-900 dark:text-neutral-100">
                                        {
                                            if currency_info.is_loading {
                                                html! { <span class="text-neutral-400">{"..."}</span> }
                                            } else if let Some(info) = currency_info.data.as_ref() {
                                                html! {
                                                    <span>
                                                        {community.community.currency.format_amount(info.balance)}
                                                    </span>
                                                }
                                            } else {
                                                html! { <span class="text-neutral-400">{"-"}</span> }
                                            }
                                        }
                                    </div>
                                </div>
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
                    if let Some(info) = currency_info.data.as_ref() {
                        html! {
                            <EditCreditLimitModal
                                member={member.user.clone()}
                                community_id={community.id}
                                current_credit_limit={info.credit_limit}
                                currency={community.community.currency.clone()}
                                on_close={on_modal_close}
                                on_success={on_modal_success}
                            />
                        }
                    } else {
                        html! {}
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}
