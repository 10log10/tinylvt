//! Total convergence for funding-intent rows against live PaymentIntent
//! state.
//!
//! Every (local status × live Stripe status) pair has exactly one owner
//! here: [`decide`] is one exhaustive match over the product, so a new
//! status variant fails compilation instead of silently falling into an
//! unhandled arm, and a pair no handler owns becomes a typed
//! [`ConvergeOutcome::Anomaly`] instead of a wedged row or a bare log
//! line. Transitions carry their side effects (journal entries,
//! notifications, pubsub emits) inside the claim transaction, so a call
//! site can't forget them.
//!
//! Callers hand in the row id and the already-retrieved live intent
//! (None = missing on the account): reconciliation repair
//! (`reconciliation::cross_check_intents`), the webhook poke handlers, and
//! the intent worker's conflict arms. The forward path — executing
//! pending orders, adopting a confirmed authorization — stays with the
//! order/execute machinery; convergence covers observation only, so
//! `Pending × requires_capture` hands off to the existing guarded
//! adoption rather than duplicating it.
//!
//! Ownership of "Waiting" cells (transitional pairs some local actor
//! already owns): the intent worker (capture/cancel arms), the aged-
//! order arm (stale pending rows), and natural hold expiry (failed
//! rows' drain).

use payloads::{FundingIntentId, FundingIntentStatus};
use rust_decimal::Decimal;

use super::locks::{LockWait, TrackedTx};
use super::{StoreError, funding};
use crate::stripe_service::{
    LivePiStatus, RetrievedPaymentIntent, StripeService, is_expiry_reason,
};
use crate::time::TimeSource;
use jiff_sqlx::ToSqlx;

/// The transition table's decision for one (local, live) pair — what
/// [`converge_intent_tx`] applies. Pure data so the table is testable
/// without a database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConvergeAction {
    /// Expected steady state; nothing to do.
    InSync,
    /// Transitional; a local actor owns the next move.
    Waiting,
    /// A pair that should be impossible; report, change nothing.
    Anomaly,
    /// Pending/CheckoutCreated × requires_capture: the confirm landed;
    /// hand off to the existing guarded adoption (caller-side, outside
    /// the claim).
    AdoptPending,
    /// Pending/CheckoutCreated × canceled: the unadopted intent died at
    /// Stripe; terminalize the row and link the PaymentIntent.
    CancelOrder,
    /// Authorized × canceled/missing: a live authorization was lost
    /// out-of-band (dashboard cancel, natural expiry, or the hold died
    /// with a replaced account — `account_gone`); mark terminal and
    /// send the backing-lost notice.
    BackingLost { account_gone: bool },
    /// Capture-owing hold gone (canceled or missing): fail to debt with
    /// the capture-failed notices (`funding::fail_capture_tx`).
    FailCapture,
    /// Money was collected on this intent (live succeeded): mark the
    /// row captured and book the received amount as the treasury→member
    /// issuance. `routine` marks the one pair where collection can be
    /// our own flow (capture_pending — a worker capture whose commit
    /// was lost); everywhere else money moved outside the app's flow.
    BookCapture { routine: bool },
    /// Superseded/release_pending × canceled: the owed release
    /// happened (worker cancel whose commit was lost, or Stripe-side);
    /// mark terminal.
    MarkReleased,
    /// A hold we owed a release died with a replaced account, or was
    /// never reachable again: mark expired.
    MarkExpired,
    /// A live hold on a locally-terminal row (expired rows in the
    /// fallback window, Stripe's lazy auto-cancel): cancel it at Stripe
    /// (caller-side, via `funding::cancel_hold`'s shared key); no local
    /// change.
    CancelLiveHold,
}

