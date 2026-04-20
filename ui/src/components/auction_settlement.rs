//! Note that for real auctions, the equal share payout is not necessarily based
//! upon auction participation. Instead, pass in the transaction that references
//! this auction to display actual payments.

use payloads::responses::UserIdentity;
use payloads::{CurrencySettings, RoundSpaceResult, SpaceId, UserId};
use rust_decimal::Decimal;
use yew::prelude::*;

use crate::components::user_identity_display::render_user_name;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub spaces: Vec<(SpaceId, String)>,
    pub bidders: Vec<UserIdentity>,
    /// Final round results (winners and prices)
    pub results: Vec<RoundSpaceResult>,
    pub currency: CurrencySettings,
}

const MUTED: &str = "text-neutral-500 dark:text-neutral-500";
const LABEL: &str = "text-neutral-700 dark:text-neutral-300";
const BOLD: &str = "font-medium text-neutral-900 dark:text-neutral-100";

struct WinnerEntry {
    user: UserIdentity,
    space_names: Vec<String>,
    total_price: Decimal,
}

#[function_component]
pub fn AuctionSettlement(props: &Props) -> Html {
    let total_proceeds: Decimal = props.results.iter().map(|r| r.value).sum();
    let n = props.bidders.len();
    if n == 0 {
        return html! {};
    }
    let share = total_proceeds / Decimal::from(n);

    // Group wins by bidder, preserving space order
    let mut winner_entries: Vec<WinnerEntry> = Vec::new();
    let mut winner_ids: Vec<UserId> = Vec::new();

    for (sid, name) in &props.spaces {
        if let Some(r) = props.results.iter().find(|r| r.space_id == *sid) {
            if let Some(entry) = winner_entries
                .iter_mut()
                .find(|e| e.user.user_id == r.winner.user_id)
            {
                entry.space_names.push(name.clone());
                entry.total_price += r.value;
            } else {
                winner_ids.push(r.winner.user_id);
                winner_entries.push(WinnerEntry {
                    user: r.winner.clone(),
                    space_names: vec![name.clone()],
                    total_price: r.value,
                });
            }
        }
    }

    // Non-winners: bidders who didn't win any space
    let non_winners: Vec<_> = props
        .bidders
        .iter()
        .filter(|b| !winner_ids.contains(&b.user_id))
        .collect();

    let fmt = |amount: Decimal| props.currency.format_amount(amount);

    let format_adjustment = |adj: Decimal| -> String {
        if adj >= Decimal::ZERO {
            format!("+{}", fmt(adj))
        } else {
            fmt(adj)
        }
    };

    html! {
        <div>
            // Totals
            <div class={classes!(
                "flex", "flex-wrap", "gap-x-6", "gap-y-1",
                "text-sm", "mb-3", MUTED
            )}>
                <span>
                    {"Total proceeds: "}
                    <span class={BOLD}>
                        {fmt(total_proceeds)}
                    </span>
                </span>
                <span>
                    {"Equal share (1/"}
                    {n.to_string()}
                    {"): "}
                    <span class={BOLD}>
                        {fmt(share)}
                    </span>
                </span>
            </div>

            // Winner adjustments
            //
            // Mobile (<sm): each row stacks — first the
            // "name → space" line, then a math grid below.
            // Desktop (sm+): a single grid holds all rows so
            // names, spaces, and numeric columns align.
            <div class={classes!(
                "space-y-2", "sm:space-y-0",
                "sm:grid",
                "sm:grid-cols-[auto_auto_1fr_auto_auto_auto_auto_auto]",
                "sm:gap-x-2", "sm:gap-y-1",
                "sm:items-center", "text-sm"
            )}>
                {for winner_entries.iter().map(|entry| {
                    let adjustment = share - entry.total_price;
                    let spaces_str = entry.space_names.join(", ");
                    html! {
                        <SettlementRow
                            name={render_user_name(&entry.user)}
                            space={html!{spaces_str}}
                            space_muted={false}
                            share={fmt(share)}
                            subtrahend={fmt(entry.total_price)}
                            adjustment={format_adjustment(adjustment)}
                        />
                    }
                })}

                // Non-winners receive their share
                {for non_winners.iter().map(|&bidder| {
                    html! {
                        <SettlementRow
                            name={render_user_name(bidder)}
                            space={html!{
                                <span class="italic">{"none"}</span>
                            }}
                            space_muted={true}
                            share={fmt(share)}
                            subtrahend={fmt(Decimal::ZERO)}
                            adjustment={format_adjustment(share)}
                        />
                    }
                })}
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct SettlementRowProps {
    name: Html,
    space: Html,
    space_muted: bool,
    share: String,
    subtrahend: String,
    adjustment: String,
}

/// One row of the settlement table.
///
/// On mobile the wrapper is a flex column: the "name → space"
/// line sits above a 5-column math grid whose columns align
/// across rows. On `sm+`, the wrapper becomes `display: contents`
/// so its eight children flow directly into the parent grid,
/// aligning names, spaces, and numeric columns across rows.
#[function_component]
fn SettlementRow(props: &SettlementRowProps) -> Html {
    let space_class = if props.space_muted { MUTED } else { LABEL };

    html! {
        <div class="flex flex-col gap-1 sm:contents">
            // Mobile-only: "name → space" line
            <div class="flex items-center gap-2 min-w-0 sm:hidden">
                <span class={classes!("truncate", LABEL)}>
                    {props.name.clone()}
                </span>
                <span class={MUTED}>{"\u{2192}"}</span>
                <span class={classes!("truncate", space_class)}>
                    {props.space.clone()}
                </span>
            </div>

            // Mobile-only: math grid, columns align across rows
            <div class={classes!(
                "grid",
                "grid-cols-[1fr_auto_1fr_auto_1fr]",
                "gap-x-1", "items-center", "sm:hidden"
            )}>
                <span class={classes!(
                    "tabular-nums", "text-right", MUTED
                )}>
                    {props.share.clone()}
                </span>
                <span class={MUTED}>{"\u{2212}"}</span>
                <span class={classes!(
                    "tabular-nums", "text-right", MUTED
                )}>
                    {props.subtrahend.clone()}
                </span>
                <span class={MUTED}>{"="}</span>
                <span class={classes!(
                    "tabular-nums", "text-right", BOLD
                )}>
                    {props.adjustment.clone()}
                </span>
            </div>

            // Desktop-only (sm+): eight cells flow into parent
            // grid via `sm:contents` on the wrapper. Each child
            // below is hidden on mobile.
            <span class={classes!(
                "hidden", "sm:inline", "truncate", LABEL
            )}>
                {props.name.clone()}
            </span>
            <span class={classes!("hidden", "sm:inline", MUTED)}>
                {"\u{2192}"}
            </span>
            <span class={classes!(
                "hidden", "sm:inline", "truncate", space_class
            )}>
                {props.space.clone()}
            </span>
            <span class={classes!(
                "hidden", "sm:inline", "tabular-nums",
                "text-right", "whitespace-nowrap", MUTED
            )}>
                {props.share.clone()}
            </span>
            <span class={classes!("hidden", "sm:inline", MUTED)}>
                {"\u{2212}"}
            </span>
            <span class={classes!(
                "hidden", "sm:inline", "tabular-nums",
                "text-right", "whitespace-nowrap", MUTED
            )}>
                {props.subtrahend.clone()}
            </span>
            <span class={classes!("hidden", "sm:inline", MUTED)}>
                {"="}
            </span>
            <span class={classes!(
                "hidden", "sm:inline", "tabular-nums",
                "text-right", "whitespace-nowrap", BOLD
            )}>
                {props.adjustment.clone()}
            </span>
        </div>
    }
}
