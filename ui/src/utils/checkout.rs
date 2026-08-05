//! Shared plumbing for the payment UI's actions: the busy/error state
//! pattern for API calls, Stripe Checkout redirects, and the post-redirect
//! return notice.

use payloads::{CheckoutSessionResponse, ClientError};
use wasm_bindgen::prelude::*;
use yew::prelude::*;

/// Run an API action with the shared busy/error handling: set the busy
/// flag, clear the error, await `fetch`, then hand the result to
/// `on_done`, whose returned message (if any) becomes the displayed
/// error. The busy flag clears when the action finishes.
pub fn run_action<T, F, D>(
    is_busy: UseStateHandle<bool>,
    error: UseStateHandle<Option<String>>,
    fetch: F,
    on_done: D,
) where
    F: Future<Output = Result<T, ClientError>> + 'static,
    D: FnOnce(Result<T, ClientError>) -> Option<String> + 'static,
    T: 'static,
{
    is_busy.set(true);
    error.set(None);
    yew::platform::spawn_local(async move {
        error.set(on_done(fetch.await));
        is_busy.set(false);
    });
}

/// Start a checkout redirect: set the busy flag, run `fetch` for a
/// Stripe-hosted session, and navigate to its URL. On failure, record
/// the error and clear the flag; on success the flag stays set — the
/// page is navigating away, so re-enabling the button would only
/// invite a second click.
pub fn redirect_to_checkout<F>(
    is_busy: UseStateHandle<bool>,
    error: UseStateHandle<Option<String>>,
    fetch: F,
) where
    F: Future<Output = Result<CheckoutSessionResponse, ClientError>> + 'static,
{
    is_busy.set(true);
    error.set(None);
    yew::platform::spawn_local(async move {
        match fetch.await {
            Ok(response) => {
                if let Some(window) = web_sys::window() {
                    let _ = window.location().set_href(&response.checkout_url);
                }
            }
            Err(e) => {
                error.set(Some(e.to_string()));
                is_busy.set(false);
            }
        }
    });
}

/// A busy flag for actions that may redirect to Checkout, with the
/// bfcache reset built in. `redirect_to_checkout` leaves the flag set
/// after navigating away, so a bfcache Back would otherwise restore the
/// app with the button stuck disabled; creating the flag and the pageshow
/// listener together makes that reset impossible to forget. (Not visible
/// in local dev: trunk's live-reload WebSocket and open DevTools make
/// pages bfcache-ineligible, so Back does a fresh load; production
/// Safari/iOS is the exposed case.)
#[hook]
pub fn use_checkout_busy() -> UseStateHandle<bool> {
    let is_busy = use_state(|| false);
    {
        let is_busy = is_busy.clone();
        use_effect_with((), move |_| {
            let listener = Closure::wrap(Box::new(
                move |event: web_sys::PageTransitionEvent| {
                    if event.persisted() {
                        is_busy.set(false);
                    }
                },
            )
                as Box<dyn FnMut(web_sys::PageTransitionEvent)>);
            let window = web_sys::window().expect("window");
            window
                .add_event_listener_with_callback(
                    "pageshow",
                    listener.as_ref().unchecked_ref(),
                )
                .expect("adding pageshow listener");
            move || {
                let _ = window.remove_event_listener_with_callback(
                    "pageshow",
                    listener.as_ref().unchecked_ref(),
                );
            }
        });
    }
    is_busy
}

/// The outcome a Checkout redirect returned with.
#[derive(Clone, Copy, PartialEq)]
pub enum CheckoutReturn {
    Success,
    Canceled,
}

/// Read the `?{param}=success|canceled` return parameter from a Checkout
/// redirect and strip it from the URL so a reload doesn't repeat the
/// notice. Only `param` is removed; other query params and the fragment
/// are preserved.
#[hook]
pub fn use_checkout_return(param: &'static str) -> Option<CheckoutReturn> {
    *use_state(|| {
        let notice = match crate::utils::url::query_param(param).as_deref() {
            Some("success") => Some(CheckoutReturn::Success),
            Some("canceled") => Some(CheckoutReturn::Canceled),
            _ => None,
        };
        if notice.is_some() {
            crate::utils::url::remove_query_param(param);
        }
        notice
    })
}

/// Renders the post-redirect notice banner with the given copy for a
/// success or canceled return, in the shared neutral card style;
/// renders nothing without a notice.
pub fn checkout_return_banner(
    notice: Option<CheckoutReturn>,
    success: &str,
    canceled: &str,
) -> Html {
    let (text_class, message) = match notice {
        Some(CheckoutReturn::Success) => {
            ("text-neutral-700 dark:text-neutral-300", success)
        }
        Some(CheckoutReturn::Canceled) => {
            ("text-neutral-600 dark:text-neutral-400", canceled)
        }
        None => return html! {},
    };
    html! {
        <p class={format!(
            "mb-4 text-sm {text_class} bg-neutral-100 \
             dark:bg-neutral-700 rounded p-3"
        )}>
            {message.to_string()}
        </p>
    }
}