/// The transition table: local statuses down, live PaymentIntent state
/// across. One exhaustive match over the enum product — extend it cell
/// by cell when a status is added; never with a catch-all across local
/// statuses.
pub(crate) fn decide(
    local: FundingIntentStatus,
    live: LivePiStatus,
) -> ConvergeAction {
    use ConvergeAction as A;
    use FundingIntentStatus as L;
    use LivePiStatus as P;

    match (local, live) {
        // Pending: the order/execute machinery owns the forward path
        // (execution, adoption) and the aged-order arm owns
        // terminalizing stale orders; convergence observes.
        (L::Pending, P::RequiresCapture) => A::AdoptPending,
        (L::Pending, P::Canceled) => A::CancelOrder,
        (L::Pending, P::Succeeded) => A::Anomaly,
        (L::Pending, P::Missing | P::RequiresPaymentMethod) => A::Waiting,
        (
            L::Pending,
            P::Processing
            | P::RequiresConfirmation
            | P::RequiresAction
            | P::Unknown,
        ) => A::Anomaly,

        // CheckoutCreated: an unsaved-card Checkout session awaiting the
        // member's payment; the session-side lifecycle
        // (`funding_checkout::converge_checkout`, the expiry webhook)
        // owns retiring the row, so PaymentIntent-side observation only
        // adopts a landed confirm or terminalizes a dead one. The
        // member-present states (requires_action is 3DS on Stripe's
        // page; requires_payment_method an in-session decline, retryable
        // there; processing a confirm mid-flight) wait. A succeeded
        // intent here means the session charged instead of holding —
        // loud, not converged.
        (L::CheckoutCreated, P::RequiresCapture) => A::AdoptPending,
        (L::CheckoutCreated, P::Canceled) => A::CancelOrder,
        (L::CheckoutCreated, P::Succeeded) => A::Anomaly,
        (
            L::CheckoutCreated,
            P::Missing
            | P::RequiresPaymentMethod
            | P::RequiresConfirmation
            | P::RequiresAction
            | P::Processing,
        ) => A::Waiting,
        (L::CheckoutCreated, P::Unknown) => A::Anomaly,

        // Authorized: a live hold backing (potential) bids.
        (L::Authorized, P::RequiresCapture) => A::InSync,
        (L::Authorized, P::Canceled) => A::BackingLost {
            account_gone: false,
        },
        (L::Authorized, P::Succeeded) => A::BookCapture { routine: false },
        (L::Authorized, P::Missing) => A::BackingLost { account_gone: true },
        (
            L::Authorized,
            P::RequiresPaymentMethod
            | P::Processing
            | P::RequiresConfirmation
            | P::RequiresAction
            | P::Unknown,
        ) => A::Anomaly,

        // CapturePending: the worker owes Stripe a capture.
        (L::CapturePending, P::RequiresCapture | P::Processing) => A::Waiting,
        (L::CapturePending, P::Succeeded) => A::BookCapture { routine: true },
        (L::CapturePending, P::Canceled | P::Missing) => A::FailCapture,
        (
            L::CapturePending,
            P::RequiresPaymentMethod
            | P::RequiresConfirmation
            | P::RequiresAction
            | P::Unknown,
        ) => A::Anomaly,

        // Superseded / ReleasePending: the worker owes Stripe a cancel.
        (L::Superseded | L::ReleasePending, P::RequiresCapture) => A::Waiting,
        (L::Superseded | L::ReleasePending, P::Canceled) => A::MarkReleased,
        // Money collected on a hold we owed a release (dashboard
        // capture): book it and report loudly; the refund, if any, is
        // the community's dashboard decision.
        (L::Superseded | L::ReleasePending, P::Succeeded) => {
            A::BookCapture { routine: false }
        }
        (L::Superseded | L::ReleasePending, P::Missing) => A::MarkExpired,
        (
            L::Superseded | L::ReleasePending,
            P::RequiresPaymentMethod
            | P::Processing
            | P::RequiresConfirmation
            | P::RequiresAction
            | P::Unknown,
        ) => A::Anomaly,

        // Captured: money booked; missing tolerates a since-replaced
        // account (the ledger is locally coherent).
        (L::Captured, P::Succeeded | P::Missing) => A::InSync,
        (
            L::Captured,
            P::RequiresCapture
            | P::RequiresPaymentMethod
            | P::Processing
            | P::RequiresConfirmation
            | P::RequiresAction
            | P::Canceled
            | P::Unknown,
        ) => A::Anomaly,

        // Canceled: terminal, holds nothing. requires_payment_method is
        // a declined confirm's inert residue (it parks there forever);
        // missing is a since-replaced account. A live hold here has no
        // owner — kill it.
        (L::Canceled, P::Canceled | P::RequiresPaymentMethod | P::Missing) => {
            A::InSync
        }
        (L::Canceled, P::RequiresCapture) => A::CancelLiveHold,
        // The orphan probe's loud case: money moved on a row we
        // canceled. Needs eyes, not silent convergence.
        (L::Canceled, P::Succeeded) => A::Anomaly,
        (
            L::Canceled,
            P::Processing
            | P::RequiresConfirmation
            | P::RequiresAction
            | P::Unknown,
        ) => A::Anomaly,

        // Expired: locally converged with no Stripe call (the shortcut
        // trusts capture_before), so the hold may outlive the row —
        // fallback windows underestimate (~7d real vs 4d assumed) and
        // Stripe's own auto-cancel is lazy. Converge reality now
        // instead of logging drift until it drains.
        (L::Expired, P::Canceled | P::Missing) => A::InSync,
        (L::Expired, P::RequiresCapture) => A::CancelLiveHold,
        (L::Expired, P::Succeeded) => A::BookCapture { routine: false },
        (
            L::Expired,
            P::RequiresPaymentMethod
            | P::Processing
            | P::RequiresConfirmation
            | P::RequiresAction
            | P::Unknown,
        ) => A::Anomaly,

        // Failed: the debt stands; the dead hold drains at natural
        // expiry. A late success is the buried-capture family — the
        // member was charged after the row was failed to debt; book the
        // recovery against it.
        (L::Failed, P::RequiresCapture | P::Processing) => A::Waiting,
        (L::Failed, P::Succeeded) => A::BookCapture { routine: false },
        (L::Failed, P::Canceled | P::Missing) => A::InSync,
        (
            L::Failed,
            P::RequiresPaymentMethod
            | P::RequiresConfirmation
            | P::RequiresAction
            | P::Unknown,
        ) => A::Anomaly,
    }
}

