//! Pub/sub for live auction state changes.
//!
//! Producers emit events by calling `pg_notify('auction_changes', $1)` inside
//! the same transaction as the data change; Postgres queues the notification
//! until commit and then delivers it. A single `PgListener` task per process
//! receives those notifications and forwards them onto an in-process
//! `tokio::sync::broadcast` channel that SSE handlers subscribe to.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::Context;
use payloads::AuctionEvent;
use sqlx::postgres::PgListener;
use tokio::sync::broadcast;

/// Postgres NOTIFY channel name.
pub const NOTIFY_CHANNEL: &str = "auction_changes";

/// In-process broadcast capacity. Events are infrequent (round transitions are
/// minutes apart), so a small buffer is plenty; slow consumers will lag and
/// disconnect, then reconnect and refetch.
const BROADCAST_CAPACITY: usize = 64;

/// Cloneable handle to the in-process broadcast channel. Cheap to clone.
///
/// The inner `Sender` is swappable: the listener calls `reset()` whenever
/// notifications may have been missed — once on every successful listener
/// (re)start (just after `LISTEN` returns) and once per transparent sqlx
/// reconnect. Existing receivers see `RecvError::Closed`, which the SSE
/// handler treats as the stream ending — the client then reconnects and
/// refetches state, closing the gap.
#[derive(Clone)]
pub struct PubSub {
    tx: Arc<RwLock<broadcast::Sender<AuctionEvent>>>,
}

impl PubSub {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            tx: Arc::new(RwLock::new(tx)),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AuctionEvent> {
        self.tx.read().unwrap().subscribe()
    }

    /// Forward an event to all current subscribers.
    ///
    /// `broadcast::Sender::send` only fails when the receiver count is zero;
    /// for our use that's a routine state (no UI clients are watching any
    /// auction right now), so we deliberately drop the error rather than
    /// log on every idle-period notification.
    fn send(&self, event: AuctionEvent) {
        let _no_subscribers = self.tx.read().unwrap().send(event);
    }

