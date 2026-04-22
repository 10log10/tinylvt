use std::collections::HashMap;

use payloads::auction_sim::SimRound;
use payloads::{CurrencySettings, SpaceId};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use yew::prelude::*;

use crate::components::timeline_layout::{
    BOTTOM_PAD, LEFT_PAD, RIGHT_PAD, TOP_PAD, label_stride_for, round_x,
};

#[derive(Properties, PartialEq)]
pub struct Props {
    /// (space_id, name) pairs. Determines which spaces to plot and the legend
    /// order. Does not affect vertical ordering in the chart (all spaces share
    /// the same Y axis).
    pub spaces: Vec<(SpaceId, String)>,
    /// All simulation rounds.
    pub rounds: Vec<SimRound>,
    /// Frame-based display cutoff. Only rounds `0..=frame` contribute.
    pub frame: usize,
    /// ViewBox units per round. Must match the value used by the subway
    /// diagram so the two charts' time axes align.
    pub col_w: f64,
    /// Currency used to format Y-axis tick labels.
    pub currency: CurrencySettings,
}

// Chart-local layout constants.
const CHART_HEIGHT: f64 = 180.0; // viewBox units
const Y_AXIS_TICK_COUNT: usize = 4;
const LINE_STROKE_WIDTH: f64 = 2.0;
const DOT_RADIUS: f64 = 3.5;
const DOT_STROKE: f64 = 1.5;

// Per-space colors use Paul Tol's "Light" qualitative palette. Distinct from
// the subway "Bright" palette so bidder and space lines don't share semantics.
// https://sronpersonalpages.nl/~pault/#sec:qualitative
const SPACE_PALETTE: &[&str] = &[
    "#77AADD", // light blue
    "#EE8866", // orange
    "#EEDD88", // sand
    "#FFAABB", // pink
    "#99DDFF", // light cyan
    "#44BB99", // mint
    "#BBCC33", // pear
    "#AAAA00", // olive
    "#DDDDDD", // pale grey
];

