use payloads::{
    AccountOwner, CommunityId, responses::CommunityWithRole,
    responses::UserProfile,
};
use yew::prelude::*;

use crate::components::{
    ActiveTab, CommunityPageWrapper, CommunityTabHeader, PaginationControls,
    RequireAuth, TransactionList, TransferForm,
};
use crate::hooks::{use_member_currency_info, use_member_transactions};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
}

#[function_component]
pub fn CommunityCurrencyPage(props: &Props) -> Html {
    let community_id = props.community_id;

    html! {
        <RequireAuth render={Callback::from(move |profile: UserProfile| {
            let render_content = Callback::from(move |community: CommunityWithRole| {
                html! {
                    <CommunityCurrencyContent
                        community={community}
                        community_id={community_id}
                        user_profile={profile.clone()}
                    />
                }
            });

            html! {
                <CommunityPageWrapper
                    community_id={community_id}
                    children={render_content}
                />
            }
        })} />
    }
}

#[derive(Properties, PartialEq)]
struct ContentProps {
    pub community: CommunityWithRole,
    pub community_id: CommunityId,
    pub user_profile: UserProfile,
}

#[function_component]
fn CommunityCurrencyContent(props: &ContentProps) -> Html {
    let target_user_id = props.user_profile.user_id;

    // Pagination state
    let offset = use_state(|| 0i64);
    let limit = 20i64;

    // Fetch own currency info (member_user_id = None means current user)
    let currency_info = use_member_currency_info(props.community_id, None);

    // Fetch transactions
    let transactions =
        use_member_transactions(props.community_id, None, limit, *offset);

    // Check if credit limits are supported for this currency mode
    let supports_credit_limits = matches!(
        props.community.community.currency.mode_config,
        payloads::CurrencyModeConfig::DistributedClearing(_)
            | payloads::CurrencyModeConfig::DeferredPayment(_)
    );

    html! {
        <div class="min-h-screen bg-neutral-50 dark:bg-neutral-900">
            <CommunityTabHeader
                community={props.community.clone()}
                active_tab={ActiveTab::Currency}
            />

            <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
                <div class="space-y-6">
                    // Balance Summary Section
                    <div class="bg-white dark:bg-neutral-800 rounded-lg shadow p-6">
                        <h2 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                            {"Your Balance"}
                        </h2>

                        {
                            if currency_info.is_initial_loading() {
                                html! {
                                    <div class="space-y-3">
                                        <div class="h-8 bg-neutral-200 dark:bg-neutral-700 rounded animate-pulse"></div>
                                        <div class="h-8 bg-neutral-200 dark:bg-neutral-700 rounded animate-pulse"></div>
                                        <div class="h-8 bg-neutral-200 dark:bg-neutral-700 rounded animate-pulse"></div>
                                    </div>
                                }
                            } else if let Some(error) = &currency_info.error {
                                html! {
                                    <div class="text-red-600 dark:text-red-400">
                                        {format!("Error loading balance: {}", error)}
                                    </div>
                                }
                            } else if let Some(info) = currency_info.data.as_ref() {
                                html! {
                                    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                                        // Balance
                                        <div class="p-4 bg-neutral-50 dark:bg-neutral-700 rounded">
                                            <div class="text-sm text-neutral-600 dark:text-neutral-400">
                                                {"Balance"}
                                            </div>
                                            <div class="text-2xl font-bold text-neutral-900 dark:text-neutral-100">
                                                {props.community.community.currency.format_amount(info.balance)}
                                            </div>
                                        </div>

                                        // Credit Limit (only for modes that support it)
                                        {
                                            if supports_credit_limits {
                                                html! {
                                                    <div class="p-4 bg-neutral-50 dark:bg-neutral-700 rounded">
                                                        <div class="text-sm text-neutral-600 dark:text-neutral-400">
                                                            {"Credit Limit"}
                                                        </div>
                                                        <div class="text-2xl font-bold text-neutral-900 dark:text-neutral-100">
                                                            {
                                                                if let Some(limit) = info.credit_limit {
                                                                    props.community.community.currency.format_amount(limit)
                                                                } else {
                                                                    "Unlimited".to_string()
                                                                }
                                                            }
                                                        </div>
                                                    </div>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }

                                        // Locked Balance
                                        <div class="p-4 bg-neutral-50 dark:bg-neutral-700 rounded">
                                            <div class="text-sm text-neutral-600 dark:text-neutral-400">
                                                {"Locked Balance"}
                                            </div>
                                            <div class="text-2xl font-bold text-neutral-900 dark:text-neutral-100">
                                                {props.community.community.currency.format_amount(info.locked_balance)}
                                            </div>
                                        </div>

                                        // Available Credit (only for modes that support credit)
                                        {
                                            if supports_credit_limits {
                                                html! {
                                                    <div class="p-4 bg-neutral-50 dark:bg-neutral-700 rounded">
                                                        <div class="text-sm text-neutral-600 dark:text-neutral-400">
                                                            {"Available Credit"}
                                                        </div>
                                                        <div class="text-2xl font-bold text-neutral-900 dark:text-neutral-100">
                                                            {
                                                                if let Some(available) = info.available_credit {
                                                                    props.community.community.currency.format_amount(available)
                                                                } else {
                                                                    "Unlimited".to_string()
                                                                }
                                                            }
                                                        </div>
                                                    </div>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>

                    // Transfer Form Section
                    <div class="bg-white dark:bg-neutral-800 rounded-lg shadow p-6">
                        <h2 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                            {"Transfer "}
                            {&props.community.community.currency.name}
                        </h2>
                        {
                            if let Some(info) = currency_info.data.as_ref() {
                                let refetch_currency = currency_info.refetch.clone();
                                let refetch_txns = transactions.refetch.clone();
                                let offset_handle = offset.clone();

                                html! {
                                    <TransferForm
                                        community_id={props.community_id}
                                        currency={props.community.community.currency.clone()}
                                        available_credit={info.available_credit}
                                        on_success={Callback::from(move |_| {
                                            refetch_currency.emit(());
                                            refetch_txns.emit(());
                                            offset_handle.set(0);
                                        })}
                                    />
                                }
                            } else {
                                html! {
                                    <div class="text-neutral-600 dark:text-neutral-400">
                                        {"Loading..."}
                                    </div>
                                }
                            }
                        }
                    </div>

                    // Transaction History Section
                    <div class="bg-white dark:bg-neutral-800 rounded-lg shadow p-6">
                        <h2 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                            {"Transaction History"}
                        </h2>

                        {
                            if transactions.is_initial_loading() {
                                html! {
                                    <div class="space-y-3">
                                        <div class="h-16 bg-neutral-200 dark:bg-neutral-700 rounded animate-pulse"></div>
                                        <div class="h-16 bg-neutral-200 dark:bg-neutral-700 rounded animate-pulse"></div>
                                        <div class="h-16 bg-neutral-200 dark:bg-neutral-700 rounded animate-pulse"></div>
                                    </div>
                                }
                            } else if let Some(error) = &transactions.error {
                                html! {
                                    <div class="text-red-600 dark:text-red-400">
                                        {format!("Error loading transactions: {}", error)}
                                    </div>
                                }
                            } else if let Some(txns) = transactions.data.as_ref() {
                                let offset_handle = offset.clone();
                                let on_offset_change = Callback::from(move |new_offset: i64| {
                                    offset_handle.set(new_offset);
                                });

                                html! {
                                    <>
                                        <TransactionList
                                            transactions={txns.clone()}
                                            currency={props.community.community.currency.clone()}
                                            target_account={AccountOwner::Member(target_user_id)}
                                        />
                                        <PaginationControls
                                            offset={*offset}
                                            limit={limit}
                                            current_count={txns.len()}
                                            on_offset_change={on_offset_change}
                                            is_loading={transactions.is_loading}
                                        />
                                    </>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                </div>
            </div>
        </div>
    }
}
