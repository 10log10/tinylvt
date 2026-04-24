use std::collections::{HashMap, HashSet};

use payloads::auction_sim::SimRound;
use payloads::{CurrencySettings, SpaceId, UserId, responses};
use rust_decimal::Decimal;
use yew::prelude::*;

use crate::components::timeline_layout::{
    BOTTOM_PAD, LEFT_PAD, RIGHT_PAD, TOP_PAD, label_stride_for, round_x,
};
use crate::components::user_identity_display::{
    format_user_name_unambiguous, render_user_name,
};

#[derive(Properties, PartialEq)]
pub struct Props {
    /// (space_id, name) pairs in display order (top to bottom)
    pub spaces: Vec<(SpaceId, String)>,
    /// Global bidder ordering. Lane positions within a space preserve this
    /// relative order.
    pub bidders: Vec<responses::UserIdentity>,
    /// All simulation rounds. Lane allocation uses the full series so lanes
    /// don't reflow as playback advances.
    pub rounds: Vec<SimRound>,
    /// Which frame to display. Controls the horizontal extent of rendered
    /// activity, not the column count.
    pub frame: usize,
    /// Bid increment used by the simulation. Needed to compute the price a new
    /// bid was placed at (prior round's price plus the increment).
    pub bid_increment: Decimal,
    /// Currency used to format prices in tooltips.
    pub currency: CurrencySettings,
    /// ViewBox units per round. Controlled by the parent container so that
    /// this diagram and the price chart share a common time axis.
    pub col_w: f64,
}

// Layout constants (SVG user units) specific to the subway diagram.
const SEGMENT_STROKE_WIDTH: f64 = 10.0;
const LANE_SPACING: f64 = SEGMENT_STROKE_WIDTH;
const BAND_PAD: f64 = 16.0;
// New-bid dot: a plain filled circle sitting on top of the track. Radius is
// track half-width minus DOT_STROKE so a track-colored ring of thickness
// DOT_STROKE remains visible around the dot.
const DOT_STROKE: f64 = 1.5;
const DOT_RADIUS: f64 = SEGMENT_STROKE_WIDTH / 2.0 - DOT_STROKE;

// Per-bidder colors use Paul Tol's "Bright" qualitative palette, a
// color-blind-safe scheme with good hue variety. Bidders cycle through the
// palette by index. Not greyscale-convertible, but conventional greyscale-safe
// palettes with this much hue variety are rare.
// https://sronpersonalpages.nl/~pault/#sec:qualitative
const BIDDER_PALETTE: &[&str] = &[
    "#4477AA", // blue
    "#EE6677", // red
    "#228833", // green
    "#CCBB44", // yellow
    "#66CCEE", // cyan
    "#AA3377", // purple
    "#BBBBBB", // grey
];