/// What [`converge_intent_tx`] did (or found), for the caller's
/// logging/reporting and post-commit follow-ups.
#[derive(Debug)]
pub(crate) enum ConvergeOutcome {
    InSync,
    Waiting,
    /// A transition was applied in the claim transaction.
    /// `out_of_band_money` marks transitions where money moved outside
    /// the app's flow — reported at error level.
    Converged {
        note: &'static str,
        out_of_band_money: bool,
    },
    /// A pair that should be impossible; nothing was changed.
    Anomaly {
        detail: String,
    },
    /// The live hold should not exist; the caller cancels it at Stripe
    /// after committing (via `funding::cancel_hold`'s shared key).
    CancelLiveHold {
        payment_intent_id: String,
    },
    /// A pending row whose confirm landed; the caller runs the guarded
    /// adoption (lock-free, outside the claim).
    AdoptPending {
        payment_intent_id: String,
        currency_minor_units: i16,
    },
}

/// The intent row plus display/booking context, read fresh under the
/// claim.
#[derive(sqlx::FromRow)]
struct IntentRowCtx {
    status: FundingIntentStatus,
    payment_intent_id: Option<String>,
    capture_amount: Option<Decimal>,
    community_id: payloads::CommunityId,
    auction_id: payloads::AuctionId,
    user_id: payloads::UserId,
    currency_minor_units: i16,
}