    /// Replace the underlying broadcast sender with a fresh one. Existing
    /// receivers observe `RecvError::Closed` on their next `recv()`. Called
    /// at every point where notifications may have been missed: on each
    /// successful listener (re)start, and on each transparent sqlx reconnect.
    /// Subscribers then reconnect and refetch.
    ///
    /// Public so integration tests can simulate a missed-event reset and
    /// observe the SSE-handler-side behavior (stream close → client refetch).
    /// Production callers should not invoke this — the listener does so at
    /// the appropriate points.
    pub fn reset(&self) {
        let (new_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        *self.tx.write().unwrap() = new_tx;
    }
}

impl Default for PubSub {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the `PgListener` forever, forwarding parsed events to `bus`.
///
/// Two kinds of gap can drop notifications between Postgres and the bus:
///
/// 1. The listener's TCP connection drops mid-recv and is successfully
///    re-established. `try_recv` returns `Ok(None)` only after
///    `PgListener::connect_if_needed` has already succeeded internally (default
///    `eager_reconnect = true`); a failed reconnect would surface as `Err`
///    instead, falling into case 2. So when we observe `Ok(None)`, the listener
///    is healthy and we can safely call `bus.reset()`. New subscribers that
///    attach after the reset see the fresh sender and any subsequent
///    `pg_notify`s; established subscribers see `Closed` and reconnect on the
///    client.
/// 2. A hard error from `connect()`, `listen()`, or `try_recv()` (including a
///    failed eager reconnect inside `try_recv`). The next `listen_loop`
///    iteration resets the bus *after* its `connect()` + `listen()` complete
///    but before entering the recv loop. By then the new listener is live, so
///    any `pg_notify` from that moment forward will be forwarded to the fresh
///    sender — there's no window between reset and listener-online for events
///    to disappear into.
///
/// In both cases, booted subscribers reconnect their `EventSource`, and the
/// SSE handler's per-stream open-time refetch repopulates state — capturing
/// anything that was emitted during the gap.
pub async fn run_listener(db_url: String, bus: PubSub) {
    const MIN_BACKOFF: Duration = Duration::from_secs(1);
    const MAX_BACKOFF: Duration = Duration::from_secs(30);
    /// Once the backoff has saturated at `MAX_BACKOFF`, log at error level
    /// (instead of warn) after this many additional consecutive failures, so
    /// monitoring can page on a sustained outage rather than a transient blip.
    const CAP_BACKOFFS_BEFORE_ERROR: u32 = 10;

    /// If a listen run lasted at least this long before failing, treat it as
    /// a successful run and reset the backoff so a later transient failure
    /// doesn't inherit the previous failure window's cap.
    const RESET_AFTER: Duration = Duration::from_secs(60);

    let mut backoff = MIN_BACKOFF;
    let mut consecutive_cap_failures: u32 = 0;

    loop {
        let started_at = tokio::time::Instant::now();
        // listen_loop never returns Ok; it only exits via Err.
        let e = listen_loop(&db_url, &bus).await.unwrap_err();
        if started_at.elapsed() >= RESET_AFTER {
            backoff = MIN_BACKOFF;
            consecutive_cap_failures = 0;
        }
        if backoff >= MAX_BACKOFF
            && consecutive_cap_failures >= CAP_BACKOFFS_BEFORE_ERROR
        {
            tracing::error!(
                error = ?e,
                backoff_secs = backoff.as_secs(),
                "pubsub listener has been failing repeatedly",
            );
        } else {
            tracing::warn!(
                error = ?e,
                backoff_secs = backoff.as_secs(),
                "pubsub listener errored; retrying",
            );
        }
        tokio::time::sleep(backoff).await;
        if backoff >= MAX_BACKOFF {
            consecutive_cap_failures =
                consecutive_cap_failures.saturating_add(1);
        } else {
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
    }
}

/// Connect, subscribe, and forward notifications until an error is hit.
/// Never returns Ok — only exits via Err.
///
/// We use `try_recv` rather than `recv` so connection drops surface as
/// `Ok(None)` (after sqlx has already reconnected, since `eager_reconnect` is
/// on by default) rather than being absorbed transparently inside `recv()`.
/// Without that visibility we'd miss notifications emitted during the gap and
/// clients would see a healthy SSE stream with a hole in the middle.
///
/// `bus.reset()` is called once on entry, after `connect()` + `listen()`
/// succeed but before the recv loop. This boots any subscribers from the
/// previous listen_loop iteration (or from before the process took over the
/// bus) at the moment the new listener is guaranteed to be observing future
/// notifications, with no race window between reset and listener-online.
async fn listen_loop(
    db_url: &str,
    bus: &PubSub,
) -> anyhow::Result<std::convert::Infallible> {
    let mut listener = PgListener::connect(db_url)
        .await
        .context("connecting pubsub listener")?;
    listener
        .listen(NOTIFY_CHANNEL)
        .await
        .context("subscribing to pubsub channel")?;
    bus.reset();
    tracing::info!(channel = NOTIFY_CHANNEL, "pubsub listener ready");

    loop {
        match listener.try_recv().await {
            Ok(Some(notification)) => {
                match serde_json::from_str::<AuctionEvent>(
                    notification.payload(),
                ) {
                    Ok(event) => bus.send(event),
                    Err(e) => {
                        tracing::warn!(
                            error = ?e,
                            payload = notification.payload(),
                            "ignoring malformed pubsub payload",
                        );
                    }
                }
            }
            Ok(None) => {
                // Connection dropped and was transparently reconnected by
                // sqlx before this call returned (eager_reconnect = true).
                // Notifications emitted while the connection was down are
                // gone, so boot subscribers; clients reconnect their
                // EventSource and the open-time refetch repopulates state.
                // The reconnect-then-reset ordering means new subscribers
                // attaching after the reset see the fresh sender and the
                // healthy listener, with no race window.
                tracing::warn!("pubsub listener reconnected; resetting bus");
                bus.reset();
            }
            Err(e) => {
                return Err(anyhow::Error::from(e)
                    .context("receiving pubsub notification"));
            }
        }
    }
}

/// Emit an event via `pg_notify` on the given transaction. The notification is
/// queued by Postgres and delivered to listeners only after the transaction
/// commits — if the tx rolls back, no notification is sent.
pub async fn emit(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event: &AuctionEvent,
) -> anyhow::Result<()> {
    let payload =
        serde_json::to_string(event).context("serializing pubsub event")?;
    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(NOTIFY_CHANNEL)
        .bind(&payload)
        .execute(&mut **tx)
        .await
        .context("calling pg_notify")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use payloads::{AuctionId, AuctionRoundId};
    use uuid::Uuid;

    #[tokio::test]
    async fn fan_out_to_multiple_subscribers() {
        let bus = PubSub::new();
        let mut a = bus.subscribe();
        let mut b = bus.subscribe();

        let event = AuctionEvent::RoundCreated {
            auction_id: AuctionId(Uuid::new_v4()),
            round_id: AuctionRoundId(Uuid::new_v4()),
        };
        bus.send(event.clone());

        assert_eq!(a.recv().await.unwrap(), event);
        assert_eq!(b.recv().await.unwrap(), event);
    }

    #[tokio::test]
    async fn reset_closes_existing_subscribers() {
        let bus = PubSub::new();
        let mut rx = bus.subscribe();

        bus.reset();

        // Existing receiver sees the channel as closed; new subscribers get
        // the fresh sender.
        assert!(matches!(
            rx.recv().await,
            Err(broadcast::error::RecvError::Closed),
        ));

        let mut rx2 = bus.subscribe();
        let event = AuctionEvent::RoundCreated {
            auction_id: AuctionId(Uuid::new_v4()),
            round_id: AuctionRoundId(Uuid::new_v4()),
        };
        bus.send(event.clone());
        assert_eq!(rx2.recv().await.unwrap(), event);
    }
}
