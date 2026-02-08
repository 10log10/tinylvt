use payloads::{
    CommunityId, IdempotencyKey, requests, responses::CommunityWithRole,
};
use uuid::Uuid;
use yew::prelude::*;

use crate::components::{
    ActiveTab, CommunityPageWrapper, CommunityTabHeader,
    user_identity_display::render_user_name,
};
use crate::get_api_client;
use crate::hooks::use_orphaned_accounts;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
}

#[function_component]
pub fn OrphanedAccountsPage(props: &Props) -> Html {
    let render_content = Callback::from(|community: CommunityWithRole| {
        html! {
            <div>
                <CommunityTabHeader
                    community={community.clone()}
                    active_tab={ActiveTab::OrphanedAccounts}
                />
                <div class="py-6">
                    <OrphanedAccountsContent community={community.clone()} />
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
pub struct OrphanedAccountsContentProps {
    pub community: CommunityWithRole,
}

#[function_component]
fn OrphanedAccountsContent(props: &OrphanedAccountsContentProps) -> Html {
    let orphaned_hook = use_orphaned_accounts(props.community.id);

    let is_distributed_clearing = matches!(
        props.community.community.currency.mode_config,
        payloads::CurrencyModeConfig::DistributedClearing(_)
    );

    let resolution_target = if is_distributed_clearing {
        "distributed to active members"
    } else {
        "transferred to the treasury"
    };

    orphaned_hook.render("orphaned accounts", |list, is_loading, error| {
        html! {
            <div class="relative">
                <div class="flex justify-between items-center mb-6">
                    <div>
                        <h2 class="text-xl font-semibold text-neutral-900 \
                                   dark:text-neutral-100">
                            {"Orphaned Accounts"}
                        </h2>
                        <p class="mt-2 text-sm text-neutral-600 \
                                 dark:text-neutral-400">
                            {"Accounts of members who have left the community. \
                              If a member rejoins, they will be reconnected with \
                              their account and balance. Otherwise, you can \
                              resolve their balance to have funds "}
                            {resolution_target}
                            {"."}
                        </p>
                    </div>
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

                {if list.orphaned_accounts.is_empty() {
                    html! {
                        <div class="text-center py-12">
                            <p class="text-neutral-600 dark:text-neutral-400">
                                {"No orphaned accounts found."}
                            </p>
                        </div>
                    }
                } else {
                    html! {
                        <div class="space-y-3">
                            {list.orphaned_accounts.iter().map(|orphaned| {
                                let on_resolve = orphaned_hook.refetch.clone();
                                html! {
                                    <OrphanedAccountRow
                                        key={orphaned.account.id.to_string()}
                                        orphaned_account={orphaned.clone()}
                                        community={props.community.clone()}
                                        on_resolve={on_resolve}
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
struct OrphanedAccountRowProps {
    pub orphaned_account: payloads::responses::OrphanedAccount,
    pub community: CommunityWithRole,
    pub on_resolve: Callback<()>,
}

#[function_component]
fn OrphanedAccountRow(props: &OrphanedAccountRowProps) -> Html {
    let is_submitting = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    let orphaned = &props.orphaned_account;
    let balance = orphaned.account.balance_cached;
    let is_zero_balance = balance.is_zero();

    let on_resolve = {
        let community_id = props.community.id;
        let account_id = orphaned.account.id;
        let is_submitting = is_submitting.clone();
        let error_message = error_message.clone();
        let on_resolve = props.on_resolve.clone();

        Callback::from(move |_: web_sys::MouseEvent| {
            let is_submitting = is_submitting.clone();
            let error_message = error_message.clone();
            let on_resolve = on_resolve.clone();

            yew::platform::spawn_local(async move {
                is_submitting.set(true);
                error_message.set(None);

                let request = requests::ResolveOrphanedBalance {
                    community_id,
                    orphaned_account_id: account_id,
                    note: None,
                    idempotency_key: IdempotencyKey(Uuid::new_v4()),
                };

                match get_api_client().resolve_orphaned_balance(&request).await
                {
                    Ok(_) => {
                        on_resolve.emit(());
                    }
                    Err(e) => {
                        error_message.set(Some(format!(
                            "Failed to resolve balance: {}",
                            e
                        )));
                    }
                }

                is_submitting.set(false);
            });
        })
    };

    html! {
        <div class="bg-white dark:bg-neutral-800 p-4 rounded-lg border \
                    border-neutral-200 dark:border-neutral-700">
            <div class="flex justify-between items-center">
                <div class="flex-1">
                    <p class="font-medium text-neutral-900 \
                             dark:text-neutral-100">
                        {
                            if let Some(user) = &orphaned.previous_owner {
                                render_user_name(user)
                            } else {
                                html! {
                                    <span class="italic text-neutral-500 \
                                                dark:text-neutral-400">
                                        {"[Deleted User]"}
                                    </span>
                                }
                            }
                        }
                    </p>
                    <p class="text-sm text-neutral-600 dark:text-neutral-400 \
                             mt-1">
                        {"Balance: "}
                        <span class="font-medium">
                            {props.community.community.currency.format_amount(balance)}
                        </span>
                    </p>
                </div>

                <button
                    onclick={on_resolve}
                    disabled={*is_submitting || is_zero_balance}
                    class="px-3 py-1 text-sm bg-neutral-200 \
                           hover:bg-neutral-300 dark:bg-neutral-700 \
                           dark:hover:bg-neutral-600 text-neutral-900 \
                           dark:text-neutral-100 rounded \
                           disabled:opacity-50 disabled:cursor-not-allowed"
                >
                    {"Resolve"}
                </button>
            </div>

            {if let Some(error) = (*error_message).clone() {
                html! {
                    <div class="mt-3 p-3 bg-red-50 dark:bg-red-900/20 \
                                border border-red-200 dark:border-red-800 \
                                rounded">
                        <p class="text-sm text-red-700 dark:text-red-400">
                            {error}
                        </p>
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
