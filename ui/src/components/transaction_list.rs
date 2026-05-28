use payloads::{
    AccountOwner, CurrencyMode, CurrencySettings, EntryType, UserId,
    responses::{MemberTransaction, TransactionParty, UserIdentity},
};
use rust_decimal::Decimal;
use yew::prelude::*;

use crate::components::user_identity_display::render_user_name;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub transactions: Vec<MemberTransaction>,
    pub currency: CurrencySettings,
    pub target_account: AccountOwner,
}

#[function_component]
pub fn TransactionList(props: &Props) -> Html {
    if props.transactions.is_empty() {
        return html! {
            <div class="text-center py-8 text-neutral-600 dark:text-neutral-400">
                {"No transactions yet"}
            </div>
        };
    }

    html! {
        <div class="space-y-3">
            {
                props.transactions.iter().map(|txn| {
                    html! {
                        <TransactionRow
                            transaction={txn.clone()}
                            currency={props.currency.clone()}
                            target_account={props.target_account}
                        />
                    }
                }).collect::<Html>()
            }
        </div>
    }
}

enum Counterparty {
    Member(UserIdentity),
    Treasury,
    NMembers(usize),
}

fn determine_counterparty(
    txn: &MemberTransaction,
    target_account: AccountOwner,
    currency_mode: CurrencyMode,
) -> Counterparty {
    // Helper: count all member lines (excluding treasury)
    let count_members = || -> usize {
        txn.lines
            .iter()
            .filter(|line| matches!(&line.party, TransactionParty::Member(_)))
            .count()
    };

    // Helper: count the number of members for a distributed clearing auction
    // settlement. Behavior depends on whether the user has a net debit or
    // credit.
    let count_members_distributed_clearing = |user_id: UserId| -> usize {
        let user_net_amount: Decimal = txn
            .lines
            .iter()
            .filter_map(|line| match &line.party {
                TransactionParty::Member(identity)
                    if identity.user_id == user_id =>
                {
                    Some(line.amount)
                }
                _ => None,
            })
            .sum();
        if user_net_amount > Decimal::ZERO {
            // User is receiving a net credit, count the number of members
            // making payments
            txn.lines
                .iter()
                .filter(|line| {
                    matches!(&line.party, TransactionParty::Member(_))
                        && line.amount < Decimal::ZERO
                })
                .count()
        } else {
            // User is making a net payment, count the number of members
            // receiving credits
            txn.lines
                .iter()
                .filter(|line| {
                    matches!(&line.party, TransactionParty::Member(_))
                        && line.amount > Decimal::ZERO
                })
                .count()
        }
    };

    // Helper: get first member identity
    let find_member = || -> Option<UserIdentity> {
        txn.lines.iter().find_map(|line| match &line.party {
            TransactionParty::Member(identity) => Some(identity.clone()),
            TransactionParty::Treasury => None,
        })
    };

    // Helper: get first member excluding a specific user
    let find_member_except = |exclude_user_id: UserId| -> Option<UserIdentity> {
        txn.lines.iter().find_map(|line| match &line.party {
            TransactionParty::Member(identity)
                if identity.user_id != exclude_user_id =>
            {
                Some(identity.clone())
            }
            _ => None,
        })
    };

    // Helper: does this entry touch the treasury account at all?
    let has_treasury_line = || -> bool {
        txn.lines
            .iter()
            .any(|line| matches!(&line.party, TransactionParty::Treasury))
    };

    match target_account {
        // Transactions from the User's perspective
        AccountOwner::Member(user_id) => match txn.entry_type {
            // Coleader-initiated treasury operations
            EntryType::TreasuryTransfer | EntryType::BalanceReset => {
                Counterparty::Treasury
            }
            EntryType::AuctionSettlement
            | EntryType::OrphanedAccountTransfer => {
                match currency_mode {
                    CurrencyMode::DistributedClearing => {
                        // Though settlement can go to treasury if there are no
                        // active members, it *should* go towards the members,
                        // so it's clearer to actually render this as "0
                        // members", which indicates the anomaly.
                        Counterparty::NMembers(
                            count_members_distributed_clearing(user_id),
                        )
                    }
                    CurrencyMode::PointsAllocation
                    | CurrencyMode::DeferredPayment
                    | CurrencyMode::PrepaidCredits => Counterparty::Treasury,
                }
            }
            // Transfer can be member->member or member->treasury
            EntryType::Transfer => {
                if has_treasury_line() {
                    Counterparty::Treasury
                } else {
                    find_member_except(user_id)
                        .map(Counterparty::Member)
                        .unwrap_or(Counterparty::NMembers(0))
                }
            }
        },
        // Transactions from the Treasury's perspective
        AccountOwner::Treasury => match txn.entry_type {
            // Treasury transfer: single-member or bulk depending on lines
            EntryType::TreasuryTransfer => {
                let member_count = count_members();
                if member_count == 1 {
                    find_member()
                        .map(Counterparty::Member)
                        .unwrap_or(Counterparty::NMembers(0))
                } else {
                    Counterparty::NMembers(member_count)
                }
            }
            // Balance reset always touches all members
            EntryType::BalanceReset => Counterparty::NMembers(count_members()),
            EntryType::AuctionSettlement => {
                Counterparty::NMembers(count_members())
            }
            // Member-initiated transfer to treasury
            EntryType::Transfer => find_member()
                .map(Counterparty::Member)
                .unwrap_or(Counterparty::NMembers(0)),
            // Orphaned account transfer to treasury
            EntryType::OrphanedAccountTransfer => find_member()
                .map(Counterparty::Member)
                .unwrap_or(Counterparty::NMembers(0)),
        },
    }
}