#[function_component]
pub fn SubwayDiagram(props: &Props) -> Html {
    let col_w = props.col_w;

    // Per-space lane order: bidders who ever bid on that space, filtered from
    // props.bidders to preserve global ordering.
    let lane_order: HashMap<SpaceId, Vec<UserId>> =
        compute_lane_order(&props.spaces, &props.bidders, &props.rounds);

    // Per-space band height: scales with the number of lanes. With N lanes
    // there are N-1 gaps between lane centers, plus BAND_PAD above the first
    // and below the last.
    let band_heights: Vec<f64> = props
        .spaces
        .iter()
        .map(|(sid, _)| {
            let n = lane_order.get(sid).map(|v| v.len()).unwrap_or(0);
            (n.saturating_sub(1)) as f64 * LANE_SPACING + BAND_PAD * 2.0
        })
        .collect();

    // Y offset (top edge) of each band in SVG coords.
    let band_tops: Vec<f64> = {
        let mut acc = TOP_PAD;
        let mut out = Vec::with_capacity(band_heights.len());
        for h in &band_heights {
            out.push(acc);
            acc += h;
        }
        out
    };

    let num_rounds = props.rounds.len();
    let svg_height = TOP_PAD + band_heights.iter().sum::<f64>() + BOTTOM_PAD;
    let svg_width = LEFT_PAD + (num_rounds as f64).max(1.0) * col_w + RIGHT_PAD;

    let viewbox = format!("0 0 {} {}", svg_width, svg_height);

    // Band backgrounds (alternating neutral stripes).
    let bands = props
        .spaces
        .iter()
        .enumerate()
        .map(|(i, (_sid, name))| {
            let y = band_tops[i];
            let h = band_heights[i];
            let stripe_class = if i % 2 == 0 {
                "fill-white dark:fill-neutral-900"
            } else {
                "fill-neutral-100 dark:fill-neutral-800"
            };
            let label_y = y + h / 2.0;
            html! {
                <>
                    <rect
                        x="0"
                        y={y.to_string()}
                        width={svg_width.to_string()}
                        height={h.to_string()}
                        class={stripe_class}
                    />
                    <text
                        x={(LEFT_PAD - 8.0).to_string()}
                        y={label_y.to_string()}
                        text-anchor="end"
                        dominant-baseline="central"
                        class="fill-neutral-700 dark:fill-neutral-300 \
                            text-xs"
                    >
                        {name.clone()}
                    </text>
                </>
            }
        })
        .collect::<Html>();

    // Per (space, bidder) new-bid entries through `frame`, each carrying the
    // round number and the price of the bid.
    let last_frame = props.frame.min(num_rounds.saturating_sub(1));
    let bids: HashMap<(SpaceId, UserId), Vec<BidEntry>> =
        compute_bids(&props.rounds, last_frame, props.bid_increment);

    // (space, bidder) -> lane_y, used to place dots and resolve segment
    // endpoints.
    let lane_y_map: HashMap<(SpaceId, UserId), f64> = props
        .spaces
        .iter()
        .enumerate()
        .flat_map(|(space_idx, (space_id, _))| {
            let band_top = band_tops[space_idx];
            lane_order
                .get(space_id)
                .map(|lanes| {
                    lanes
                        .iter()
                        .enumerate()
                        .map(move |(lane_i, uid)| {
                            ((*space_id, *uid), lane_y(band_top, lane_i))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect();

    // Bidder -> (stack order, inline style). The stroke classes read the custom
    // properties via Tailwind's `dark:` variant. Stack order equals the
    // bidder's position in `props.bidders` and breaks ties when two segments
    // have the same slope.
    let bidder_info: HashMap<UserId, (usize, String)> = props
        .bidders
        .iter()
        .enumerate()
        .map(|(i, b)| (b.user_id, (i, bidder_color_style(i))))
        .collect();

    // Bidder lookup for tooltip labels.
    let bidder_names: HashMap<UserId, String> = props
        .bidders
        .iter()
        .map(|b| (b.user_id, format_user_name_unambiguous(b)))
        .collect();

    let segments = render_segments(
        &compute_segments(&props.rounds, last_frame),
        &lane_y_map,
        &bidder_info,
        col_w,
    );
    let dots = render_dots(
        &bids,
        &lane_y_map,
        &bidder_info,
        &bidder_names,
        &props.currency,
        col_w,
    );

    // Round axis labels (top) + vertical gridlines. Gridlines always draw per
    // round, but labels skip rounds when col_w is too small to fit the digit
    // width without overlap.
    let max_round_num =
        props.rounds.iter().map(|r| r.round_num).max().unwrap_or(0);
    let label_stride = label_stride_for(max_round_num, col_w);
    let axis = (0..num_rounds)
        .map(|r| {
            let x = round_x(r, col_w);
            let round_num = props.rounds[r].round_num;
            let grid_class = if r <= props.frame {
                "stroke-neutral-300 dark:stroke-neutral-700"
            } else {
                "stroke-neutral-200 dark:stroke-neutral-800"
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

    // SVG rendered height is fixed in rem, and width flows from the viewBox
    // aspect ratio. Since col_w grows the viewBox horizontally without touching
    // vertical geometry, the rendered height stays constant while the rendered
    // width expands/compresses with col_w. The svg_height viewBox units map to
    // pixels assuming the default 1rem = 16px, so dividing by 16 gives us rem.
    let svg_height_rem = svg_height / 16.0;
    let svg_style = format!("height: {:.2}rem", svg_height_rem);

    html! {
        <svg
            viewBox={viewbox}
            class="h-auto mx-auto"
            preserveAspectRatio="xMinYMid meet"
            style={svg_style}
        >
            {bands}
            {axis}
            {segments}
            {dots}
        </svg>
    }
}

#[derive(Properties, PartialEq)]
pub struct SubwayLegendProps {
    pub bidders: Vec<responses::UserIdentity>,
}

/// Horizontal legend of bidder swatches. Each entry is a short colored pill
/// followed by the bidder's display name. Rendered separately from the
/// diagram so the container can place it outside the horizontal scroll area.
#[function_component]
pub fn SubwayLegend(props: &SubwayLegendProps) -> Html {
    let entries = props
        .bidders
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let style = bidder_color_style(i);
            html! {
                <span class="inline-flex items-center gap-1.5 \
                    text-xs text-neutral-700 \
                    dark:text-neutral-300">
                    <span
                        class="w-6 h-1 rounded-full \
                            bg-[var(--subway-light)] \
                            dark:bg-[var(--subway-dark)]"
                        style={style}
                    />
                    {render_user_name(b)}
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

/// Renders each segment as an SVG `<line>`, ordered so that steeper switches
/// render first (deeper) and horizontal continuations render last (on top).
/// Ties within a slope break by bidder index to ensure consistency across
/// frames.
fn render_segments(
    segments: &[Segment],
    lane_y_map: &HashMap<(SpaceId, UserId), f64>,
    bidder_info: &HashMap<UserId, (usize, String)>,
    col_w: f64,
) -> Html {
    let mut entries: Vec<(u64, usize, Html)> = segments
        .iter()
        .filter_map(|seg| {
            let y0 = *lane_y_map.get(&(seg.space0, seg.bidder))?;
            let y1 = *lane_y_map.get(&(seg.space1, seg.bidder))?;
            let (z, style) = bidder_info.get(&seg.bidder)?.clone();
            // Multiply by 1000 so sub-unit |dy| differences survive the u64
            // cast (in practice dy is an integer multiple of LANE_SPACING, but
            // this is cheap insurance).
            let dy_key = ((y1 - y0).abs() * 1000.0) as u64;
            let node = html! {
                <line
                    x1={round_x(seg.r0, col_w).to_string()}
                    y1={y0.to_string()}
                    x2={round_x(seg.r1, col_w).to_string()}
                    y2={y1.to_string()}
                    stroke-width={SEGMENT_STROKE_WIDTH.to_string()}
                    stroke-linecap="round"
                    class="stroke-[var(--subway-light)] \
                        dark:stroke-[var(--subway-dark)]"
                    style={style}
                />
            };
            Some((dy_key, z, node))
        })
        .collect();
    entries.sort_by_key(|(dy, z, _)| (std::cmp::Reverse(*dy), *z));
    entries.into_iter().map(|(_, _, h)| h).collect()
}

/// Renders one dot per (space, round, bidder) new-bid entry. New-bid dots are
/// a neutral-colored fill sitting inside the track, emphasizing the
/// price-changing event. Segment rounded end caps handle the visual rounding
/// at high-bid vertices, so those don't need their own dots.
/// No sort needed: positions are unique to (space, bidder, round) and
/// same-round dots in adjacent lanes touch at their edges without overlapping.
fn render_dots(
    bids: &HashMap<(SpaceId, UserId), Vec<BidEntry>>,
    lane_y_map: &HashMap<(SpaceId, UserId), f64>,
    bidder_info: &HashMap<UserId, (usize, String)>,
    bidder_names: &HashMap<UserId, String>,
    currency: &CurrencySettings,
    col_w: f64,
) -> Html {
    bids.iter()
        .flat_map(|((space_id, uid), entries)| {
            if let Some(&cy) = lane_y_map.get(&(*space_id, *uid))
                && let Some((_, style)) = bidder_info.get(uid)
                && let Some(name) = bidder_names.get(uid).cloned()
            {
                entries
                    .iter()
                    .map(|entry| {
                        let cx = round_x(entry.round, col_w);
                        let price_str = currency.format_amount(entry.price);
                        let tooltip = format!(
                            "{} — round {} — new bid at {}",
                            name, entry.round, price_str,
                        );
                        // Zero-length rounded-cap stroke draws a track-colored
                        // disc via the same rasterizer as the segments, so dots
                        // without an adjacent segment (e.g. round 0) still get
                        // the track ring, and dots that do have segments line
                        // up pixel-perfectly with them.
                        html! {
                            <>
                                <line
                                    x1={cx.to_string()}
                                    y1={cy.to_string()}
                                    x2={cx.to_string()}
                                    y2={cy.to_string()}
                                    stroke-width={
                                        SEGMENT_STROKE_WIDTH.to_string()
                                    }
                                    stroke-linecap="round"
                                    class="stroke-[var(--subway-light)] \
                                        dark:stroke-[var(--subway-dark)]"
                                    style={style.clone()}
                                />
                                <circle
                                    cx={cx.to_string()}
                                    cy={cy.to_string()}
                                    r={DOT_RADIUS.to_string()}
                                    class="fill-white dark:fill-black"
                                >
                                    <title>{tooltip}</title>
                                </circle>
                            </>
                        }
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        })
        .collect()
}

/// Per-bidder inline-style string setting --subway-light and --subway-dark CSS
/// custom properties. The corresponding stroke classes on each element read
/// whichever applies to the active color mode via Tailwind's `dark:` variant.
/// Bidders cycle through BIDDER_PALETTE; the same color is used in both modes
/// since Tol's palette reads well on both white and dark backgrounds.
pub fn bidder_color_style(idx: usize) -> String {
    let color = BIDDER_PALETTE[idx % BIDDER_PALETTE.len()];
    format!("--subway-light: {}; --subway-dark: {};", color, color)
}

/// Returns true when a bidder pill at this palette index reads better with dark
/// text than white text. The bright palette mixes mid-tones (dark on white) and
/// light tones (dark on dark), so foreground color varies per index.
pub fn bidder_pill_needs_dark_text(idx: usize) -> bool {
    // Indices into BIDDER_PALETTE with light-enough backgrounds for dark text:
    // 3 = #CCBB44 yellow, 4 = #66CCEE cyan, 6 = #BBBBBB grey.
    matches!(idx % BIDDER_PALETTE.len(), 3 | 4 | 6)
}

/// Vertical center of a lane within its band. The first lane sits BAND_PAD
/// below the band top; each subsequent lane is LANE_SPACING below the previous.
fn lane_y(band_top: f64, lane_idx: usize) -> f64 {
    band_top + BAND_PAD + lane_idx as f64 * LANE_SPACING
}

struct Segment {
    bidder: UserId,
    r0: usize,
    space0: SpaceId,
    r1: usize,
    space1: SpaceId,
}

/// Computes the connecting line segments for each bidder's activity across
/// adjacent rounds in `0..=last`.
///
/// Rules per adjacent pair (r, r+1) and per bidder, letting A = spaces present
/// in r, B = spaces present in r+1:
/// - For each space in A ∩ B: connect within that space (continuation).
/// - For each (stopped in A\B, new in B\A) pair: connect them (activity flows
///   from stopped spaces to newly-joined ones, fully bipartite when multiples
///   on both sides).
/// - If A\B is empty but B\A is not, new spaces emerge as unconnected branches.
/// - If B\A is empty but A\B is not, stopped spaces end as unconnected
///   branches.
fn compute_segments(rounds: &[SimRound], last: usize) -> Vec<Segment> {
    // bidder -> round -> set of spaces present
    let mut presence_by_bidder_round: HashMap<
        UserId,
        HashMap<usize, HashSet<SpaceId>>,
    > = HashMap::new();
    for r in 0..=last {
        let Some(round) = rounds.get(r) else { continue };
        for (sid, bidders_this_round) in &round.bids {
            for b in bidders_this_round {
                presence_by_bidder_round
                    .entry(b.user_id)
                    .or_default()
                    .entry(r)
                    .or_default()
                    .insert(*sid);
            }
        }
        if r > 0
            && let Some(prev) = rounds.get(r - 1)
        {
            for rsr in &prev.results {
                presence_by_bidder_round
                    .entry(rsr.winner.user_id)
                    .or_default()
                    .entry(r)
                    .or_default()
                    .insert(rsr.space_id);
            }
        }
    }

    let mut out = Vec::new();
    for (bidder, by_round) in presence_by_bidder_round {
        for r in 0..last {
            let (Some(a), Some(b)) = (by_round.get(&r), by_round.get(&(r + 1)))
            else {
                continue;
            };
            // Continuations
            for sid in a.intersection(b) {
                out.push(Segment {
                    bidder,
                    r0: r,
                    space0: *sid,
                    r1: r + 1,
                    space1: *sid,
                });
            }
            // Switches: bipartite across stopped × new
            let stopped: Vec<&SpaceId> = a.difference(b).collect();
            let new_spaces: Vec<&SpaceId> = b.difference(a).collect();
            for s_from in &stopped {
                for s_to in &new_spaces {
                    out.push(Segment {
                        bidder,
                        r0: r,
                        space0: **s_from,
                        r1: r + 1,
                        space1: **s_to,
                    });
                }
            }
        }
    }
    out
}

/// A single new-bid entry: which round the bid was placed in, and the price
/// that bid was placed at.
#[derive(Clone, Copy)]
struct BidEntry {
    round: usize,
    price: Decimal,
}

/// For each (space, bidder), new-bid entries in `0..=last` where that bidder
/// placed a new bid. `price` is the new bid price: `prior + bid_increment`, or
/// 0 if the space has no prior result. Carried-forward high bidders are not
/// included here — their segments' rounded end caps serve as the visual
/// vertex.
fn compute_bids(
    rounds: &[SimRound],
    last: usize,
    bid_increment: Decimal,
) -> HashMap<(SpaceId, UserId), Vec<BidEntry>> {
    let mut out: HashMap<(SpaceId, UserId), Vec<BidEntry>> = HashMap::new();
    for r in 0..=last {
        let Some(round) = rounds.get(r) else { continue };
        // Prior-round results indexed by space, for pricing new bids this
        // round.
        let prev_price: HashMap<SpaceId, Decimal> = if r == 0 {
            HashMap::new()
        } else {
            rounds
                .get(r - 1)
                .map(|prev| {
                    prev.results
                        .iter()
                        .map(|rsr| (rsr.space_id, rsr.value))
                        .collect()
                })
                .unwrap_or_default()
        };
        for (sid, bidders_this_round) in &round.bids {
            // New bid price = prior-round price + increment, or 0 if the space
            // has no prior result.
            let price = prev_price
                .get(sid)
                .map(|p| *p + bid_increment)
                .unwrap_or(Decimal::ZERO);
            for b in bidders_this_round {
                out.entry((*sid, b.user_id))
                    .or_default()
                    .push(BidEntry { round: r, price });
            }
        }
    }
    out
}

/// For each space, the ordered list of user_ids that ever placed a bid on that
/// space, filtered from the global `bidders` list to preserve ordering.
fn compute_lane_order(
    spaces: &[(SpaceId, String)],
    bidders: &[responses::UserIdentity],
    rounds: &[SimRound],
) -> HashMap<SpaceId, Vec<UserId>> {
    // Collect the set of user_ids that bid on each space across all rounds.
    let mut bid_sets: HashMap<SpaceId, HashSet<UserId>> = HashMap::new();
    for round in rounds {
        for (sid, bidders_this_round) in &round.bids {
            let set = bid_sets.entry(*sid).or_default();
            for b in bidders_this_round {
                set.insert(b.user_id);
            }
        }
    }

    spaces
        .iter()
        .map(|(sid, _)| {
            let set = bid_sets.remove(sid).unwrap_or_default();
            let ordered: Vec<UserId> = bidders
                .iter()
                .filter(|b| set.contains(&b.user_id))
                .map(|b| b.user_id)
                .collect();
            (*sid, ordered)
        })
        .collect()
}
