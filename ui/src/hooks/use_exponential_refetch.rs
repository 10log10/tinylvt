use gloo_timers::callback::Timeout;
use yew::prelude::*;

/// Hook for exponential backoff refetching
///
/// Starts with initial_delay_ms and doubles on each retry until max_delay_ms
/// is reached. If max_delay_ms is reached without success, sets error state.
///
/// Returns (start_refetch, cancel_refetch, error)
#[hook]
pub fn use_exponential_refetch(
    refetch: Callback<()>,
    initial_delay_ms: u32,
    max_delay_ms: u32,
) -> (Callback<()>, Callback<()>, Option<String>) {
    let error = use_state(|| None);
    let current_delay = use_state(|| initial_delay_ms);
    let timeout_handle = use_state(|| None::<Timeout>);

    let cancel_refetch = {
        let timeout_handle = timeout_handle.clone();
        let error = error.clone();

        use_callback((), move |_, _| {
            tracing::info!("Exponential refetch: canceling pending timeouts");
            timeout_handle.set(None); // Drop the timeout, canceling it
            error.set(None);
        })
    };

    let start_refetch = {
        let error = error.clone();
        let current_delay = current_delay.clone();
        let timeout_handle = timeout_handle.clone();
        let refetch = refetch.clone();

        use_callback(
            (initial_delay_ms, max_delay_ms),
            move |_, (initial_delay_ms, max_delay_ms)| {
                tracing::info!(
                    "Exponential refetch: starting with initial_delay={}ms, \
                     max_delay={}ms",
                    initial_delay_ms,
                    max_delay_ms
                );

                // Clear any existing error and reset delay
                error.set(None);
                current_delay.set(*initial_delay_ms);

                // Start the refetch cycle
                let error = error.clone();
                let current_delay = current_delay.clone();
                let timeout_handle = timeout_handle.clone();
                let refetch = refetch.clone();
                let initial_delay_ms = *initial_delay_ms;
                let max_delay_ms = *max_delay_ms;

                schedule_refetch(
                    initial_delay_ms,
                    max_delay_ms,
                    refetch,
                    error,
                    current_delay,
                    timeout_handle,
                );
            },
        )
    };

    let current_error = (*error).clone();

    (start_refetch, cancel_refetch, current_error)
}

fn schedule_refetch(
    delay_ms: u32,
    max_delay_ms: u32,
    refetch: Callback<()>,
    error: UseStateHandle<Option<String>>,
    current_delay: UseStateHandle<u32>,
    timeout_handle: UseStateHandle<Option<Timeout>>,
) {
    let timeout_handle_for_closure = timeout_handle.clone();

    tracing::info!("Exponential refetch: scheduling attempt in {}ms", delay_ms);

    let timeout = Timeout::new(delay_ms, move || {
        tracing::info!(
            "Exponential refetch: timeout fired after {}ms, calling refetch",
            delay_ms
        );
        refetch.emit(());

        // Calculate next delay
        let next_delay = delay_ms * 2;

        if next_delay <= max_delay_ms {
            // Schedule next refetch
            current_delay.set(next_delay);
            schedule_refetch(
                next_delay,
                max_delay_ms,
                refetch.clone(),
                error.clone(),
                current_delay.clone(),
                timeout_handle_for_closure.clone(),
            );
        } else {
            // We've hit the max delay, set error
            tracing::error!(
                "Exponential refetch: reached max delay {}ms without success",
                max_delay_ms
            );
            error.set(Some(
                "The round is taking longer than expected to process. \
                 Please refresh the page."
                    .to_string(),
            ));
        }
    });

    timeout_handle.set(Some(timeout));
}
