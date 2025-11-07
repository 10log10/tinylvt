use gloo_timers::future::sleep;
use jiff::Timestamp;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use yew::platform::spawn_local;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub target_time: Timestamp,
    #[prop_or_default]
    pub on_complete: Option<Callback<()>>,
}

#[function_component]
pub fn CountdownTimer(props: &Props) -> Html {
    let time_remaining =
        use_state(|| calculate_time_remaining(props.target_time));

    // Update the countdown every second using spawn_local
    {
        let time_remaining = time_remaining.clone();
        let target_time = props.target_time;
        let on_complete = props.on_complete.clone();

        use_effect_with(target_time, move |&target_time| {
            // Reset state immediately
            time_remaining.set(calculate_time_remaining(target_time));

            let callback_called = std::cell::Cell::new(false);
            let cancelled = Rc::new(AtomicBool::new(false));
            let cancelled_clone = cancelled.clone();

            // Spawn async task to update every second
            spawn_local(async move {
                while !cancelled_clone.load(Ordering::Relaxed) {
                    sleep(Duration::from_secs(1)).await;

                    if cancelled_clone.load(Ordering::Relaxed) {
                        break;
                    }

                    let remaining = calculate_time_remaining(target_time);

                    if remaining.is_past
                        && !callback_called.get()
                        && let Some(callback) = &on_complete
                    {
                        tracing::info!(
                            "CountdownTimer: countdown reached zero, \
                             triggering on_complete callback"
                        );
                        callback.emit(());
                        callback_called.set(true);
                    }

                    time_remaining.set(remaining);
                }
            });

            // Cleanup: signal cancellation when effect re-runs or component unmounts
            move || {
                cancelled.store(true, Ordering::Relaxed);
            }
        });
    }

    let remaining = *time_remaining;

    if remaining.is_past {
        return html! {
            <span class="text-neutral-900 dark:text-white font-mono">
                {"Starting..."}
            </span>
        };
    }

    html! {
        <span class="text-neutral-900 dark:text-white font-mono">
            {format_time_remaining(&remaining)}
        </span>
    }
}

#[derive(Clone, Copy, PartialEq)]
struct TimeRemaining {
    days: i64,
    hours: i64,
    minutes: i64,
    seconds: i64,
    is_past: bool,
}

fn calculate_time_remaining(target: Timestamp) -> TimeRemaining {
    let now = Timestamp::now();

    if now >= target {
        return TimeRemaining {
            days: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
            is_past: true,
        };
    }

    let duration = target.duration_since(now);
    let total_seconds = duration.as_secs();

    let days = total_seconds / 86400;
    let hours = (total_seconds % 86400) / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    TimeRemaining {
        days,
        hours,
        minutes,
        seconds,
        is_past: false,
    }
}

fn format_time_remaining(remaining: &TimeRemaining) -> String {
    if remaining.days > 0 {
        format!(
            "{}d {:02}h {:02}m {:02}s",
            remaining.days,
            remaining.hours,
            remaining.minutes,
            remaining.seconds
        )
    } else if remaining.hours > 0 {
        format!(
            "{:02}h {:02}m {:02}s",
            remaining.hours, remaining.minutes, remaining.seconds
        )
    } else if remaining.minutes > 0 {
        format!("{:02}m {:02}s", remaining.minutes, remaining.seconds)
    } else {
        format!("{}s", remaining.seconds)
    }
}