#[derive(Properties, PartialEq)]
struct TransactionRowProps {
    pub transaction: MemberTransaction,
    pub currency: CurrencySettings,
    pub target_account: AccountOwner,
}

#[function_component]
fn TransactionRow(props: &TransactionRowProps) -> Html {
    let txn = &props.transaction;

    // Format entry type for display
    let entry_type_label = match txn.entry_type {
        payloads::EntryType::Transfer => "Transfer",
        payloads::EntryType::TreasuryTransfer => "Treasury Transfer",
        payloads::EntryType::AuctionSettlement => "Auction Settlement",
        payloads::EntryType::BalanceReset => "Balance Reset",
        payloads::EntryType::OrphanedAccountTransfer => {
            "Orphaned Account Transfer"
        }
    };

    // Determine counterparty
    let counterparty = determine_counterparty(
        txn,
        props.target_account,
        props.currency.mode(),
    );

    // Render counterparty as Html
    let counterparty_display = match counterparty {
        Counterparty::Member(identity) => render_user_name(&identity),
        Counterparty::Treasury => html! { "Treasury" },
        Counterparty::NMembers(count) => {
            html! { {format!("{} Members", count)} }
        }
    };

    // Net amount for the target account only (not all lines which sum
    // to zero)
    let net_amount: rust_decimal::Decimal = txn
        .lines
        .iter()
        .filter(|line| match &line.party {
            TransactionParty::Member(identity) => match props.target_account {
                AccountOwner::Member(user_id) => identity.user_id == user_id,
                AccountOwner::Treasury => false,
            },
            TransactionParty::Treasury => {
                matches!(props.target_account, AccountOwner::Treasury)
            }
        })
        .map(|line| line.amount)
        .sum();

    // Determine if this is a debit or credit
    let is_credit = net_amount > rust_decimal::Decimal::ZERO;

    // Format timestamp
    let timestamp_display = {
        use jiff::tz::TimeZone;
        let system_tz = TimeZone::system();
        let zoned = txn.created_at.to_zoned(system_tz);
        zoned.strftime("%b %d, %Y %I:%M %p").to_string()
    };

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 rounded-lg p-4 bg-white dark:bg-neutral-800">
            <div class="flex items-start justify-between">
                <div class="flex-1">
                    <div class="flex items-center gap-2">
                        <span class="font-medium text-neutral-900 dark:text-neutral-100">
                            {entry_type_label}
                        </span>
                        <span class="text-sm text-neutral-600 dark:text-neutral-400">
                            {if is_credit { "from" } else { "to" }}
                            {" "}
                            {counterparty_display}
                        </span>
                    </div>

                    <div class="text-sm text-neutral-600 dark:text-neutral-400 mt-1">
                        {&timestamp_display}
                    </div>

                    {
                        if let Some(note) = &txn.note {
                            html! {
                                <div class="text-sm text-neutral-700 dark:text-neutral-300 mt-2 italic">
                                    {"\""}{note}{"\""}
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>

                <div class={classes!(
                    "text-lg", "font-semibold",
                    if is_credit {
                        classes!("text-green-600", "dark:text-green-400")
                    } else {
                        classes!("text-red-600", "dark:text-red-400")
                    }
                )}>
                    {if is_credit { "+" } else { "" }}
                    {props.currency.format_amount(net_amount.abs())}
                </div>
            </div>

            // Show transaction lines (for debugging/transparency)
            {
                if txn.lines.len() > 2 {
                    html! {
                        <details class="mt-3">
                            <summary class="text-xs text-neutral-500 dark:text-neutral-400 cursor-pointer">
                                {"View details"}
                            </summary>
                            <div class="mt-2 space-y-1 text-xs">
                                {
                                    txn.lines.iter().map(|line| {
                                        let party_display = match &line.party {
                                            TransactionParty::Member(identity) => {
                                                render_user_name(identity)
                                            }
                                            TransactionParty::Treasury => html! { "Treasury" },
                                        };

                                        html! {
                                            <div class="flex justify-between text-neutral-600 dark:text-neutral-400">
                                                <span>{party_display}</span>
                                                <span>
                                                    {if line.amount > rust_decimal::Decimal::ZERO { "+" } else { "" }}
                                                    {props.currency.format_amount(line.amount.abs())}
                                                </span>
                                            </div>
                                        }
                                    }).collect::<Html>()
                                }
                            </div>
                        </details>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}
