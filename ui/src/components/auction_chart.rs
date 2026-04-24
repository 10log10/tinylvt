use std::collections::HashMap;

use payloads::{
    CurrencySettings, RoundSpaceResult, SpaceId, UserId, responses,
};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::components::price_chart::space_color_style;
use crate::components::subway_diagram::{
    bidder_color_style, bidder_pill_needs_dark_text,
};
use crate::components::user_identity_display::render_user_name;
use crate::utils::capitalize;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    /// (space_id, name) pairs in display order. Same ordering as PriceChart so
    /// space colors line up across the two views.
    pub spaces: Vec<(SpaceId, String)>,
    /// Global bidder list in the same order used by SubwayDiagram, so bidder
    /// pill colors match the subway lanes.
    pub bidders: Vec<responses::UserIdentity>,
    /// Standing high bidders (results from previous round)
    pub results: Vec<RoundSpaceResult>,
    /// Bids placed this round: space_id -> list of bidders
    pub bids: HashMap<SpaceId, Vec<responses::UserIdentity>>,
    /// Maximum price for scaling bar widths
    pub x_max: Decimal,
    pub currency: CurrencySettings,
    pub item_term: &'static str,
}

// Desktop: 5-column grid with headings (space | bar | price | high-bid |
// new-bids). Each column is sized to stay stable across frames so the layout
// doesn't jitter when the winner or the set of new bidders changes:
// - space: auto (short pill labels)
// - bar: 1fr (absorbs remaining space)
// - price: minmax(4rem, auto) — wide enough for typical values, grows for
//   unusually large amounts
// - high-bid: minmax(5rem, 7rem) — capped so a long winner name doesn't push
//   the bar column around; overflow truncates inside the bidder pill
// - new-bids: minmax(8rem, 1fr) — pills wrap vertically when full
//
// Mobile: a stacked card per space (see mobile_card). The grid and its
// headings are hidden on mobile.

const HEADING_CLASSES: &str = "\
    text-xs font-medium text-neutral-500 \
    dark:text-neutral-500 uppercase tracking-wide";

const MOBILE_LABEL_CLASSES: &str = "\
    text-[0.65rem] font-medium text-neutral-500 \
    dark:text-neutral-500 uppercase tracking-wide";

#[function_component]
pub fn AuctionChart(props: &Props) -> Html {
    let x_max_f64 = props.x_max.to_f64().unwrap_or(1.0);

    // Bidder -> palette index, matching the order SubwayDiagram assigns.
    let bidder_idx: HashMap<UserId, usize> = props
        .bidders
        .iter()
        .enumerate()
        .map(|(i, b)| (b.user_id, i))
        .collect();

    let rows =
        props
            .spaces
            .iter()
            .enumerate()
            .map(|(space_idx, (space_id, name))| {
                let result =
                    props.results.iter().find(|r| r.space_id == *space_id);
                let space_bids = props.bids.get(space_id);
                RowData {
                    space_idx,
                    name: name.clone(),
                    result: result.cloned(),
                    new_bids: space_bids.cloned().unwrap_or_default(),
                }
            });
    // Materialize so both the desktop grid and the mobile stack can iterate.
    let rows: Vec<RowData> = rows.collect();

    html! {
        <>
            {desktop_grid_view(
                props, &rows, &bidder_idx, x_max_f64,
            )}
            {mobile_stack_view(
                props, &rows, &bidder_idx, x_max_f64,
            )}
        </>
    }
}

/// Flattened per-space row data shared between the mobile and desktop layouts.
struct RowData {
    space_idx: usize,
    name: String,
    result: Option<RoundSpaceResult>,
    new_bids: Vec<responses::UserIdentity>,
}