/// Converge one intent row against its PaymentIntent's live state
/// (`live` None = missing on the community's account): re-read the row
/// under the claim, take the table's decision, and apply it with its
/// side effects in the claim transaction.
///
/// Guarded transitions that find the row already moved return `Waiting`
/// (another actor won). Acting on a possibly-stale `live` is safe: the
/// live states acted on are Stripe-terminal (`succeeded`, `canceled`)
/// or re-verified by the follow-up Stripe call's own conflict handling
/// (`CancelLiveHold`).
///
/// Lock contract: expects the pair lock (key-checked); the money-
/// booking transition acquires the treasury and member account locks
/// via the issuance entry, and `FailCapture` follows
/// `fail_capture_tx`'s contract.
pub(crate) async fn converge_intent_tx(
    intent_id: &FundingIntentId,
    live: Option<&RetrievedPaymentIntent>,
    time_source: &TimeSource,
    locks: &mut TrackedTx<'_, '_>,
) -> Result<ConvergeOutcome, StoreError> {
    let row: Option<IntentRowCtx> = sqlx::query_as(
        "SELECT fi.status, fi.payment_intent_id, fi.capture_amount, \
                s.community_id, fi.auction_id, fi.user_id, \
                c.currency_minor_units \
         FROM funding_intents fi \
         JOIN auctions a ON fi.auction_id = a.id \
         JOIN sites s ON a.site_id = s.id \
         JOIN communities c ON s.community_id = c.id \
         WHERE fi.id = $1",
    )
    .bind(intent_id)
    .fetch_optional(&mut **locks.tx())
    .await?;
    let Some(row) = row else {
        // Cascade-deleted since selection; nothing local to converge.
        return Ok(ConvergeOutcome::Waiting);
    };
    locks.expect_pair(&row.auction_id, &row.user_id)?;
    let Some(payment_intent_id) = row.payment_intent_id.clone() else {
        // No Stripe linkage (an order canceled before any create); the
        // orphan sweep owns these.
        return Ok(ConvergeOutcome::Waiting);
    };

    let live_status = live.map(|l| l.status).unwrap_or(LivePiStatus::Missing);
    let action = decide(row.status, live_status);
    let anomaly = |detail: String| ConvergeOutcome::Anomaly { detail };
    let now = time_source.now().to_sqlx();

    Ok(match action {
        ConvergeAction::InSync => ConvergeOutcome::InSync,
        ConvergeAction::Waiting => ConvergeOutcome::Waiting,
        ConvergeAction::Anomaly => anomaly(format!(
            "local {} × live {}: no legal transition",
            row.status, live_status
        )),
        ConvergeAction::AdoptPending => ConvergeOutcome::AdoptPending {
            payment_intent_id,
            currency_minor_units: row.currency_minor_units,
        },
        ConvergeAction::CancelLiveHold => {
            ConvergeOutcome::CancelLiveHold { payment_intent_id }
        }
        ConvergeAction::CancelOrder => {
            let updated = sqlx::query(
                "UPDATE funding_intents \
                 SET status = 'canceled', \
                     payment_intent_id = COALESCE(payment_intent_id, $1), \
                     updated_at = $2 \
                 WHERE id = $3 AND status IN ('pending', 'checkout_created')",
            )
            .bind(&payment_intent_id)
            .bind(now)
            .bind(intent_id)
            .execute(&mut **locks.tx())
            .await?;
            if updated.rows_affected() == 0 {
                return Ok(ConvergeOutcome::Waiting);
            }
            tracing::info!(
                %intent_id,
                payment_intent_id,
                "pending order's intent died at Stripe; order canceled"
            );
            ConvergeOutcome::Converged {
                note: "canceled a pending order whose intent died at Stripe",
                out_of_band_money: false,
            }
        }
        ConvergeAction::BackingLost { account_gone } => {
            let expired = account_gone
                || live.is_some_and(|l| {
                    is_expiry_reason(l.cancellation_reason.as_deref())
                });
            let terminal = if expired {
                FundingIntentStatus::Expired
            } else {
                FundingIntentStatus::Canceled
            };
            if !funding::mark_intent_canceled_tx(
                intent_id,
                FundingIntentStatus::Authorized,
                terminal,
                time_source,
                locks.tx(),
            )
            .await?
            {
                return Ok(ConvergeOutcome::Waiting);
            }
            funding::notify_backing_lost_tx(
                intent_id,
                expired,
                time_source,
                locks.tx(),
            )
            .await?;
            funding::emit_funding_changed(intent_id, locks.tx()).await?;
            tracing::warn!(
                %intent_id,
                payment_intent_id,
                expired,
                account_gone,
                "live authorization lost out-of-band; converged"
            );
            ConvergeOutcome::Converged {
                note: "marked a lost live authorization terminal",
                out_of_band_money: false,
            }
        }
        ConvergeAction::FailCapture => {
            if !funding::fail_capture_tx(intent_id, time_source, locks).await? {
                return Ok(ConvergeOutcome::Waiting);
            }
            tracing::error!(
                %intent_id,
                payment_intent_id,
                "capture-owing hold gone at Stripe; the uncollected \
                 amount remains as member debt"
            );
            ConvergeOutcome::Converged {
                note: "failed an uncollectible capture to member debt",
                out_of_band_money: false,
            }
        }
        ConvergeAction::MarkReleased | ConvergeAction::MarkExpired => {
            let expired = action == ConvergeAction::MarkExpired
                || live.is_some_and(|l| {
                    is_expiry_reason(l.cancellation_reason.as_deref())
                });
            let terminal = if expired {
                FundingIntentStatus::Expired
            } else {
                FundingIntentStatus::Canceled
            };
            if !funding::mark_intent_canceled_tx(
                intent_id,
                row.status,
                terminal,
                time_source,
                locks.tx(),
            )
            .await?
            {
                return Ok(ConvergeOutcome::Waiting);
            }
            funding::emit_funding_changed(intent_id, locks.tx()).await?;
            tracing::info!(
                %intent_id,
                payment_intent_id,
                prior_status = %row.status,
                %terminal,
                "owed release already happened at Stripe; converged"
            );
            ConvergeOutcome::Converged {
                note: "marked an owed release terminal",
                out_of_band_money: false,
            }
        }
        ConvergeAction::BookCapture { routine } => {
            let Some(live) = live else {
                // decide() never returns BookCapture for Missing.
                return Ok(anomaly(format!(
                    "local {} × live missing decided BookCapture",
                    row.status
                )));
            };
            let received_minor = live.amount_received_minor;
            if received_minor <= 0 {
                return Ok(anomaly(format!(
                    "local {} × live succeeded with amount_received \
                     {received_minor}",
                    row.status
                )));
            }
            let received = payloads::from_minor_units(
                received_minor,
                row.currency_minor_units,
            );
            let updated = sqlx::query(
                "UPDATE funding_intents \
                 SET status = 'captured', is_active = FALSE, \
                     capture_amount = $1, worker_failure_count = 0, \
                     worker_last_failed_at = NULL, updated_at = $2 \
                 WHERE id = $3 AND status = $4",
            )
            .bind(received)
            .bind(now)
            .bind(intent_id)
            .bind(row.status)
            .execute(&mut **locks.tx())
            .await?;
            if updated.rows_affected() == 0 {
                return Ok(ConvergeOutcome::Waiting);
            }
            // Book the amount the card actually paid as the standard
            // treasury→member issuance credit (the shared triple with
            // the capture worker; the entry's idempotency key makes the
            // two paths once-only per intent). Any excess over what
            // settlement sized stays as member balance.
            funding::book_capture_tx(
                intent_id,
                &row.auction_id,
                &row.community_id,
                &row.user_id,
                received,
                &payment_intent_id,
                time_source,
                locks,
            )
            .await?;
            // Routine only when collection matches our own sized
            // capture — a lost commit's replay. A different amount, or
            // any other prior status, means an out-of-band capture
            // (e.g. the dashboard's Capture button).
            let out_of_band = !routine || row.capture_amount != Some(received);
            if out_of_band {
                tracing::warn!(
                    %intent_id,
                    payment_intent_id,
                    prior_status = %row.status,
                    %received,
                    "money collected outside the app's flow; booked as \
                     captured"
                );
            } else {
                tracing::info!(
                    %intent_id,
                    payment_intent_id,
                    %received,
                    "capture succeeded at Stripe but was never \
                     committed; booked"
                );
            }
            ConvergeOutcome::Converged {
                note: "booked a collected capture",
                out_of_band_money: out_of_band,
            }
        }
    })
}

