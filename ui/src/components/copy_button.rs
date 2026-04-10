use gloo_timers::callback::Timeout;
use wasm_bindgen_futures::JsFuture;
use yew::prelude::*;

#[derive(PartialEq, Clone, Copy)]
pub enum CopyStatus {
    Idle,
    Copied,
    Error,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    /// The text to copy to the clipboard when the button is clicked.
    pub text: String,
    /// Optional extra classes to append to the default button styling.
    #[prop_or_default]
    pub extra_class: Classes,
}

/// A button that copies `text` to the clipboard when clicked.
///
/// The clipboard write is initiated synchronously inside the click handler
/// so that the browser's transient user activation is preserved — this
/// matters on Safari/iOS, where calling `navigator.clipboard.writeText`
/// after an await boundary is rejected.
#[function_component]
pub fn CopyButton(props: &Props) -> Html {
    let status = use_state(|| CopyStatus::Idle);
    // Hold the pending reset timeout so it's cancelled if the component
    // re-renders or the button is clicked again before it fires.
    let reset_timeout = use_state(|| None::<Timeout>);

    let on_click = {
        let text = props.text.clone();
        let status = status.clone();
        let reset_timeout = reset_timeout.clone();
        Callback::from(move |_: MouseEvent| {
            // Grab the clipboard synchronously — critical for preserving
            // the user activation gesture on Safari.
            let Some(window) = web_sys::window() else {
                status.set(CopyStatus::Error);
                return;
            };
            let clipboard = window.navigator().clipboard();
            let promise = clipboard.write_text(&text);

            let status_for_future = status.clone();
            let reset_timeout_for_future = reset_timeout.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match JsFuture::from(promise).await {
                    Ok(_) => {
                        status_for_future.set(CopyStatus::Copied);
                    }
                    Err(_) => {
                        status_for_future.set(CopyStatus::Error);
                    }
                }
                // Reset status back to idle after a short delay so the
                // feedback is transient.
                let status_for_timeout = status_for_future.clone();
                let timeout = Timeout::new(2000, move || {
                    status_for_timeout.set(CopyStatus::Idle);
                });
                reset_timeout_for_future.set(Some(timeout));
            });
        })
    };

    let (icon, state_class, title) = match *status {
        CopyStatus::Idle => (
            clipboard_icon(),
            "text-neutral-700 dark:text-neutral-300 \
             bg-white dark:bg-neutral-700 \
             border-neutral-300 dark:border-neutral-600 \
             hover:bg-neutral-50 dark:hover:bg-neutral-600",
            "Copy to clipboard",
        ),
        CopyStatus::Copied => (
            check_icon(),
            "text-green-700 dark:text-green-400 \
             bg-green-50 dark:bg-green-900/20 \
             border-green-300 dark:border-green-700",
            "Copied",
        ),
        CopyStatus::Error => (
            x_icon(),
            "text-red-700 dark:text-red-400 \
             bg-red-50 dark:bg-red-900/20 \
             border-red-300 dark:border-red-700",
            "Copy failed",
        ),
    };

    let base_class = "inline-flex items-center justify-center p-2 border \
                      rounded-md transition-colors disabled:opacity-50";

    html! {
        <button
            type="button"
            onclick={on_click}
            class={classes!(base_class, state_class, props.extra_class.clone())}
            title={title}
            aria-label={title}
        >
            {icon}
        </button>
    }
}

fn clipboard_icon() -> Html {
    html! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            class="w-4 h-4"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
        >
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
        </svg>
    }
}

fn check_icon() -> Html {
    html! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            class="w-4 h-4"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
        >
            <polyline points="20 6 9 17 4 12" />
        </svg>
    }
}

fn x_icon() -> Html {
    html! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            class="w-4 h-4"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
        >
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
        </svg>
    }
}