fn desktop_grid_view(
    props: &Props,
    rows: &[RowData],
    bidder_idx: &HashMap<UserId, usize>,
    x_max_f64: f64,
) -> Html {
    html! {
        <div class={classes!(
            "hidden", "sm:grid", "gap-x-3", "gap-y-2", "items-center",
            "sm:grid-cols-[auto_1fr_minmax(4rem,auto)_minmax(5rem,7rem)_minmax(8rem,1fr)]",
        )}>
            // Column headings
            <div class={classes!(HEADING_CLASSES, "text-right")}>
                {capitalize(props.item_term)}
            </div>
            // Bar column has no heading.
            <div />
            <div class={classes!(HEADING_CLASSES, "-ml-2", "text-right")}>
                {"Price"}
            </div>
            <div class={HEADING_CLASSES}>
                {"High bid"}
            </div>
            <div class={HEADING_CLASSES}>
                {"New bids"}
            </div>

            {for rows.iter().map(|row| desktop_row(
                row, bidder_idx, &props.currency, x_max_f64,
            ))}
        </div>
    }
}

fn desktop_row(
    row: &RowData,
    bidder_idx: &HashMap<UserId, usize>,
    currency: &CurrencySettings,
    x_max_f64: f64,
) -> Html {
    let bar = bar_html(row.result.as_ref(), x_max_f64);
    html! {
        <>
            // Space name — pill tinted with the space color so it ties to the
            // line in PriceChart and the band in SubwayDiagram. Flex +
            // justify-end pushes the block pill to the right edge so it sits
            // flush against the bar.
            <div class="text-sm flex justify-end min-w-0">
                {space_pill(row.space_idx, &row.name)}
            </div>

            // Bar
            {bar}

            // Price
            <div class="-ml-2 text-sm flex justify-end \
                text-neutral-600 dark:text-neutral-400 \
                tabular-nums">
                {price_cell(row.result.as_ref(), currency)}
            </div>

            // Standing high bidder
            <div class="text-sm min-w-0">
                {high_bid_cell(row.result.as_ref(), bidder_idx)}
            </div>

            // New bids — wraps vertically so rows grow rather than clip.
            // Uses text-sm (same as high-bid) so pill heights match and the
            // em-dash placeholder lines up across columns.
            <div class="text-sm flex flex-wrap gap-1">
                {new_bids_cell(&row.new_bids, bidder_idx)}
            </div>
        </>
    }
}

fn mobile_stack_view(
    props: &Props,
    rows: &[RowData],
    bidder_idx: &HashMap<UserId, usize>,
    x_max_f64: f64,
) -> Html {
    html! {
        <div class="sm:hidden flex flex-col gap-3">
            {for rows.iter().map(|row| mobile_card(
                row, bidder_idx, &props.currency, x_max_f64,
            ))}
        </div>
    }
}

fn mobile_card(
    row: &RowData,
    bidder_idx: &HashMap<UserId, usize>,
    currency: &CurrencySettings,
    x_max_f64: f64,
) -> Html {
    let bar = bar_html(row.result.as_ref(), x_max_f64);
    html! {
        <div class="flex flex-col gap-1.5">
            // Row 1: space pill + price on the right. Space pill is free to
            // shrink via min-w-0 on its wrapper, so long names truncate
            // inside the pill rather than pushing the price off-screen.
            <div class="flex items-center justify-between gap-2">
                <div class="min-w-0 truncate">
                    {space_pill(row.space_idx, &row.name)}
                </div>
                <div class="text-sm tabular-nums \
                    text-neutral-600 dark:text-neutral-400 \
                    shrink-0">
                    {price_cell(row.result.as_ref(), currency)}
                </div>
            </div>

            // Row 2: bar
            {bar}

            // Row 3: high bid + new bids, both labeled. Pills wrap freely.
            <div class="flex flex-col gap-1">
                <div class="flex items-center flex-wrap gap-x-2 gap-y-1">
                    <span class={MOBILE_LABEL_CLASSES}>{"High bid"}</span>
                    <div class="text-sm">
                        {high_bid_cell(row.result.as_ref(), bidder_idx)}
                    </div>
                </div>
                <div class="flex items-center flex-wrap gap-x-2 gap-y-1">
                    <span class={MOBILE_LABEL_CLASSES}>{"New bids"}</span>
                    <div class="text-sm flex flex-wrap gap-1">
                        {new_bids_cell(&row.new_bids, bidder_idx)}
                    </div>
                </div>
            </div>
        </div>
    }
}

