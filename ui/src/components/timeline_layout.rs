//! Shared layout primitives for the auction-timeline views (subway diagram and
//! price chart). These SVGs live inside the same horizontal scroll container
//! and use a common x-axis geometry so round columns line up vertically across
//! plots.
//!
//! The container (`AuctionTimeline`) owns the `col_w` state; each child plot
//! takes it as a prop and uses these helpers to convert rounds to viewBox
//! x-coordinates and to decide which round labels to show.

pub const LEFT_PAD: f64 = 80.0;
pub const RIGHT_PAD: f64 = 0.0;
pub const TOP_PAD: f64 = 28.0;
pub const BOTTOM_PAD: f64 = 6.0;

/// Bounds for the container's horizontal-scale slider, in viewBox units per
/// round. Larger values stretch the time axis; vertical dimensions in each plot
/// are unaffected so text and marks stay the same rendered size.
pub const MIN_COL_W: f64 = 12.0;
pub const MAX_COL_W: f64 = 72.0;
pub const DEFAULT_COL_W: f64 = 36.0;

/// ViewBox x-coordinate of the center of round `round_idx`.
pub fn round_x(round_idx: usize, col_w: f64) -> f64 {
    LEFT_PAD + (round_idx as f64 + 0.5) * col_w
}

/// How many rounds to skip between axis labels, so labels don't overlap when
/// col_w is too small for the widest digit count. Gridlines still render at
/// every round; only text labels are affected.
pub fn label_stride_for(max_round_num: i32, col_w: f64) -> i32 {
    // Calculate the number of digits in the round number, which is always
    // non-negative. Returns 1 for non-positive numbers.
    let digits = max_round_num.checked_ilog10().unwrap_or(0) + 1;
    match digits {
        0 | 1 => 1,
        2 => {
            if col_w >= 20.0 {
                1
            } else {
                2
            }
        }
        _ => {
            if col_w >= 28.0 {
                1
            } else if col_w >= 14.0 {
                2
            } else {
                5
            }
        }
    }
}