/// A standalone convergence pass over one intent row, as reported to
/// the caller (reconciliation repair, webhook pokes).
#[derive(Debug)]
pub(crate) enum ConvergeReport {
    /// Another claimant owns the (auction, user) pair; skipped. The
    /// next poke or reconciliation pass retries.
    Busy,
    InSync,
    Waiting,
    Converged {
        note: &'static str,
        out_of_band_money: bool,
    },
    Anomaly {
        detail: String,
    },
}

/// The pool a standalone converge pass draws its connections from —
/// each caller passes its own context's pool via the matching
/// constructor. Either budget safely absorbs the work: the pair claim
/// is short pure-DB work and the follow-up Stripe calls run with no
/// transaction open, so no connection is ever pinned across the
/// network.
#[derive(Clone, Copy)]
pub(crate) struct ConvergePool<'a>(&'a sqlx::PgPool);

impl<'a> ConvergePool<'a> {
    /// Reconciliation repair and the intent worker: the worker pool.
    pub(crate) fn worker(pool: &'a crate::WorkerPool) -> Self {
        Self(&pool.0)
    }

    /// Webhook pokes: the shared API pool.
    pub(crate) fn api(pool: &'a sqlx::PgPool) -> Self {
        Self(pool)
    }
}

/// Converge one intent row under its own pair-lock claim, then execute
/// any Stripe-side follow-up (canceling a stray live hold) or adoption
/// hand-off after the claim commits. `live` is the already-retrieved
/// PaymentIntent (None = missing on the account); see
/// `converge_intent_tx` for why acting on a possibly-stale retrieve is
/// safe.
///
/// Lock contract: acquires the pair try-lock; the Stripe cancel runs
/// after commit (no locks held), and adoption manages its own locking
/// (`funding::adopt_confirmed_intent`).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn converge_intent(
    intent_id: &FundingIntentId,
    auction_id: &payloads::AuctionId,
    user_id: &payloads::UserId,
    account_id: &str,
    live: Option<&RetrievedPaymentIntent>,
    pool: ConvergePool<'_>,
    time_source: &TimeSource,
    stripe_service: &StripeService,
) -> Result<ConvergeReport, StoreError> {
    let mut locks = TrackedTx::begin(pool.0).await?;
    if !locks
        .acquire_pair(auction_id, user_id, LockWait::Try)
        .await?
    {
        return Ok(ConvergeReport::Busy);
    }
    let outcome =
        converge_intent_tx(intent_id, live, time_source, &mut locks).await?;
    locks.commit().await?;
    finish_converge(
        outcome,
        intent_id,
        account_id,
        pool,
        time_source,
        stripe_service,
    )
    .await
}

