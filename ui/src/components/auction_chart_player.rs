use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use payloads::auction_sim::SimRound;
use payloads::{CurrencySettings, SpaceId};
use rust_decimal::Decimal;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::platform::spawn_local;
use yew::prelude::*;

use crate::components::AuctionChart;

const TOTAL_ANIMATION_MS: u64 = 10_000;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub spaces: Vec<(SpaceId, String)>,
    pub rounds: Vec<SimRound>,
    pub currency: CurrencySettings,
    #[prop_or(false)]
    pub autoplay: bool,
}

/// Frames for visualization:
///
/// Frame 0 ("Round 0"): no high bidders, round 0's bids
/// Frame 1 ("Round 1"): round 0's results, round 1's bids
/// ...
/// Frame N ("Round N"): round N-1's results, round N's bids (none)
/// Frame N+1 ("Concluded"): round N's results, no bids
///
/// Total frames = rounds.len() + 1
#[function_component]
pub fn AuctionChartPlayer(props: &Props) -> Html {
    let current_frame = use_state(|| 0_usize);
    let prev_num_frames = use_state(|| 0_usize);
    let is_playing = use_state(|| false);
    // Shared cancel token. use_mut_ref avoids re-renders
    // when we swap the token.
    let cancel_ref = use_mut_ref(|| Rc::new(Cell::new(false)));

    let num_frames = props.rounds.len() + 1;

    // If we were on the last frame (concluded) and the number
    // of rounds changed, stay on the new last frame. Also clamp
    // if the current frame is now out of bounds.
    {
        let current_frame = current_frame.clone();
        let prev_num_frames = prev_num_frames.clone();
        let is_playing = is_playing.clone();
        let cancel_ref = cancel_ref.clone();
        let was_at_end =
            *prev_num_frames > 0 && *current_frame >= *prev_num_frames - 1;
        use_effect_with(num_frames, move |&num_frames| {
            prev_num_frames.set(num_frames);
            let last = num_frames.saturating_sub(1);
            if was_at_end || *current_frame > last {
                current_frame.set(last);
            }
            // Stop playback when data changes
            cancel_ref.borrow().set(true);
            is_playing.set(false);
        });
    }

    // Autoplay on mount
    {
        let autoplay = props.autoplay;
        let is_playing = is_playing.clone();
        let current_frame = current_frame.clone();
        let cancel_ref = cancel_ref.clone();
        use_effect_with((), move |_| {
            if autoplay && num_frames > 1 {
                start_playback(
                    current_frame,
                    is_playing,
                    cancel_ref,
                    0,
                    num_frames,
                    true,
                );
            }
        });
    }

    // Clamp in case the effect hasn't run yet this render
    let frame = (*current_frame).min(num_frames.saturating_sub(1));

    let x_max = props
        .rounds
        .iter()
        .flat_map(|r| r.results.iter().map(|rsr| rsr.value))
        .max()
        .unwrap_or(Decimal::ZERO);

    // Previous round's results (high bidders + prices)
    let results = if frame == 0 {
        // Frame 0: no previous results
        Vec::new()
    } else {
        // Frames 1..=N+1: show results from round (frame - 1)
        props
            .rounds
            .get(frame - 1)
            .map(|r| r.results.clone())
            .unwrap_or_default()
    };

    // Current round's bids
    let bids = if frame < props.rounds.len() {
        props.rounds[frame].bids.clone()
    } else {
        // Final "Concluded" frame: no bids
        Default::default()
    };

    let is_concluded = frame >= props.rounds.len();

    let label = if is_concluded {
        "Concluded".to_string()
    } else {
        format!(
            "Round {}",
            props.rounds.get(frame).map(|r| r.round_num).unwrap_or(0)
        )
    };

    let at_start = frame == 0;
    let at_end = frame >= num_frames - 1;

    let on_play = {
        let current_frame = current_frame.clone();
        let is_playing = is_playing.clone();
        let cancel_ref = cancel_ref.clone();
        Callback::from(move |_: MouseEvent| {
            if *is_playing {
                // Pause
                cancel_ref.borrow().set(true);
                is_playing.set(false);
            } else {
                // If at the end, restart from the beginning
                // and dwell on frame 0 before advancing
                let (start, delay) = if *current_frame >= num_frames - 1 {
                    current_frame.set(0);
                    (0, true)
                } else {
                    (*current_frame, false)
                };
                start_playback(
                    current_frame.clone(),
                    is_playing.clone(),
                    cancel_ref.clone(),
                    start,
                    num_frames,
                    delay,
                );
            }
        })
    };

    let on_prev = {
        let current_frame = current_frame.clone();
        let is_playing = is_playing.clone();
        let cancel_ref = cancel_ref.clone();
        Callback::from(move |_: MouseEvent| {
            // Stop playback on manual navigation
            cancel_ref.borrow().set(true);
            is_playing.set(false);
            if *current_frame > 0 {
                current_frame.set(*current_frame - 1);
            }
        })
    };

    let on_next = {
        let current_frame = current_frame.clone();
        let is_playing = is_playing.clone();
        let cancel_ref = cancel_ref.clone();
        Callback::from(move |_: MouseEvent| {
            cancel_ref.borrow().set(true);
            is_playing.set(false);
            if *current_frame < num_frames - 1 {
                current_frame.set(*current_frame + 1);
            }
        })
    };

    let on_slider = {
        let current_frame = current_frame.clone();
        let is_playing = is_playing.clone();
        let cancel_ref = cancel_ref.clone();
        Callback::from(move |e: InputEvent| {
            cancel_ref.borrow().set(true);
            is_playing.set(false);
            if let Some(input) = e
                .target()
                .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
                && let Ok(val) = input.value().parse::<usize>()
            {
                current_frame.set(val);
            }
        })
    };

    let button_base =
        "px-3 py-1.5 text-sm font-medium rounded transition-colors";
    let button_enabled = "bg-neutral-200 dark:bg-neutral-700 \
        text-neutral-700 dark:text-neutral-300 \
        hover:bg-neutral-300 dark:hover:bg-neutral-600";
    let button_disabled = "bg-neutral-100 dark:bg-neutral-800 \
        text-neutral-400 dark:text-neutral-600 \
        cursor-not-allowed";

    let max_val = (num_frames - 1).to_string();
    let cur_val = frame.to_string();
    let play_icon = if *is_playing {
        // Pause: two vertical bars
        html! {
            <svg viewBox="0 0 16 16" class="w-4 h-4 fill-current">
                <rect x="3" y="2" width="4" height="12" />
                <rect x="9" y="2" width="4" height="12" />
            </svg>
        }
    } else {
        // Play: right-pointing triangle
        html! {
            <svg viewBox="0 0 16 16" class="w-4 h-4 fill-current">
                <polygon points="3,2 13,8 3,14" />
            </svg>
        }
    };

    html! {
        <div class="space-y-3">
            // Desktop: label | play | slider | prev/next
            // Mobile: label + play + prev/next row, slider below
            <div class="flex flex-wrap items-center justify-between">
                <span class="text-sm font-medium \
                    text-neutral-700 dark:text-neutral-300 \
                    w-[4.5rem] shrink-0">
                    {label}
                </span>

                // Play + slider (desktop)
                <div class="hidden sm:flex items-center \
                    gap-2 flex-1 max-w-72">
                    <button
                        onclick={on_play.clone()}
                        class={classes!(button_base, button_enabled)}
                    >
                        {play_icon.clone()}
                    </button>
                    <input
                        type="range"
                        min="0"
                        max={max_val}
                        value={cur_val}
                        oninput={on_slider.clone()}
                        class="flex-1 h-1.5 py-2 \
                            accent-neutral-500 cursor-pointer"
                    />
                </div>

                <div class="flex gap-2">
                    <button
                        class={classes!(
                            button_base,
                            if at_start {
                                button_disabled
                            } else {
                                button_enabled
                            }
                        )}
                        disabled={at_start}
                        onclick={on_prev}
                    >
                        {"Prev"}
                    </button>
                    <button
                        class={classes!(
                            button_base,
                            if at_end {
                                button_disabled
                            } else {
                                button_enabled
                            }
                        )}
                        disabled={at_end}
                        onclick={on_next}
                    >
                        {"Next"}
                    </button>
                </div>

                // Play + slider (mobile)
                <div class="sm:hidden w-full flex items-center gap-3 my-4">
                    <button
                        onclick={on_play}
                        class={classes!(button_base, button_enabled)}
                    >
                        {play_icon.clone()}
                    </button>
                    <input
                        type="range"
                        min="0"
                        max={(num_frames - 1).to_string()}
                        value={frame.to_string()}
                        oninput={on_slider.clone()}
                        class="flex-1 h-1.5 accent-neutral-500 cursor-pointer"
                    />
                </div>
            </div>

            <AuctionChart
                spaces={props.spaces.clone()}
                results={results}
                bids={bids}
                x_max={x_max}
                currency={props.currency.clone()}
            />
        </div>
    }
}

fn start_playback(
    current_frame: UseStateHandle<usize>,
    is_playing: UseStateHandle<bool>,
    cancel_ref: Rc<std::cell::RefCell<Rc<Cell<bool>>>>,
    start_from: usize,
    num_frames: usize,
    initial_delay: bool,
) {
    let cancel = Rc::new(Cell::new(false));
    *cancel_ref.borrow_mut() = cancel.clone();
    is_playing.set(true);

    let delay_ms = if num_frames > 1 {
        TOTAL_ANIMATION_MS / (num_frames as u64 - 1)
    } else {
        TOTAL_ANIMATION_MS
    };
    let delay = Duration::from_millis(delay_ms);

    spawn_local(async move {
        // Track position locally so we don't depend on
        // UseStateHandle's stale deref value
        let mut pos = start_from;

        if initial_delay {
            gloo_timers::future::sleep(delay).await;
            if cancel.get() {
                return;
            }
        }

        loop {
            pos += 1;
            if pos >= num_frames - 1 {
                // Show final frame and stop immediately
                current_frame.set(num_frames - 1);
                is_playing.set(false);
                break;
            }
            current_frame.set(pos);
            gloo_timers::future::sleep(delay).await;
            if cancel.get() {
                break;
            }
        }
    });
}