fn bar_html(result: Option<&RoundSpaceResult>, x_max_f64: f64) -> Html {
    let pct = result
        .map(|r| {
            if x_max_f64 > 0.0 {
                r.value.to_f64().unwrap_or(0.0) / x_max_f64 * 100.0
            } else {
                0.0
            }
        })
        .unwrap_or(0.0);
    let width_style = format!("width: {:.1}%", pct);
    html! {
        <div class="h-6 bg-neutral-100 \
            dark:bg-neutral-800 rounded \
            overflow-hidden">
            <div class="h-full bg-neutral-400 \
                dark:bg-neutral-500 rounded"
                style={width_style}
            />
        </div>
    }
}

fn space_pill(space_idx: usize, name: &str) -> Html {
    let style = space_color_style(space_idx);
    html! {
        // `block w-fit` gives a content-sized block that skips inline
        // line-box metrics, so the pill's height is exactly text + padding
        // with no phantom descent space around it.
        <span
            class="block w-fit max-w-full truncate \
                px-1.5 py-0.5 rounded font-medium \
                text-neutral-900 \
                bg-[var(--space-light)] \
                dark:bg-[var(--space-dark)]"
            style={style}
        >
            {name.to_string()}
        </span>
    }
}

fn price_cell(
    result: Option<&RoundSpaceResult>,
    currency: &CurrencySettings,
) -> Html {
    match result {
        Some(r) => html! {
            // Match pill padding so the baseline aligns with pills in the
            // same row and swapping in/out of an em-dash doesn't shift the
            // row height.
            <span class="block w-fit px-1.5 py-0.5">
                {currency.format_amount(r.value)}
            </span>
        },
        None => {
            em_dash_placeholder_pill("text-neutral-600 dark:text-neutral-400")
        }
    }
}

fn high_bid_cell(
    result: Option<&RoundSpaceResult>,
    bidder_idx: &HashMap<UserId, usize>,
) -> Html {
    match result {
        Some(r) => render_bidder_pill(&r.winner, bidder_idx),
        None => {
            em_dash_placeholder_pill("text-neutral-700 dark:text-neutral-300")
        }
    }
}

fn new_bids_cell(
    bidders: &[responses::UserIdentity],
    bidder_idx: &HashMap<UserId, usize>,
) -> Html {
    if bidders.is_empty() {
        em_dash_placeholder_pill("text-neutral-500 dark:text-neutral-500")
    } else {
        bidders
            .iter()
            .map(|b| render_bidder_pill(b, bidder_idx))
            .collect::<Html>()
    }
}

/// Pill-shaped em-dash with transparent background. Preserves the vertical
/// footprint of a real pill so rows don't jitter when a cell toggles between
/// a pill and a dash across frames.
fn em_dash_placeholder_pill(text_class: &'static str) -> Html {
    html! {
        <span class={classes!(
            "block",
            "w-fit",
            "px-1.5",
            "py-0.5",
            "rounded",
            text_class,
        )}>
            {"\u{2014}"}
        </span>
    }
}

/// Render a bidder name inside a colored pill so it can be visually linked to
/// the matching lane in SubwayDiagram. Falls back to a neutral label when the
/// bidder is unknown to the global bidder list.
fn render_bidder_pill(
    bidder: &responses::UserIdentity,
    bidder_idx: &HashMap<UserId, usize>,
) -> Html {
    let Some(&idx) = bidder_idx.get(&bidder.user_id) else {
        return html! {
            <span class="block w-fit max-w-full truncate \
                text-neutral-700 dark:text-neutral-300">
                {render_user_name(bidder)}
            </span>
        };
    };
    let style = bidder_color_style(idx);
    let text_class = if bidder_pill_needs_dark_text(idx) {
        "text-neutral-900"
    } else {
        "text-white"
    };
    html! {
        <span
            class={classes!(
                "block",
                "w-fit",
                "max-w-full",
                "truncate",
                "px-1.5",
                "py-0.5",
                "rounded",
                "bg-[var(--subway-light)]",
                "dark:bg-[var(--subway-dark)]",
                text_class,
            )}
            style={style}
        >
            {render_user_name(bidder)}
        </span>
    }
}
