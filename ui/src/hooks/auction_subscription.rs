//! Live updates for auction pages via Server-Sent Events.
//!
//! Multiple components on a single page may want to react to the same auction
//! events (e.g., the parent page and a child round component). Rather than
//! opening one `EventSource` per component, this module maintains a process-
//! wide registry keyed by `AuctionId`: any number of `use_auction_subscription`
//! calls for the same auction share one underlying connection. The connection
//! opens on the first registration and closes when the last handler is
//! unregistered.

use std::cell::RefCell;
use std::collections::HashMap;

use payloads::{AuctionEvent, AuctionId};
use wasm_bindgen::prelude::*;
use web_sys::{EventSource, EventSourceInit, MessageEvent};
use yew::prelude::*;

use crate::get_api_client;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Within the 5s grace period after first mount, before EventSource has
    /// fired `open`. Indicator hidden.
    Connecting,
    /// EventSource has fired `open`. Indicator hidden.
    Connected,
    /// Either the grace period elapsed without an `open`, or `EventSource`
    /// fired `error` with `readyState == CLOSED`. Indicator visible.
    Failed,
}

/// Per-event refetch callbacks used internally by the registry. The
/// auction-id and round-id contained in the events are not forwarded — the
/// SSE handler already filters server-side, and the underlying fetch hooks
/// are keyed on the relevant ids, so callers don't need them here.
///
/// Callers don't construct this directly; they pass a slice of
/// `SubscribedEvent` into `use_subscribed_fetch`, and the hook builds the
/// refetches struct internally via `SubscribedEvent::refetches_for`.
#[derive(Clone, PartialEq)]
pub(crate) struct AuctionSubscriptionRefetches {
    pub on_round_created: Callback<()>,
    pub on_round_ended: Callback<()>,
    pub on_auction_ended: Callback<()>,
    pub on_bids_changed: Callback<()>,
}

/// The four routing-only auction event kinds that the SSE stream delivers.
/// Callers pass a slice of these into `use_subscribed_fetch` to declare which
/// events should trigger a refetch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubscribedEvent {
    RoundCreated,
    RoundEnded,
    AuctionEnded,
    BidsChanged,
}

impl SubscribedEvent {
    /// Build an `AuctionSubscriptionRefetches` where each event in `events`
    /// maps to `refetch.clone()` and the rest are no-ops.
    pub(crate) fn refetches_for(
        events: &[SubscribedEvent],
        refetch: Callback<()>,
    ) -> AuctionSubscriptionRefetches {
        let cb = |kind: SubscribedEvent| {
            if events.contains(&kind) {
                refetch.clone()
            } else {
                Callback::noop()
            }
        };
        AuctionSubscriptionRefetches {
            on_round_created: cb(SubscribedEvent::RoundCreated),
            on_round_ended: cb(SubscribedEvent::RoundEnded),
            on_auction_ended: cb(SubscribedEvent::AuctionEnded),
            on_bids_changed: cb(SubscribedEvent::BidsChanged),
        }
    }
}

pub(crate) mod registry {
    use super::*;

    pub(crate) type HandlerToken = u64;

    struct SharedSubscription {
        event_source: EventSource,
        // Closures must outlive the EventSource. Held only to keep them alive
        // for the duration of the subscription; not invoked from Rust.
        _onopen: Closure<dyn FnMut()>,
        _onmessage: Closure<dyn FnMut(MessageEvent)>,
        _onerror: Closure<dyn FnMut(web_sys::Event)>,
        status: ConnectionStatus,
        // Status subscribers (one per call to `use_auction_subscription`).
        // Indexed by token so `unregister` can drop the right one.
        status_subscribers: HashMap<HandlerToken, Callback<ConnectionStatus>>,
        // Per-mount event refetch handlers.
        handlers: HashMap<HandlerToken, AuctionSubscriptionRefetches>,
    }

    thread_local! {
        static REGISTRY: RefCell<HashMap<AuctionId, SharedSubscription>> =
            RefCell::new(HashMap::new());
        static NEXT_TOKEN: RefCell<HandlerToken> = const { RefCell::new(0) };
    }

    fn next_token() -> HandlerToken {
        NEXT_TOKEN.with(|t| {
            let mut t = t.borrow_mut();
            *t += 1;
            *t
        })
    }

    pub(crate) fn register(
        auction_id: AuctionId,
        refetches: AuctionSubscriptionRefetches,
        status_cb: Callback<ConnectionStatus>,
    ) -> HandlerToken {
        let token = next_token();

        REGISTRY.with(|reg| {
            let mut reg = reg.borrow_mut();
            if let Some(sub) = reg.get_mut(&auction_id) {
                // Subscription already open. Push current status to the new
                // subscriber so it sees Connected/Failed immediately rather
                // than waiting through the grace period.
                status_cb.emit(sub.status);
                sub.status_subscribers.insert(token, status_cb);
                sub.handlers.insert(token, refetches);
            } else {
                let sub =
                    open_subscription(auction_id, token, refetches, status_cb);
                reg.insert(auction_id, sub);
            }
        });

        token
    }

