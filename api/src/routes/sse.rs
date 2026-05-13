//! Server-Sent Events for live auction updates.
//!
//! Clients open one stream per auction. Events are filtered server-side and
//! delivered as `data: <json>\n\n` frames. A heartbeat (`: heartbeat\n\n`)
//! every 20 seconds keeps reverse proxies from closing idle connections.

use std::time::Duration;

use actix_identity::Identity;
use actix_web::{HttpResponse, get, web};
use payloads::{AuctionEvent, AuctionId, UserId};
use sqlx::PgPool;
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::ReceiverStream;

use crate::pubsub::PubSub;
use crate::routes::{APIError, get_user_id};
use crate::store;

const HEARTBEAT: Duration = Duration::from_secs(20);
/// Bound on the per-stream forwarder buffer. Events are infrequent so a small
/// buffer is fine; if the client backpressures past this, the stream ends and
/// the client reconnects.
const FORWARDER_BUFFER: usize = 16;

#[get("/sse/auctions/{auction_id}")]
pub async fn sse_auction(
    user: Identity,
    path: web::Path<AuctionId>,
    pool: web::Data<PgPool>,
    bus: web::Data<PubSub>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let auction_id = path.into_inner();
    // Same gate as POST /auction.
    store::auction::read_auction(&auction_id, &user_id, &pool).await?;

    let rx = bus.subscribe();
    let stream = build_event_stream(rx, auction_id, user_id);

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("cache-control", "no-cache, no-transform"))
        .insert_header(("x-accel-buffering", "no"))
        .streaming(stream))
}

/// Build the SSE body stream. Spawns a forwarder that merges broadcast events
/// with a heartbeat ticker and writes encoded SSE frames to an mpsc channel.
/// The returned stream is driven by actix-web; when the client disconnects,
/// the channel send fails and the forwarder exits.
fn build_event_stream(
    mut rx: broadcast::Receiver<AuctionEvent>,
    auction_id: AuctionId,
    user_id: UserId,
) -> ReceiverStream<Result<actix_web::web::Bytes, std::io::Error>> {
    let (tx, body_rx) = mpsc::channel::<
        Result<actix_web::web::Bytes, std::io::Error>,
    >(FORWARDER_BUFFER);

    tokio::spawn(async move {
        // `tokio::time::interval` resolves its first tick immediately, so
        // entering the select loop fires a heartbeat right away. This is
        // intentional: it flushes the response headers and lets the
        // client's `onopen` fire immediately. Without an initial byte,
        // some HTTP layers buffer the response until the first chunk
        // arrives, which would mean up to `HEARTBEAT` of perceived
        // connection latency on an idle stream.
        let mut heartbeat = tokio::time::interval(HEARTBEAT);

        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Ok(event) => {
                            if !event_matches(&event, auction_id, user_id) {
                                continue;
                            }
                            let frame = match encode_event(&event) {
                                Ok(f) => f,
                                Err(e) => {
                                    tracing::warn!(
                                        error = ?e,
                                        "failed to encode sse event",
                                    );
                                    continue;
                                }
                            };
                            if tx.send(Ok(frame)).await.is_err() {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            // Slow consumer fell behind. Close the stream;
                            // the client will reconnect via EventSource and
                            // refetch state.
                            tracing::warn!(
                                skipped = n,
                                "sse subscriber lagged; closing stream",
                            );
                            break;
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = heartbeat.tick() => {
                    let frame = actix_web::web::Bytes::from_static(
                        b": heartbeat\n\n",
                    );
                    if tx.send(Ok(frame)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    ReceiverStream::new(body_rx)
}

fn event_matches(
    event: &AuctionEvent,
    auction_id: AuctionId,
    user_id: UserId,
) -> bool {
    match event {
        AuctionEvent::RoundCreated { auction_id: a, .. }
        | AuctionEvent::RoundEnded { auction_id: a, .. }
        | AuctionEvent::AuctionEnded { auction_id: a } => *a == auction_id,
        AuctionEvent::BidsChanged {
            auction_id: a,
            user_id: u,
            ..
        } => *a == auction_id && *u == user_id,
    }
}

/// Encode an `AuctionEvent` as an SSE `data:` frame.
///
/// We don't fold heartbeats into the JSON payload (e.g. as an extra enum
/// variant) because SSE has its own wire format that `EventSource` parses
/// directly: `data:` frames fire `onmessage` on the client and count as real
/// events for backpressure / lag accounting, while `:` comment lines (used
/// for heartbeats) are silently consumed by the browser without waking
/// application code. Keeping that distinction means heartbeats stay invisible
/// to the JS layer instead of becoming events the client has to filter out.
fn encode_event(
    event: &AuctionEvent,
) -> Result<actix_web::web::Bytes, serde_json::Error> {
    let json = serde_json::to_string(event)?;
    let mut frame = String::with_capacity(json.len() + 8);
    frame.push_str("data: ");
    frame.push_str(&json);
    frame.push_str("\n\n");
    Ok(actix_web::web::Bytes::from(frame))
}