#[function_component]
pub fn PriceChart(props: &Props) -> Html {
    let col_w = props.col_w;
    let num_rounds = props.rounds.len();

    let svg_height = TOP_PAD + CHART_HEIGHT + BOTTOM_PAD;
    let svg_width = LEFT_PAD + (num_rounds as f64).max(1.0) * col_w + RIGHT_PAD;
    let viewbox = format!("0 0 {} {}", svg_width, svg_height);

    // Per-space step-polyline points: (round_idx, price) pairs with duplicate-x
    // entries inserted at step corners. Each series starts the first round a
    // space trades. Between rounds, a horizontal carry is followed by a
    // vertical step at the next round's tick, producing a staircase when the
    // price changes and a flat line when it doesn't.
    let last_frame = props.frame.min(num_rounds.saturating_sub(1));
    let series: HashMap<SpaceId, Vec<(usize, f64)>> =
        compute_series(&props.rounds, last_frame, &props.spaces);

    // Global Y max across the entire auction (not just rounds up to the current
    // frame). This gives the plot foresight of how prices play out so the
    // y-axis doesn't rescale as frames advance during playback. Callers who
    // want a hidden-future view can trim `rounds` themselves.
    let y_max = props
        .rounds
        .iter()
        .flat_map(|r| r.results.iter())
        .filter_map(|rsr| rsr.value.to_f64())
        .fold(0.0_f64, f64::max);
    let (y_axis_max, y_ticks) = nice_y_ticks(y_max, Y_AXIS_TICK_COUNT);

    // Convert price -> viewBox y. The plot area spans
    // [TOP_PAD .. TOP_PAD + CHART_HEIGHT].
    let price_to_y = move |price: f64| -> f64 {
        let t = if y_axis_max > 0.0 {
            price / y_axis_max
        } else {
            0.0
        };
        TOP_PAD + CHART_HEIGHT * (1.0 - t)
    };

    // Y-axis gridlines and tick labels.
    let y_axis_html = y_ticks
        .iter()
        .map(|&tick| {
            let y = price_to_y(tick);
            let label = props.currency.format_amount(
                Decimal::try_from(tick).unwrap_or(Decimal::ZERO),
            );
            html! {
                <>
                    <line
                        x1={LEFT_PAD.to_string()}
                        y1={y.to_string()}
                        x2={svg_width.to_string()}
                        y2={y.to_string()}
                        class="stroke-neutral-200 dark:stroke-neutral-800"
                        stroke-width="1"
                    />
                    <text
                        x={(LEFT_PAD - 8.0).to_string()}
                        y={y.to_string()}
                        text-anchor="end"
                        dominant-baseline="central"
                        class="fill-neutral-600 dark:fill-neutral-400 \
                            text-xs tabular-nums"
                    >
                        {label}
                    </text>
                </>
            }
        })
        .collect::<Html>();

    // X-axis gridlines + round labels, same rules as the subway.
    let max_round_num =
        props.rounds.iter().map(|r| r.round_num).max().unwrap_or(0);
    let label_stride = label_stride_for(max_round_num, col_w);
    let x_axis_html = (0..num_rounds)
        .map(|r| {
            let x = round_x(r, col_w);
            let round_num = props.rounds[r].round_num;
            let grid_class = if r <= props.frame {
                "stroke-neutral-200 dark:stroke-neutral-800"
            } else {
                "stroke-neutral-100 dark:stroke-neutral-900"
            };
            let label_class = if r <= props.frame {
                "fill-neutral-600 dark:fill-neutral-400 text-xs \
                tabular-nums"
            } else {
                "fill-neutral-400 dark:fill-neutral-600 text-xs \
                tabular-nums"
            };
            let show_label = round_num % label_stride == 0;
            let label = if show_label {
                html! {
                    <text
                        x={x.to_string()}
                        y={(TOP_PAD - 8.0).to_string()}
                        text-anchor="middle"
                        class={label_class}
                    >
                        {round_num.to_string()}
                    </text>
                }
            } else {
                Html::default()
            };
            html! {
                <>
                    <line
                        x1={x.to_string()}
                        y1={TOP_PAD.to_string()}
                        x2={x.to_string()}
                        y2={(svg_height - BOTTOM_PAD).to_string()}
                        class={grid_class}
                        stroke-width="1"
                    />
                    {label}
                </>
            }
        })
        .collect::<Html>();

    // Render order: less-desirable spaces first (bottom), most-desirable last
    // (on top). Within each space, emit the line first, then its dots, so a
    // space's own dots cover its own line without being hidden by another
    // space's line segment. A foreign line *can* cross over a less-desirable
    // space's dot, but that's a rarer collision than mismatched
    // dots-over-wrong-line, and the dot-below artifact reads less confusingly.
    let render_order = space_render_order(&props.rounds, &props.spaces);

    // Per-space render chunks: line then its new-bid dots, emitted in order
    // from least- to most-desirable space. Each dot uses the subway-style white
    // fill + space-colored stroke to visually tie bidding events to the price
    // movement they caused. Round 0 dots place at y=0 so newly-trading spaces
    // appear immediately.
    let space_layers = render_order
        .iter()
        .filter_map(|&(orig_idx, space_id)| {
            let points = series.get(&space_id)?;
            let style = space_color_style(orig_idx);
            let line_html = if points.is_empty() {
                Html::default()
            } else {
                let point_str = points
                    .iter()
                    .map(|&(r, p)| {
                        format!("{},{}", round_x(r, col_w), price_to_y(p))
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                html! {
                    <polyline
                        points={point_str}
                        fill="none"
                        stroke-width={LINE_STROKE_WIDTH.to_string()}
                        class="stroke-[var(--space-light)] \
                            dark:stroke-[var(--space-dark)]"
                        style={style.clone()}
                    />
                }
            };
            let dots_html = (0..=last_frame)
                .filter_map(|r| {
                    let round = props.rounds.get(r)?;
                    if !round.bids.contains_key(&space_id) {
                        return None;
                    }
                    let price = round
                        .results
                        .iter()
                        .find(|rsr| rsr.space_id == space_id)
                        .and_then(|rsr| rsr.value.to_f64())
                        .unwrap_or(0.0);
                    let cx = round_x(r, col_w);
                    let cy = price_to_y(price);
                    Some(html! {
                        <circle
                            cx={cx.to_string()}
                            cy={cy.to_string()}
                            r={DOT_RADIUS.to_string()}
                            stroke-width={DOT_STROKE.to_string()}
                            class="fill-white dark:fill-black \
                                stroke-[var(--space-light)] \
                                dark:stroke-[var(--space-dark)]"
                            style={style.clone()}
                        />
                    })
                })
                .collect::<Html>();
            Some(html! { <>{line_html}{dots_html}</> })
        })
        .collect::<Html>();

    let svg_height_rem = svg_height / 16.0;
    let svg_style = format!("height: {:.2}rem", svg_height_rem);

    html! {
        <svg
            viewBox={viewbox}
            class="h-auto mx-auto"
            preserveAspectRatio="xMinYMid meet"
            style={svg_style}
        >
            {x_axis_html}
            {y_axis_html}
            {space_layers}
        </svg>
    }
}

#[derive(Properties, PartialEq)]
pub struct PriceLegendProps {
    pub spaces: Vec<(SpaceId, String)>,
}

/// Horizontal legend of space swatches. Each entry is a short colored pill
/// followed by the space name. Rendered separately from the chart so the
/// container can place it outside the horizontal scroll area.
#[function_component]
pub fn PriceLegend(props: &PriceLegendProps) -> Html {
    let entries = props
        .spaces
        .iter()
        .enumerate()
        .map(|(i, (_, name))| {
            let style = space_color_style(i);
            html! {
                <span class="inline-flex items-center gap-1.5 \
                    text-xs text-neutral-700 \
                    dark:text-neutral-300">
                    <span
                        class="w-6 h-1 rounded-full \
                            bg-[var(--space-light)] \
                            dark:bg-[var(--space-dark)]"
                        style={style}
                    />
                    {name.clone()}
                </span>
            }
        })
        .collect::<Html>();
    html! {
        <div class="flex flex-wrap gap-x-4 gap-y-1">
            {entries}
        </div>
    }
}

/// Returns spaces in least-to-most-desirable order, each entry carrying the
/// space's original index in `props.spaces` (for palette lookup) and its
/// `SpaceId`. More-desirable spaces render later (on top). Desirability uses
/// three nested sort keys, each a plausible heuristic that breaks ties when the
/// more-obvious ones coincide: (1) final price in the last displayed round —
/// the best direct signal of how much bidders valued it; (2) cumulative price
/// integrated over every round — picks up which space stayed expensive longest
/// when two endings coincide; (3) earliest round where anyone bid on it —
/// demand emerging earlier suggests higher desirability, a final tiebreaker for
/// spaces that stayed at $0.
fn space_render_order(
    rounds: &[SimRound],
    spaces: &[(SpaceId, String)],
) -> Vec<(usize, SpaceId)> {
    let final_price: HashMap<SpaceId, Decimal> = rounds
        .last()
        .map(|r| {
            r.results
                .iter()
                .map(|rsr| (rsr.space_id, rsr.value))
                .collect()
        })
        .unwrap_or_default();

    let mut total_price: HashMap<SpaceId, Decimal> = HashMap::new();
    for round in rounds {
        for rsr in &round.results {
            *total_price.entry(rsr.space_id).or_insert(Decimal::ZERO) +=
                rsr.value;
        }
    }

    // Earliest round a space received any bids. Spaces never bid on sort
    // after all bid-on spaces.
    let mut first_bid_round: HashMap<SpaceId, usize> = HashMap::new();
    for (r, round) in rounds.iter().enumerate() {
        for sid in round.bids.keys() {
            first_bid_round.entry(*sid).or_insert(r);
        }
    }

    // Sort key components put less-desirable spaces first (rendered at the
    // bottom, overlaid by more-desirable ones). Final and total prices sort
    // ascending (lower = less desirable). Earliest-bid round sorts with
    // never-bid spaces first, then later-bid before earlier-bid, since later
    // bidding indicates lower desirability.
    let mut ordered: Vec<(usize, SpaceId)> = spaces
        .iter()
        .enumerate()
        .map(|(i, (sid, _))| (i, *sid))
        .collect();
    ordered.sort_by_key(|(_, sid)| {
        let fp = final_price.get(sid).copied().unwrap_or(Decimal::ZERO);
        let tp = total_price.get(sid).copied().unwrap_or(Decimal::ZERO);
        // Spaces with no bids get None, which sorts before any Some(r).
        // Among Some values, we want later rounds to sort earlier
        // ("later-bid = less desirable"), so wrap in Reverse.
        let fbr = first_bid_round.get(sid).copied().map(std::cmp::Reverse);
        (fp, tp, fbr)
    });
    ordered
}

/// Builds per-space step-polyline points over rounds `0..=last`. Output shape:
/// `(round_idx, price)` pairs, with duplicate-x corner points inserted so the
/// polyline renders as a step curve. Each round r (after the first traded one)
/// contributes two points: a horizontal-carry endpoint at the prior price, and
/// a vertical-step endpoint at the new price. When the price is unchanged the
/// two points coincide in y, producing a clean horizontal segment. A space only
/// enters its series once it first has a result.
fn compute_series(
    rounds: &[SimRound],
    last: usize,
    spaces: &[(SpaceId, String)],
) -> HashMap<SpaceId, Vec<(usize, f64)>> {
    let space_ids: Vec<SpaceId> = spaces.iter().map(|(id, _)| *id).collect();
    let mut out: HashMap<SpaceId, Vec<(usize, f64)>> =
        space_ids.iter().map(|id| (*id, Vec::new())).collect();
    for r in 0..=last {
        let Some(round) = rounds.get(r) else { continue };
        let prices: HashMap<SpaceId, f64> = round
            .results
            .iter()
            .filter_map(|rsr| rsr.value.to_f64().map(|v| (rsr.space_id, v)))
            .collect();
        for sid in &space_ids {
            let Some(&price) = prices.get(sid) else {
                continue;
            };
            let Some(series) = out.get_mut(sid) else {
                continue;
            };
            if series.is_empty() {
                // First round this space trades: anchor point.
                series.push((r, price));
            } else {
                // Carry horizontal from prior round's price to this round's
                // tick, then step vertically to this round's price.
                let prev_price = series.last().map(|&(_, p)| p).unwrap_or(0.0);
                series.push((r, prev_price));
                series.push((r, price));
            }
        }
    }
    out
}

/// Per-space inline-style string setting --space-light and --space-dark CSS
/// custom properties. Same color in both modes — Tol's Light palette reads on
/// both.
fn space_color_style(idx: usize) -> String {
    let color = SPACE_PALETTE[idx % SPACE_PALETTE.len()];
    format!("--space-light: {}; --space-dark: {};", color, color)
}

/// Picks a "nice" upper bound and tick list covering `[0, max]`. Returns the
/// adjusted upper bound and a vector of tick values (including 0 and the upper
/// bound).
fn nice_y_ticks(max: f64, target_count: usize) -> (f64, Vec<f64>) {
    if max <= 0.0 {
        return (1.0, vec![0.0, 1.0]);
    }
    let raw_step = max / target_count.max(1) as f64;
    let magnitude = 10f64.powi(raw_step.log10().floor() as i32);
    let normalized = raw_step / magnitude;
    let step_mul = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    let step = step_mul * magnitude;
    let nice_max = (max / step).ceil() * step;
    let mut ticks = Vec::new();
    let mut t = 0.0;
    while t <= nice_max + step * 0.5 {
        ticks.push(t);
        t += step;
    }
    (nice_max, ticks)
}