/// Run a committed converge outcome's post-commit follow-ups — the
/// Stripe cancel of a stray live hold, the adoption hand-off — and map
/// it to the caller-facing report. Split from [`converge_intent`] so
/// the intent worker, which converges inside its own already-held
/// claim, can run the same follow-ups after committing (no locks are
/// held at the Stripe calls either way).
pub(crate) async fn finish_converge(
    outcome: ConvergeOutcome,
    intent_id: &FundingIntentId,
    account_id: &str,
    pool: ConvergePool<'_>,
    time_source: &TimeSource,
    stripe_service: &StripeService,
) -> Result<ConvergeReport, StoreError> {
    Ok(match outcome {
        ConvergeOutcome::InSync => ConvergeReport::InSync,
        ConvergeOutcome::Waiting => ConvergeReport::Waiting,
        ConvergeOutcome::Converged {
            note,
            out_of_band_money,
        } => ConvergeReport::Converged {
            note,
            out_of_band_money,
        },
        ConvergeOutcome::Anomaly { detail } => {
            ConvergeReport::Anomaly { detail }
        }
        ConvergeOutcome::CancelLiveHold { payment_intent_id } => {
            funding::cancel_hold(
                account_id,
                &payment_intent_id,
                intent_id,
                stripe_service,
            )
            .await
            .map_err(|e| StoreError::StripeError(e.to_string()))?;
            tracing::info!(
                %intent_id,
                payment_intent_id,
                "canceled a live hold outliving its terminal row"
            );
            ConvergeReport::Converged {
                note: "canceled a live hold outliving its terminal row",
                out_of_band_money: false,
            }
        }
        ConvergeOutcome::AdoptPending {
            payment_intent_id,
            currency_minor_units,
        } => {
            funding::adopt_confirmed_intent(
                intent_id,
                &payment_intent_id,
                account_id,
                currency_minor_units,
                stripe_service,
                time_source,
                pool.0,
            )
            .await?;
            ConvergeReport::Converged {
                note: "poked adoption of a confirmed authorization",
                out_of_band_money: false,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every cell of the (local × live) product returns its intended
    /// classification — the table written down as a test, so a changed
    /// cell is a conscious edit here too. Compile-time exhaustiveness
    /// (no catch-alls in `decide`) is what guarantees new statuses
    /// can't fall through silently.
    #[test]
    fn transition_table() {
        use ConvergeAction as A;
        use FundingIntentStatus as L;
        use LivePiStatus as P;

        const LOCALS: [FundingIntentStatus; 10] = [
            L::Pending,
            L::CheckoutCreated,
            L::Authorized,
            L::CapturePending,
            L::Captured,
            L::ReleasePending,
            L::Superseded,
            L::Canceled,
            L::Expired,
            L::Failed,
        ];
        const LIVES: [LivePiStatus; 9] = [
            P::RequiresPaymentMethod,
            P::RequiresConfirmation,
            P::RequiresAction,
            P::Processing,
            P::RequiresCapture,
            P::Succeeded,
            P::Canceled,
            P::Missing,
            P::Unknown,
        ];

        let expected = |local: L, live: P| -> A {
            match (local, live) {
                (L::Pending, P::RequiresCapture) => A::AdoptPending,
                (L::Pending, P::Canceled) => A::CancelOrder,
                (L::Pending, P::Missing | P::RequiresPaymentMethod) => {
                    A::Waiting
                }

                (L::CheckoutCreated, P::RequiresCapture) => A::AdoptPending,
                (L::CheckoutCreated, P::Canceled) => A::CancelOrder,
                (
                    L::CheckoutCreated,
                    P::Missing
                    | P::RequiresPaymentMethod
                    | P::RequiresConfirmation
                    | P::RequiresAction
                    | P::Processing,
                ) => A::Waiting,

                (L::Authorized, P::RequiresCapture) => A::InSync,
                (L::Authorized, P::Canceled) => A::BackingLost {
                    account_gone: false,
                },
                (L::Authorized, P::Missing) => {
                    A::BackingLost { account_gone: true }
                }
                (L::Authorized, P::Succeeded) => {
                    A::BookCapture { routine: false }
                }

                (L::CapturePending, P::RequiresCapture | P::Processing) => {
                    A::Waiting
                }
                (L::CapturePending, P::Succeeded) => {
                    A::BookCapture { routine: true }
                }
                (L::CapturePending, P::Canceled | P::Missing) => A::FailCapture,

                (L::Superseded | L::ReleasePending, P::RequiresCapture) => {
                    A::Waiting
                }
                (L::Superseded | L::ReleasePending, P::Canceled) => {
                    A::MarkReleased
                }
                (L::Superseded | L::ReleasePending, P::Succeeded) => {
                    A::BookCapture { routine: false }
                }
                (L::Superseded | L::ReleasePending, P::Missing) => {
                    A::MarkExpired
                }

                (L::Captured, P::Succeeded | P::Missing) => A::InSync,

                (
                    L::Canceled,
                    P::Canceled | P::RequiresPaymentMethod | P::Missing,
                ) => A::InSync,
                (L::Canceled, P::RequiresCapture) => A::CancelLiveHold,

                (L::Expired, P::Canceled | P::Missing) => A::InSync,
                (L::Expired, P::RequiresCapture) => A::CancelLiveHold,
                (L::Expired, P::Succeeded) => A::BookCapture { routine: false },

                (L::Failed, P::RequiresCapture | P::Processing) => A::Waiting,
                (L::Failed, P::Succeeded) => A::BookCapture { routine: false },
                (L::Failed, P::Canceled | P::Missing) => A::InSync,

                _ => A::Anomaly,
            }
        };

        for local in LOCALS {
            for live in LIVES {
                assert_eq!(
                    decide(local, live),
                    expected(local, live),
                    "cell ({local}, {live})"
                );
            }
        }
    }
}