    pub(crate) fn unregister(auction_id: AuctionId, token: HandlerToken) {
        REGISTRY.with(|reg| {
            let mut reg = reg.borrow_mut();
            let should_close = if let Some(sub) = reg.get_mut(&auction_id) {
                sub.status_subscribers.remove(&token);
                sub.handlers.remove(&token);
                sub.handlers.is_empty()
            } else {
                false
            };
            if should_close && let Some(sub) = reg.remove(&auction_id) {
                sub.event_source.close();
                // Closures dropped here.
            }
        });
    }

    fn open_subscription(
        auction_id: AuctionId,
        initial_token: HandlerToken,
        initial_refetches: AuctionSubscriptionRefetches,
        initial_status_cb: Callback<ConnectionStatus>,
    ) -> SharedSubscription {
        let url = get_api_client().sse_auction_url(auction_id);

        // `with_credentials` matters only cross-origin (dev). Same-origin
        // (prod) sends cookies regardless. Setting it true is harmless in
        // both cases.
        let init = EventSourceInit::new();
        init.set_with_credentials(true);
        let event_source =
            EventSource::new_with_event_source_init_dict(&url, &init)
                .expect("constructing EventSource");

        let onopen = Closure::wrap(Box::new(move || {
            set_status_for(auction_id, ConnectionStatus::Connected);
        }) as Box<dyn FnMut()>);
        event_source.set_onopen(Some(onopen.as_ref().unchecked_ref()));

        let onmessage = Closure::wrap(Box::new(move |evt: MessageEvent| {
            let data: String = match evt.data().as_string() {
                Some(s) => s,
                None => {
                    tracing::warn!("SSE message data was not a string");
                    return;
                }
            };
            match serde_json::from_str::<AuctionEvent>(&data) {
                Ok(event) => dispatch_event(auction_id, &event),
                Err(e) => {
                    tracing::warn!(?e, ?data, "failed to parse AuctionEvent",)
                }
            }
        })
            as Box<dyn FnMut(MessageEvent)>);
        event_source.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

        let onerror = Closure::wrap(Box::new(move |_evt: web_sys::Event| {
            // EventSource auto-reconnects on transient errors and only
            // hits CLOSED after exhausting its retry attempts. We treat
            // CLOSED as Failed; intermediate errors keep us in our
            // current state (likely still Connecting or Connected).
            let closed = REGISTRY.with(|reg| {
                reg.borrow()
                    .get(&auction_id)
                    .map(|s| {
                        s.event_source.ready_state() == EventSource::CLOSED
                    })
                    .unwrap_or(false)
            });
            if closed {
                set_status_for(auction_id, ConnectionStatus::Failed);
            }
        })
            as Box<dyn FnMut(web_sys::Event)>);
        event_source.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        let mut status_subscribers = HashMap::new();
        status_subscribers.insert(initial_token, initial_status_cb);
        let mut handlers = HashMap::new();
        handlers.insert(initial_token, initial_refetches);

        SharedSubscription {
            event_source,
            _onopen: onopen,
            _onmessage: onmessage,
            _onerror: onerror,
            status: ConnectionStatus::Connecting,
            status_subscribers,
            handlers,
        }
    }

    fn set_status_for(auction_id: AuctionId, new_status: ConnectionStatus) {
        // Snapshot subscribers under the borrow, then emit outside it so
        // callbacks (which may reborrow the registry indirectly) don't panic.
        let subscribers: Vec<Callback<ConnectionStatus>> =
            REGISTRY.with(|reg| {
                let mut reg = reg.borrow_mut();
                let Some(sub) = reg.get_mut(&auction_id) else {
                    return Vec::new();
                };
                sub.status = new_status;
                sub.status_subscribers.values().cloned().collect()
            });
        for cb in subscribers {
            cb.emit(new_status);
        }
    }

    fn dispatch_event(auction_id: AuctionId, event: &AuctionEvent) {
        // Snapshot the relevant handlers under the borrow, then invoke them
        // outside it so handler callbacks can freely refetch (which may
        // indirectly touch this registry on a future tick) without risking a
        // re-entrant borrow.
        let handlers: Vec<AuctionSubscriptionRefetches> =
            REGISTRY.with(|reg| {
                let reg = reg.borrow();
                reg.get(&auction_id)
                    .map(|s| s.handlers.values().cloned().collect())
                    .unwrap_or_default()
            });
        for h in handlers {
            match event {
                AuctionEvent::RoundCreated { .. } => {
                    h.on_round_created.emit(());
                }
                AuctionEvent::RoundEnded { .. } => {
                    h.on_round_ended.emit(());
                }
                AuctionEvent::AuctionEnded { .. } => {
                    h.on_auction_ended.emit(());
                }
                AuctionEvent::BidsChanged { .. } => {
                    h.on_bids_changed.emit(());
                }
            }
        }
    }
}
