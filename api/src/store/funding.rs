//! Per-auction funding state for the stripe-backed backed_credits mode.
//!
//! There is no stored allocation: how much balance backs a member's bids is
//! derived. Per live auction, a member's commitment (what their standing and
//! pending bids oblige them to pay) splits by identity into an auth commitment,
//! `min(commitment, live authorized_amount)`, covered by the card hold, and a
//! balance commitment, `max(0, commitment − live authorized_amount)`, which
//! balance must back; any auth excess is headroom, not commitment. `unallocated
//! = balance + Σ capture_pending amounts − Σ balance commitments` is what new
//! commitments or outflows may take (the pending term counts committed
//! settlement captures as succeeded, uniformly, so a member keeps operating in
//! the debit→credit-back seconds after a conclusion). Backing is auth-first:
//! the card-covered portion of a commitment frees balance immediately for other
//! auctions and outflows, and settlement sizes its capture from whatever
//! balance then remains (`settle_auction_funding_tx`) — captures grow when the
//! freed balance left in the meantime, which is the intended meaning of a hold.
//!
//! Both quantities range over live auctions only (`end_at IS NULL`):
//! commitments end when their auction does, so conclusion, cancellation, and a
//! bid claim racing the conclusion transaction all need no release bookkeeping.
//! Shortfall — balance commitments exceeding what balance plus pending can
//! back, after an out-of-band auth loss or a terminal capture failure — is a
//! tolerated self-healing state: commitments stand, new bids and outflows gate
//! against the diminished backing, and re-auth, repayment, or settlement's debt
//! booking closes the gap (reconciliation reports the volume).
//!
//! # Concurrency
//!
//! The authoritative locking model — holder criteria per lock, per-path
//! acquisition sequences, and the pairwise deadlock-freedom arguments — is
//! `api/CONCURRENCY.md`; keep it current when touching any locking path. Its
//! runtime enforcement point is `store::locks` (`TrackedTx`), which records
//! every coordination-lock acquisition and checks the composition rules.
//! Functions on those paths declare their expectations and acquisitions in a
//! `Lock contract:` docstring line (grep for it), backed by `expect_*`
//! assertions where the contract is an expectation. Two properties matter
//! constantly when reading this module: the member's account row lock is the
//! funding mutex — every gate reads balance from the locked-row snapshot and
//! derives claims inside that transaction — and webhook handlers take no pair
//! locks (our own confirm fires events mid-claim, so a lock-taking handler
//! would deadlock with the call that triggered it — they converge with
//! status-guarded single-row UPDATEs instead; adoption's activation acquires
//! only the processing lock, inside `activate_intent_tx`).
//!
//! One organizational exception: store functions normally leave transaction
//! boundaries to the flow modules (`funding_flow` owns every Stripe-sandwiching
//! sequence), but `handle_intent_event`, the webhook applier, opens its own
//! short transactions here — it is pure state-machine convergence whose one
//! Stripe call, adoption's retrieve, happens lock-free before its
//! transaction, and it belongs beside the transitions it applies.
//!
//! # Intent state machine
//!
//! A `funding_intents` row mirrors one manual-capture Stripe PaymentIntent.
//! `status` is the lifecycle; `is_active` marks the funding row's single
//! current lineage head (at most one active row per funding row, by partial
//! unique index). Every transition is a status-guarded UPDATE — the WHERE
//! clause re-checks the expected prior status, so racing writers converge
//! instead of erroring. All transition SQL lives in this module: paths
//! elsewhere (the execute claim, the intent worker) call the transition
//! functions here rather than writing `funding_intents` directly. The
//! full edge set and its owners:
//!
//! ```text
//! pending ─┬─> authorized       execute claim finalize (activate_intent_tx)
//!          │                    or webhook adoption (handle_intent_event)
//!          └─> canceled         confirm refused (record_intent_decline);
//!                               order staled by a larger live hold
//!                               (execute_auth_order); payment_failed
//!                               webhook (handle_intent_event); aged
//!                               unexecuted order (funding_flow::
//!                               cancel_aged_pending_order); create
//!                               unexecutable or intent died at Stripe
//!                               (authorize_and_activate, convergence)
//!
//! authorized ─┬─> superseded       a raise's activation demoted it
//!             │                    (activate_intent_tx)
//!             ├─> capture_pending  settlement: winner owes a card
//!             │                    capture (settle_auction_funding_tx)
//!             ├─> release_pending  nothing owed: settlement, auction
//!             │                    cancel, member departure
//!             │                    (settle_auction_funding_tx,
//!             │                    release_auction_intents_tx,
//!             │                    release_member_intents_tx)
//!             ├─> canceled|expired out-of-band Stripe cancel
//!             │                    (handle_intent_event; reason
//!             │                    'automatic' = expired); the
//!             │                    worker's ended-auction catchall
//!             │                    (funding_flow::cancel_worker_intent);
//!             │                    a window reject landing on an
//!             │                    adopted row (record_intent_decline);
//!             │                    convergence repair (missed webhook)
//!             └─> captured         out-of-band capture (e.g. the
//!                                  dashboard's Capture button), booked
//!                                  by convergence
//!
//! superseded,     ─┬─> canceled    worker canceled the hold at Stripe
//! release_pending  │  |expired     (funding_flow::cancel_worker_intent);
//!                  │               convergence repair (missed webhook,
//!                  │               replaced account)
//!                  └─> captured    out-of-band capture on a hold owed
//!                                  a release, booked by convergence
//!
//! capture_pending ─┬─> captured    worker captured at Stripe
//!                  │               (funding_flow::capture_intent);
//!                  │               convergence re-booking a capture
//!                  │               whose commit was lost
//!                  └─> failed      hold gone at Stripe (canceled
//!                                  out-of-band or expired)
//!                                  (fail_capture_tx)
//!
//! failed ──> captured              a buried capture surfaced (charged
//!                                  after the row was failed to debt);
//!                                  convergence books the recovery
//! ```
//!
//! Terminal states: `captured`, `failed`, `canceled`, `expired` —
//! `failed` excepted, reality outranks terminality: money collected on
//! a failed row converges to `captured` with the recovery booked. The
//! convergence edges all live in `store::convergence`, whose transition
//! table totally covers (local status × live Stripe status); this
//! module's guarded transition functions are what it applies.
//! `is_active` is set TRUE only by activation and cleared by demote,
//! worker cancel, capture, failure, and webhook cancels. It is nearly
//! derivable from status (`authorized` / `capture_pending` /
//! `release_pending` rows are active) but not quite: a late
//! activation's demote deactivates whatever row was active without
//! changing a non-`authorized` status, so the column stays explicit.

use jiff::Timestamp;
use payloads::{
    AccountOwner, ApiError, AuctionId, CommunityId, CurrencyMode,
    FundingIntentId, FundingIntentOrigin, FundingIntentStatus,
    OptionalTimestamp, PermissionLevel, SpaceId, UserId,
};
use rust_decimal::Decimal;
use sqlx::PgPool;

use super::StoreError;
use super::currency::get_auction_commitment_tx;
use super::locks::TrackedTx;
use crate::time::TimeSource;
use jiff_sqlx::ToSqlx;

// # Age-rule constants
//
// Defined together because they trade off against each other —
// adjusting one without the others silently breaks authorization
// coverage. The budget identity for weighing changes: `PREAUTH_ADVANCE
// + postponement_slack + RUNAWAY_DEADLINE + CAPTURE_MARGIN ≤
// MIN_HOLD_WINDOW`, where `postponement_slack` is how far a start can
// slip before the age scan cancels a hold minted at the advance edge —
// the identity leaves 12h. The floor is an assumption, not a
// guarantee: Stripe's per-charge `capture_before` is authoritative and
// stored at activation; the age scan and the mint-site window
// predicate read the stored value, and only the pre-auth advance gate
// predicts with the constant.

/// The assumed floor on a card authorization's capture window, used as
/// the fallback when Stripe omits the per-charge `capture_before`.
/// Sized to the shortest common real-world window — Visa gives
/// merchant-initiated transactions exactly 4d18h (5 nominal days less
/// clearing time; most other networks give 7d), and our off-session
/// confirms on saved cards are MITs — so 4d underestimates safely: a
/// too-short assumed window costs headroom, a too-long one would count
/// dead authorizations as live.
const MIN_HOLD_WINDOW_HOURS: i64 = 4 * 24;

/// Retry headroom between an auction's conclusion deadline and the
/// auth's capture window: capture failures are availability failures
/// (funds are reserved, so issuers can't decline an in-window capture),
/// and the margin buys the capture worker's backoff time in the worst
/// alignment (full-runtime auction × oldest-allowed auth). Margin
/// overrun degrades to manual debt collection, so it wins the trade
/// against postponement slack, whose overrun self-heals.
const CAPTURE_MARGIN_HOURS: i64 = 12;

/// The maximum runtime of a backed-mode auction, fixed at creation: rather
/// than create a round that would end past this point, the scheduler cancels
/// via the runaway path (all holds release, nobody pays), so no round straddles
/// the deadline. This is what makes every auth's window cover conclusion by
/// construction. Defined in payloads so the auction-creation UI can warn
/// against it.
const RUNAWAY_DEADLINE_HOURS: i64 = payloads::AUCTION_RUNAWAY_DEADLINE_HOURS;

/// The uniform authorization advance: how early the scheduled pre-auth
/// task mints holds, and how early the manual pre-authorize endpoint
/// opens before a known start — one number organizers and members
/// reason about, and the guarantee to organizers that a start can be
/// postponed up to the 12h slack before anyone's hold is invalidated.
/// The identity would allow up to 36h (`MIN_HOLD_WINDOW −
/// RUNAWAY_DEADLINE − CAPTURE_MARGIN`), but a hold minted at that edge
/// has zero slack — any postponement cancels it.
const PREAUTH_ADVANCE_HOURS: i64 = 24;

pub(crate) fn min_hold_window() -> jiff::Span {
    jiff::Span::new().hours(MIN_HOLD_WINDOW_HOURS)
}

pub(crate) fn runaway_deadline() -> jiff::Span {
    jiff::Span::new().hours(RUNAWAY_DEADLINE_HOURS)
}

pub(crate) fn preauth_advance() -> jiff::Span {
    jiff::Span::new().hours(PREAUTH_ADVANCE_HOURS)
}

/// The window a pre-start authorization must retain past the auction
/// deadline's base (the known start, or now when the start is unknown)
/// to stay viable: the full runtime plus capture margin. The age scan
/// cancels active holds below it and the mint-site predicate rejects
/// fresh auths below it — the same comparison, so a hold that survives
/// minting only ever cancels because the start moved or lingered.
pub(crate) fn min_viable_window_hours() -> i64 {
    RUNAWAY_DEADLINE_HOURS + CAPTURE_MARGIN_HOURS
}

/// Whether a hold's capture window covers the auction's conclusion
/// deadline plus capture margin — the viability predicate shared by the
/// finalize's mint-site check and webhook adoption, so the two never
/// disagree on more than the `now` between their evaluations. The
/// deadline base is the known start, or `now` while the start is
/// unknown.
pub(crate) async fn hold_window_covers_auction(
    auction_id: &AuctionId,
    capture_before: Timestamp,
    now: Timestamp,
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar(
        "SELECT $2 >= COALESCE(a.start_at, $3) + $4 * INTERVAL '1 hour' \
         FROM auctions a WHERE a.id = $1",
    )
    .bind(auction_id)
    .bind(capture_before.to_sqlx())
    .bind(now.to_sqlx())
    .bind(min_viable_window_hours() as f64)
    .fetch_one(executor)
    .await?)
}

/// A fresh authorization's capture deadline: Stripe's per-charge
/// `capture_before` when present, else the assumed floor from now (the
/// mock, or a missing field — worth noticing if real Stripe starts
/// omitting it). Shared by the finalize's confirm path and webhook
/// adoption.
pub(crate) fn capture_before_or_floor(
    epoch: Option<i64>,
    intent_id: &FundingIntentId,
    payment_intent_id: &str,
    time_source: &TimeSource,
) -> anyhow::Result<Timestamp> {
    match epoch {
        Some(epoch) => Ok(Timestamp::from_second(epoch)?),
        None => {
            tracing::warn!(
                %intent_id,
                payment_intent_id,
                "authorization lacked the per-charge capture window; \
                 assuming the floor"
            );
            Ok(time_source.now().checked_add(min_hold_window())?)
        }
    }
}

/// How long a `pending` authorization order may sit unexecuted before the
/// intent worker cancels it. A stranded order — crash between order and
/// execute, a proxy try-lock miss on the round's last pass, a card removed
/// after ordering — would otherwise pin its stale amount forever: orders are
/// immutable in place, so `ensure_pending_intent_tx` reuses the row's amount
/// for every later need it covers (undersized rows it cancels and re-orders
/// itself). Must exceed the webhook-adoption window: a crashed execute
/// that did reach Stripe converges via `amount_capturable_updated` adoption,
/// which requires the row still `pending`. A canceled no-PI row then feeds
/// the orphaned-hold sweep, which resolves any Stripe-side hold by metadata
/// probe.
const STALE_ORDER_AGE_HOURS: i64 = 1;

pub(crate) fn stale_order_age_hours() -> i64 {
    STALE_ORDER_AGE_HOURS
}

/// A funding_intents row (the mirror of a manual-capture PaymentIntent).
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct FundingIntent {
    pub id: FundingIntentId,
    // Carried for phase 6/7 consumers (capture worker, hold history);
    // dead-code-allowed until those land.
    #[allow(dead_code)]
    pub auction_id: AuctionId,
    #[allow(dead_code)]
    pub user_id: UserId,
    #[allow(dead_code)]
    pub payment_intent_id: Option<String>,
    pub is_active: bool,
    pub status: FundingIntentStatus,
    #[allow(dead_code)]
    pub origin: FundingIntentOrigin,
    pub authorized_amount: Option<Decimal>,
    pub capture_amount: Option<Decimal>,
    #[sqlx(try_from = "OptionalTimestamp")]
    pub capture_before: Option<Timestamp>,
    #[sqlx(try_from = "OptionalTimestamp")]
    pub last_decline_at: Option<Timestamp>,
    pub last_decline_code: Option<String>,
}

impl FundingIntent {
    /// Whether this intent is a live authorization whose backing bids can
    /// count on: active, authorized, and inside its capture window.
    pub fn is_live_auth(&self, now: Timestamp) -> bool {
        self.is_active
            && self.status == FundingIntentStatus::Authorized
            && self.capture_before.is_some_and(|t| t > now)
    }
}

/// A member's funding view for one auction: what the bidding UI needs to
/// show headroom, the live authorization, and whether the card path can
/// extend backing (the funding-regime indicator).
pub async fn get_auction_funding(
    auction_id: &AuctionId,
    user_id: &UserId,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<payloads::responses::AuctionFunding, StoreError> {
    let (auction, _) = super::auction::get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Member,
        pool,
    )
    .await?;

    let mut tx = pool.begin().await?;
    let community_id: CommunityId = sqlx::query_scalar(
        "SELECT s.community_id FROM auctions a \
         JOIN sites s ON a.site_id = s.id WHERE a.id = $1",
    )
    .bind(auction_id)
    .fetch_one(&mut *tx)
    .await?;

    // An ended auction (settled or canceled) commits nothing further — mirror
    // `get_commitment_tx`'s active-auction filter rather than reporting the
    // final round's stale winning values as committed, and report zero balance
    // backing (nothing here can draw on it).
    let (balance_backing, commitment) = if auction.end_at.is_some() {
        (Decimal::ZERO, Decimal::ZERO)
    } else {
        let commitment =
            get_auction_commitment_tx(user_id, auction_id, &mut tx).await?;
        let backing = balance_backing_tx(
            &community_id,
            auction_id,
            user_id,
            time_source.now(),
            &mut tx,
        )
        .await?;
        (backing, commitment)
    };

    // Preview of the strategy-sized "place hold now": the same sizing
    // the pre-authorize endpoint applies to a request without an
    // explicit amount. None once ended (nothing left to hold).
    let preauth_target = if auction.end_at.is_none() {
        let sizing =
            load_sizing_context(&community_id, user_id, &mut tx).await?;
        strategy_preauth_amount(
            &sizing,
            (commitment - balance_backing).max(Decimal::ZERO),
            balance_backing,
            auction_id,
            user_id,
            &mut *tx,
        )
        .await?
    } else {
        None
    };

    let live_auth = active_intent(auction_id, user_id, &mut *tx)
        .await?
        .filter(|i| i.is_live_auth(time_source.now()));
    let latest = latest_intent(auction_id, user_id, &mut *tx).await?;
    // Post-settlement card charge state: the latest intent once
    // conclusion has marked it for capture.
    let capture = latest
        .as_ref()
        .filter(|i| {
            matches!(
                i.status,
                FundingIntentStatus::CapturePending
                    | FundingIntentStatus::Captured
                    | FundingIntentStatus::Failed
            )
        })
        .and_then(|i| {
            Some(payloads::responses::CaptureState {
                amount: i.capture_amount?,
                status: i.status,
            })
        });
    // Decline metadata on the latest intent is what pauses automatic
    // holds (`process_scheduled_preauths`' decline-pause arm); surface
    // it while the auction is live so the member learns of an
    // off-session decline on the page rather than only by email.
    let decline_pause = latest
        .filter(|i| auction.end_at.is_none() && i.last_decline_at.is_some())
        .map(|i| payloads::responses::FundingDeclinePause {
            code: i.last_decline_code,
        });

    let card_available =
        card_availability(&community_id, user_id, &mut *tx).await?;
    // Only surfaced while the auction is live: an open checkout on an
    // ended auction is just waiting out its session expiry.
    let checkout_pending = if auction.end_at.is_none() {
        super::funding_checkout::checkout_pending_amount(
            auction_id, user_id, &mut *tx,
        )
        .await?
    } else {
        None
    };

    Ok(payloads::responses::AuctionFunding {
        balance_backing,
        commitment,
        authorized: live_auth
            .as_ref()
            .and_then(|i| i.authorized_amount)
            .unwrap_or(Decimal::ZERO),
        capture_before: live_auth.and_then(|i| i.capture_before),
        card_available,
        checkout_pending,
        capture,
        decline_pause,
        preauth_target,
    })
}

/// Whether card-backed funding can operate for this member in this
/// community, reporting the first missing prerequisite otherwise.
pub(crate) async fn card_availability(
    community_id: &CommunityId,
    user_id: &UserId,
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<payloads::responses::CardAvailability, StoreError> {
    use payloads::responses::CardAvailability;

    let (charges_enabled, has_card, granted): (bool, bool, bool) =
        sqlx::query_as(&format!(
            "SELECT \
                EXISTS (SELECT 1 FROM communities \
                        WHERE id = $1 AND {op}), \
                EXISTS (SELECT 1 FROM user_payment_profiles \
                        WHERE user_id = $2 \
                          AND payment_method_id IS NOT NULL), \
                EXISTS (SELECT 1 FROM community_members \
                        WHERE community_id = $1 AND user_id = $2 \
                          AND card_charges_granted_at IS NOT NULL)",
            op = super::connect::account_operational_sql(""),
        ))
        .bind(community_id)
        .bind(user_id)
        .fetch_one(executor)
        .await?;

    Ok(if !charges_enabled {
        CardAvailability::CommunityNotChargesEnabled
    } else if !has_card {
        CardAvailability::NoSavedCard
    } else if !granted {
        CardAvailability::NotGranted
    } else {
        CardAvailability::Available
    })
}

/// Gate a bid against the member's derived backing.
///
/// This is the backed_credits replacement for
/// `check_sufficient_credit_tx`, called from the bid finalize. For
/// balance-only members (no intents) it is equivalent to the
/// community-wide credit check at limit 0, since balance commitments
/// reduce to commitments.
///
/// The check: the member's commitments in `auction_id` after adding
/// `bid_amount` must be covered by the auction's live card authorization
/// plus the balance no other auction claims (the module docs' unallocated
/// derivation, clamped at zero so a shortfall elsewhere never blocks a
/// fully card-backed bid here). Errors with `InsufficientBalance`
/// otherwise; writes nothing — backing is derived, so accepting the bid
/// is what claims the balance.
///
/// Lock contract: the member's account row must be covered by the transaction's
/// account locks — the balance is read from the locked-row snapshot, which
/// checks exactly that (member and community) on entry.
pub(crate) async fn check_bid_backing_tx(
    community_id: &CommunityId,
    auction_id: &AuctionId,
    user_id: &UserId,
    bid_amount: Decimal,
    time_source: &TimeSource,
    ttx: &mut TrackedTx<'_, '_>,
) -> Result<(), StoreError> {
    // The caller's liveness check may have raced the conclusion transaction,
    // and balance commitments on an ended auction derive to nothing — re-check
    // rather than approve a bid nothing will ever back.
    let live: Option<bool> =
        sqlx::query_scalar("SELECT end_at IS NULL FROM auctions WHERE id = $1")
            .bind(auction_id)
            .fetch_optional(&mut **ttx.tx())
            .await?;
    if live != Some(true) {
        return Err(ApiError::RoundEnded.into());
    }

    let balance = ttx
        .locked_member_account(community_id, user_id)?
        .balance_cached;
    let commitment_in_auction =
        get_auction_commitment_tx(user_id, auction_id, ttx.tx()).await?;
    let auth = active_intent(auction_id, user_id, &mut **ttx.tx())
        .await?
        .filter(|i| i.is_live_auth(time_source.now()))
        .and_then(|i| i.authorized_amount)
        .unwrap_or(Decimal::ZERO);
    let pending =
        pending_captures(community_id, user_id, &mut **ttx.tx()).await?;
    let other_balance_commitments = member_balance_commitments_tx(
        community_id,
        user_id,
        Some(auction_id),
        time_source.now(),
        ttx.tx(),
    )
    .await?;
    let backing =
        (balance + pending - other_balance_commitments).max(Decimal::ZERO);
    if commitment_in_auction + bid_amount > auth + backing {
        return Err(ApiError::InsufficientBalance.into());
    }
    // The funding view derives from bids, so an accepted bid changes it.
    crate::pubsub::emit(
        ttx.tx(),
        &payloads::AuctionEvent::FundingChanged {
            auction_id: *auction_id,
            user_id: *user_id,
        },
    )
    .await?;
    Ok(())
}

// There is no separate outflow gate: in backed_credits the generic debit check
// (`check_sufficient_credit_tx`, effective limit 0) already computes available
// credit as `balance + pending captures − Σ balance commitments`, so any entry
// debiting a member gates on unallocated balance by construction.

/// Settle the auction's card authorizations at conclusion.
///
/// Runs in the conclusion transaction after the settlement entry has
/// debited winners (whose account rows that entry locked), so balance
/// reads here see the post-debit state. Each winner's active
/// authorization is marked `capture_pending` sized to what their balance
/// can't cover, and every other active authorization (losers,
/// fully-covered winners) is marked `release_pending` for the intent
/// worker.
///
/// Capture sizing is `W − usable`, where `usable = max(0, balance + W +
/// pending captures − Σ other balance commitments)` (the module docs'
/// unallocated derivation, with the just-debited `W` added back to
/// recover the pre-settlement balance; this auction's own `end_at` is
/// already posted, so the live filter excludes it). Adjustments to the
/// result:
///
/// - Counting other auctions' pending captures as succeeded prevents
///   over-capturing when a prior auction's capture is still retrying; if one
///   later terminally fails, the member's debt is exactly its uncollected
///   amount (`fail_capture_tx`).
/// - The result is capped by the authorized amount — any remainder beyond it
///   surfaces as the member's negative balance.
/// - Sub-minimum results (below the denomination's minimum charge, which a card
///   cannot collect) release the hold and forgive the remainder with a treasury
///   issuance so the balance returns to zero.
///
/// Lock contract: expects the auction-processing lock (key-checked) and, when
/// there are winners to size captures for, the settlement entry's account locks
/// (winners + treasury) to be held — the balance reads re-lock them to refresh
/// the snapshot the entry consumed. The intent marks are single-row guarded
/// updates and the forgiveness entry re-locks already-held accounts (checked
/// by the re-lock's subset rule).
pub(crate) async fn settle_auction_funding_tx(
    community_id: &CommunityId,
    auction_id: &AuctionId,
    winner_payments: &std::collections::HashMap<UserId, Decimal>,
    time_source: &TimeSource,
    locks: &mut TrackedTx<'_, '_>,
) -> Result<(), StoreError> {
    locks.expect_processing(auction_id)?;
    let intents: Vec<(FundingIntentId, Decimal, UserId)> = sqlx::query_as(
        "SELECT id, authorized_amount, user_id \
         FROM funding_intents \
         WHERE auction_id = $1 AND is_active \
           AND status = 'authorized'",
    )
    .bind(auction_id)
    .fetch_all(&mut **locks.tx())
    .await?;
    if intents.is_empty() {
        return Ok(());
    }

    // The settlement entry's balance updates consumed the account snapshot;
    // refresh it (a re-lock of held rows, never a wait) and take each winner's
    // post-debit balance for the capture sizing below. Collected up front
    // because the forgiveness entries in the loop invalidate the snapshot
    // again. Losers-only conclusions skip this: no settlement entry ran, so no
    // account locks are held and no balance is read.
    let winner_ids: Vec<UserId> = intents
        .iter()
        .filter(|(_, _, u)| {
            winner_payments.get(u).is_some_and(|w| *w > Decimal::ZERO)
        })
        .map(|(_, _, u)| *u)
        .collect();
    let mut balances = std::collections::HashMap::<UserId, Decimal>::new();
    if !winner_ids.is_empty() {
        locks.refresh_accounts().await?;
        for user_id in &winner_ids {
            let account = locks.locked_member_account(community_id, user_id)?;
            balances.insert(*user_id, account.balance_cached);
        }
    }

    let currency_name: String = sqlx::query_scalar(
        "SELECT currency_name FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_one(&mut **locks.tx())
    .await?;
    let stripe_min = payloads::denomination(&currency_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "backed community {community_id} has non-denominated \
                 currency {currency_name}"
            )
        })?
        .stripe_min_charge;

    let now = time_source.now().to_sqlx();
    for (intent_id, authorized, user_id) in intents {
        let won = winner_payments
            .get(&user_id)
            .copied()
            .unwrap_or(Decimal::ZERO);
        let capture = if won > Decimal::ZERO {
            // `won > 0` is exactly the `winner_ids` predicate, so the map
            // covers this user.
            let balance = balances[&user_id];
            let pending =
                pending_captures(community_id, &user_id, &mut **locks.tx())
                    .await?;
            // This auction's `end_at` is already posted in this
            // transaction, so the live filter excludes it — no explicit
            // exclusion needed.
            let other_balance_commitments = member_balance_commitments_tx(
                community_id,
                &user_id,
                None,
                time_source.now(),
                locks.tx(),
            )
            .await?;
            let usable = (balance + won + pending - other_balance_commitments)
                .max(Decimal::ZERO);
            let needed = won - usable;
            if needed > authorized {
                tracing::warn!(
                    %intent_id, %user_id, %needed, %authorized,
                    "capture need exceeds the authorized amount; the \
                     remainder stays as the member's negative balance"
                );
            }
            needed.min(authorized)
        } else {
            Decimal::ZERO
        };

        // Both marks re-check status: activations (and their demotes)
        // are excluded by the processing lock this transaction holds
        // (see `activate_intent_tx`), but a lock-free webhook can still
        // converge an out-of-band cancel between the snapshot above and
        // this write. Leaving a canceled row alone makes the debit stand
        // as immediate honest debt rather than a capture attempt that
        // would only discover the gone hold at Stripe.
        if capture >= stripe_min {
            sqlx::query(
                "UPDATE funding_intents \
                 SET status = 'capture_pending', capture_amount = $1, \
                     updated_at = $2 \
                 WHERE id = $3 AND status = 'authorized'",
            )
            .bind(capture)
            .bind(now)
            .bind(intent_id)
            .execute(&mut **locks.tx())
            .await?;
            tracing::info!(
                %intent_id, %user_id, %capture,
                "authorization marked for capture at auction conclusion"
            );
        } else {
            if capture > Decimal::ZERO {
                // A card can't collect below the Stripe minimum, and
                // padding up would overcharge; forgive the remainder so
                // the balance returns to zero.
                super::currency::create_forgiveness_issuance_entry_tx(
                    community_id,
                    auction_id,
                    &user_id,
                    capture,
                    &intent_id,
                    time_source,
                    locks,
                )
                .await?;
                tracing::info!(
                    %intent_id, %user_id, amount = %capture,
                    "forgave sub-minimum card remainder"
                );
            }
            sqlx::query(
                "UPDATE funding_intents \
                 SET status = 'release_pending', updated_at = $1 \
                 WHERE id = $2 AND status = 'authorized'",
            )
            .bind(now)
            .bind(intent_id)
            .execute(&mut **locks.tx())
            .await?;
        }
        emit_funding_changed(&intent_id, locks.tx()).await?;
    }
    Ok(())
}

/// Mark the auction's active authorizations `release_pending` for the
/// intent worker: the auction was canceled, so no capture will ever be
/// owed. Runs in the cancel transaction alongside `end_at` /
/// `was_canceled`.
///
/// Lock contract: expects the auction-processing lock (key-checked — it
/// excludes concurrent activations); acquires only the affected intent
/// rows' locks.
pub(crate) async fn release_auction_intents_tx(
    auction_id: &AuctionId,
    time_source: &TimeSource,
    locks: &mut TrackedTx<'_, '_>,
) -> Result<(), StoreError> {
    locks.expect_processing(auction_id)?;
    let released: Vec<(FundingIntentId,)> = sqlx::query_as(
        "UPDATE funding_intents \
         SET status = 'release_pending', updated_at = $1 \
         WHERE auction_id = $2 \
           AND status = 'authorized' AND is_active \
         RETURNING id",
    )
    .bind(time_source.now().to_sqlx())
    .bind(auction_id)
    .fetch_all(&mut **locks.tx())
    .await?;
    for (intent_id,) in released {
        emit_funding_changed(&intent_id, locks.tx()).await?;
    }
    Ok(())
}

/// Mark a departing member's idle authorizations `release_pending` —
/// but never one backing standing bids.
///
/// Runs in the membership-deletion transaction; `community_id` None
/// spans all communities (account deletion). Open checkout rows are
/// canceled too (`funding_checkout::cancel_member_checkouts_tx`).
///
/// Bids are commitments that survive departure: if leaving released
/// their card backing, a member could walk away from any bid, and honest
/// bidding would no longer be enforced. Backed intents stay active until
/// settlement captures or releases them (the departed member can win;
/// their account persists for the debit and credit-back, and
/// orphaned-balance resolution already refuses committed funds).
/// Departure does end future obligations: proxy rows are deleted
/// alongside, and membership checks stop new bids. Owed captures
/// (`capture_pending`) proceed regardless.
///
/// Lock contract: takes no advisory locks; the status-guarded row
/// updates converge against racing claims (a swap-demoted candidate is
/// skipped, and a replacement activated mid-departure is kept if it
/// backs bids, else released by the worker's ended-auction catchall
/// when its auction ends).
pub(crate) async fn release_member_intents_tx(
    community_id: Option<&CommunityId>,
    user_id: &UserId,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    // Live auctions only: ended auctions' intents belong to settlement
    // (and the intent worker's ended-auction catchall).
    let candidates: Vec<(FundingIntentId, AuctionId)> = sqlx::query_as(
        "SELECT fi.id, fi.auction_id \
         FROM funding_intents fi \
         JOIN auctions a ON fi.auction_id = a.id \
         JOIN sites s ON a.site_id = s.id \
         WHERE fi.user_id = $1 \
           AND ($2::uuid IS NULL OR s.community_id = $2) \
           AND fi.status = 'authorized' AND fi.is_active \
           AND a.end_at IS NULL",
    )
    .bind(user_id)
    .bind(community_id)
    .fetch_all(&mut **tx)
    .await?;

    let now = time_source.now().to_sqlx();
    for (intent_id, auction_id) in candidates {
        let commitment =
            get_auction_commitment_tx(user_id, &auction_id, tx).await?;
        if commitment > Decimal::ZERO {
            tracing::info!(
                %intent_id, %user_id, %auction_id, %commitment,
                "keeping departing member's authorization: it backs \
                 standing bids"
            );
            continue;
        }
        sqlx::query(
            "UPDATE funding_intents \
             SET status = 'release_pending', updated_at = $1 \
             WHERE id = $2 AND status = 'authorized'",
        )
        .bind(now)
        .bind(intent_id)
        .execute(&mut **tx)
        .await?;
        emit_funding_changed(&intent_id, tx).await?;
    }

    super::funding_checkout::cancel_member_checkouts_tx(
        community_id,
        user_id,
        time_source,
        tx,
    )
    .await?;
    Ok(())
}

/// The context a terminal capture failure's notifications need,
/// returned by the failure mark's guarded update.
#[derive(sqlx::FromRow)]
struct FailedRow {
    community_id: CommunityId,
    auction_id: AuctionId,
    user_id: UserId,
    capture_amount: Option<Decimal>,
}

/// Transition a `capture_pending` intent to `failed` — the capture will
/// never be collected (canceled out-of-band, or the window expired) —
/// and notify the member and the community's stewards.
///
/// This is the moment a win degrades into member debt, symmetric with
/// `CaptureReceipt` on success. The member's negative balance remains
/// the authoritative debt record (the lost credit-back shrinks the
/// derived unallocated balance by itself — nothing to re-fit); the
/// stored `capture_amount` is informational from here on.
///
/// Returns whether the transition applied (false when another writer
/// already moved the row); callers log the triggering condition.
///
/// Lock contract: callers hold either no advisory locks (the webhook) or the
/// pair lock (the capture worker); acquires only the intent row.
pub(crate) async fn fail_capture_tx(
    intent_id: &FundingIntentId,
    time_source: &TimeSource,
    ttx: &mut TrackedTx<'_, '_>,
) -> Result<bool, StoreError> {
    let row: Option<FailedRow> = sqlx::query_as(
        "UPDATE funding_intents fi \
         SET status = 'failed', is_active = FALSE, updated_at = $1 \
         FROM auctions a \
         JOIN sites s ON a.site_id = s.id \
         WHERE fi.auction_id = a.id AND fi.id = $2 \
           AND fi.status = 'capture_pending' \
         RETURNING s.community_id, fi.auction_id, fi.user_id, \
                   fi.capture_amount",
    )
    .bind(time_source.now().to_sqlx())
    .bind(intent_id)
    .fetch_optional(&mut **ttx.tx())
    .await?;
    let Some(row) = row else {
        return Ok(false);
    };
    notify_capture_failed_tx(intent_id, &row, time_source, ttx.tx()).await?;
    emit_funding_changed(intent_id, ttx.tx()).await?;
    Ok(true)
}

/// Enqueue the capture-failed notifications (member plus the
/// community's coleaders/leader) in the failure transaction.
async fn notify_capture_failed_tx(
    intent_id: &FundingIntentId,
    row: &FailedRow,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    use super::notifications::{Audience, NotificationParams};

    let (community_name, currency_symbol, currency_minor_units): (
        String,
        String,
        i16,
    ) = sqlx::query_as(
        "SELECT name, currency_symbol, currency_minor_units \
         FROM communities WHERE id = $1",
    )
    .bind(row.community_id)
    .fetch_one(&mut **tx)
    .await?;
    let amount = payloads::format_amount(
        &currency_symbol,
        currency_minor_units,
        row.capture_amount.unwrap_or_default(),
    );
    let params = |audience| NotificationParams::CaptureFailed {
        community_name: community_name.clone(),
        auction_id: row.auction_id,
        amount: amount.clone(),
        audience,
    };
    super::notifications::fan_out_to_member_and_stewards(
        &row.community_id,
        &row.user_id,
        &format!("capture_failed:{intent_id}"),
        &params(Audience::Member),
        Some(&params(Audience::Steward)),
        time_source,
        tx,
    )
    .await
}

/// Enqueue the authorize-again notice for an age-canceled member hold
/// in the worker's claim transaction (dedup key `age_cancel:{intent
/// id}`), reading the community's display fields on the transaction.
pub(crate) async fn notify_authorization_expiring_tx(
    intent_id: &FundingIntentId,
    auction_id: &AuctionId,
    community_id: &CommunityId,
    user_id: &UserId,
    amount: Decimal,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    let (community_name, currency_symbol, currency_minor_units): (
        String,
        String,
        i16,
    ) = sqlx::query_as(
        "SELECT name, currency_symbol, currency_minor_units \
         FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_one(&mut **tx)
    .await?;
    super::notifications::enqueue_notification_tx(
        user_id,
        &format!("age_cancel:{intent_id}"),
        &super::notifications::NotificationParams::AuthorizationExpiring {
            community_name,
            auction_id: *auction_id,
            amount: payloads::format_amount(
                &currency_symbol,
                currency_minor_units,
                amount,
            ),
        },
        time_source,
        &mut **tx,
    )
    .await
}

/// Enqueue the capture receipt in the capture worker's claim
/// transaction (dedup key `capture_receipt:{intent id}`), reading the
/// community's display fields on the transaction.
pub(crate) async fn notify_capture_receipt_tx(
    intent_id: &FundingIntentId,
    auction_id: &AuctionId,
    community_id: &CommunityId,
    user_id: &UserId,
    amount: Decimal,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    let (community_name, currency_symbol, currency_minor_units): (
        String,
        String,
        i16,
    ) = sqlx::query_as(
        "SELECT name, currency_symbol, currency_minor_units \
         FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_one(&mut **tx)
    .await?;
    super::notifications::enqueue_notification_tx(
        user_id,
        &format!("capture_receipt:{intent_id}"),
        &super::notifications::NotificationParams::CaptureReceipt {
            community_name,
            auction_id: *auction_id,
            amount: payloads::format_amount(
                &currency_symbol,
                currency_minor_units,
                amount,
            ),
        },
        time_source,
        &mut **tx,
    )
    .await
}

/// Book a captured hold's collected amount in the caller's transaction —
/// the money-path triple shared by the capture worker and convergence's
/// `BookCapture` arm: the treasury→member issuance entry (idempotent per
/// intent via the shared entry key), the capture receipt notification, and
/// the `FundingChanged` emit. The status transition stays caller-side: the
/// worker marks `captured` from `capture_pending`, convergence's guarded
/// UPDATE covers the other prior statuses.
///
/// Lock contract: expects the pair lock (key-checked by the issuance
/// entry, which also acquires the treasury and member account locks,
/// sorted).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn book_capture_tx(
    intent_id: &FundingIntentId,
    auction_id: &AuctionId,
    community_id: &CommunityId,
    user_id: &UserId,
    amount: Decimal,
    payment_intent_id: &str,
    time_source: &TimeSource,
    locks: &mut TrackedTx<'_, '_>,
) -> Result<(), StoreError> {
    super::currency::create_capture_issuance_entry_tx(
        community_id,
        auction_id,
        user_id,
        amount,
        payment_intent_id,
        intent_id,
        time_source,
        locks,
    )
    .await?;
    notify_capture_receipt_tx(
        intent_id,
        auction_id,
        community_id,
        user_id,
        amount,
        time_source,
        locks.tx(),
    )
    .await?;
    crate::pubsub::emit(
        locks.tx(),
        &payloads::AuctionEvent::FundingChanged {
            auction_id: *auction_id,
            user_id: *user_id,
        },
    )
    .await?;
    Ok(())
}

/// The member's committed incoming credit-backs: Σ `capture_amount` over
/// their `capture_pending` intents in the community. Treated as succeeded
/// by every derived-backing computation (the gates' pending term and
/// settlement sizing). Also the pending term in the purchase module's
/// effective-debt derivation, so the two stay in lockstep by
/// construction.
pub(crate) async fn pending_captures(
    community_id: &CommunityId,
    user_id: &UserId,
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<Decimal, StoreError> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(SUM(fi.capture_amount), 0) \
         FROM funding_intents fi \
         JOIN auctions a ON fi.auction_id = a.id \
         JOIN sites s ON a.site_id = s.id \
         WHERE s.community_id = $1 AND fi.user_id = $2 \
           AND fi.status = 'capture_pending'",
    )
    .bind(community_id)
    .bind(user_id)
    .fetch_one(executor)
    .await?)
}

/// Whether the community runs the stripe-backed backed_credits mode (the
/// only mode with funding rows).
pub(crate) async fn is_backed_mode(
    community_id: &CommunityId,
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<bool, StoreError> {
    let mode: CurrencyMode = sqlx::query_scalar(
        "SELECT currency_mode FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_one(executor)
    .await?;
    Ok(mode == CurrencyMode::BackedCredits)
}

/// The balance backing available to one auction — the module docs'
/// unallocated derivation with this auction's own balance commitment
/// excluded, the same balance term the bid gate applies.
///
/// Lock-free callers (plan-phase gap sizing, the funding view, the
/// checkout mint's floor) treat it as advisory — the authoritative gate
/// is `check_bid_backing_tx` at finalize; the shrink guard in
/// `activate_intent_tx` reads it under the member's account-row lock,
/// where it is authoritative.
pub(crate) async fn balance_backing_tx(
    community_id: &CommunityId,
    auction_id: &AuctionId,
    user_id: &UserId,
    now: Timestamp,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Decimal, StoreError> {
    let balance: Decimal = sqlx::query_scalar(
        "SELECT balance_cached FROM accounts \
         WHERE community_id = $1 AND owner_type = 'member_main' \
           AND owner_id = $2",
    )
    .bind(community_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or(Decimal::ZERO);
    let pending = pending_captures(community_id, user_id, &mut **tx).await?;
    let other_balance_commitments = member_balance_commitments_tx(
        community_id,
        user_id,
        Some(auction_id),
        now,
        tx,
    )
    .await?;
    Ok((balance + pending - other_balance_commitments).max(Decimal::ZERO))
}

/// The replacement floor for shedding a live hold: `max(0, commitment −
/// balance backing)` — what the bid gate would demand of the smaller
/// hold (the reduction grows this auction's balance commitment, which
/// must fit in the backing no other auction claims; already-gated bids
/// elsewhere are excluded from the backing term, so they keep theirs).
/// One definition so the advisory check (the checkout mint) and the
/// authoritative one (`activate_intent_tx`'s shrink guard, under the
/// account-row lock) can't drift; the pre-authorize resize reads the same
/// quantity as its gap view's `need_from_card`.
pub(crate) async fn replacement_floor_tx(
    community_id: &CommunityId,
    auction_id: &AuctionId,
    user_id: &UserId,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Decimal, StoreError> {
    let commitment = get_auction_commitment_tx(user_id, auction_id, tx).await?;
    let backing = balance_backing_tx(
        community_id,
        auction_id,
        user_id,
        time_source.now(),
        tx,
    )
    .await?;
    Ok((commitment - backing).max(Decimal::ZERO))
}

/// Σ `max(0, commitment − live authorized_amount)` over the member's live
/// auctions (minus `exclude`): the balance their bid commitments require — the
/// card hold covers first, balance covers the rest. The module docs define the
/// derivation; every gate and sizing reads balance commitments through here so
/// the definition has one home.
pub(crate) async fn member_balance_commitments_tx(
    community_id: &CommunityId,
    user_id: &UserId,
    exclude: Option<&AuctionId>,
    now: Timestamp,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Decimal, StoreError> {
    // One query for the live auths; commitment amounts are derived per auction
    // below (the same loop `get_commitment_tx` runs).
    let auths: Vec<(AuctionId, Decimal)> = sqlx::query_as(
        "SELECT fi.auction_id, fi.authorized_amount \
         FROM funding_intents fi \
         JOIN auctions a ON fi.auction_id = a.id \
         JOIN sites s ON a.site_id = s.id \
         WHERE s.community_id = $1 AND fi.user_id = $2 \
           AND fi.is_active AND fi.status = 'authorized' \
           AND fi.capture_before > $3 \
           AND a.end_at IS NULL",
    )
    .bind(community_id)
    .bind(user_id)
    .bind(now.to_sqlx())
    .fetch_all(&mut **tx)
    .await?;
    let auth_by_auction: std::collections::HashMap<AuctionId, Decimal> =
        auths.into_iter().collect();

    let live_auctions: Vec<AuctionId> = sqlx::query_scalar(
        "SELECT a.id FROM auctions a \
         JOIN sites s ON a.site_id = s.id \
         WHERE s.community_id = $1 AND a.end_at IS NULL",
    )
    .bind(community_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut balance_commitments = Decimal::ZERO;
    for live_id in &live_auctions {
        if Some(live_id) == exclude {
            continue;
        }
        let commitment =
            get_auction_commitment_tx(user_id, live_id, tx).await?;
        let auth = auth_by_auction
            .get(live_id)
            .copied()
            .unwrap_or(Decimal::ZERO);
        balance_commitments += (commitment - auth).max(Decimal::ZERO);
    }
    Ok(balance_commitments)
}

/// The (member, auction)'s active intent (at most one, by partial unique
/// index), regardless of liveness — callers filter with `is_live_auth`.
pub(crate) async fn active_intent(
    auction_id: &AuctionId,
    user_id: &UserId,
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<Option<FundingIntent>, StoreError> {
    Ok(sqlx::query_as(
        "SELECT id, auction_id, user_id, payment_intent_id, is_active, \
                status, origin, authorized_amount, capture_amount, \
                capture_before, last_decline_at, last_decline_code \
         FROM funding_intents \
         WHERE auction_id = $1 AND user_id = $2 AND is_active",
    )
    .bind(auction_id)
    .bind(user_id)
    .fetch_optional(executor)
    .await?)
}

/// The (member, auction)'s most recent intent row (by creation). Its
/// decline fields are what pause automatic (proxy/scheduled)
/// authorization attempts: a declined card would otherwise churn futile
/// swap attempts round after round. A later successful intent naturally
/// lifts the pause; member-present attempts ignore it.
pub(crate) async fn latest_intent(
    auction_id: &AuctionId,
    user_id: &UserId,
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<Option<FundingIntent>, StoreError> {
    Ok(sqlx::query_as(
        "SELECT id, auction_id, user_id, payment_intent_id, is_active, \
                status, origin, authorized_amount, capture_amount, \
                capture_before, last_decline_at, last_decline_code \
         FROM funding_intents \
         WHERE auction_id = $1 AND user_id = $2 \
         ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(auction_id)
    .bind(user_id)
    .fetch_optional(executor)
    .await?)
}

/// A sized authorization order as `ensure_pending_intent_tx` commits
/// it. Built by `GapView::order` (funding_flow), which derives the
/// order's relationship to the live hold — `replaces_intent_id` and
/// `reduction` — from the same view the amount was sized against, so
/// the bid, proxy, and pre-authorize paths can't disagree on it.
pub(crate) struct AuthOrder {
    pub origin: FundingIntentOrigin,
    /// The live hold this order supersedes on activation, if any.
    pub replaces_intent_id: Option<FundingIntentId>,
    pub requested_amount: Decimal,
    /// The order deliberately sizes below the live hold (a member
    /// resize down); stranded-row reuse then requires an exact match.
    pub reduction: bool,
}

/// Get-or-create the `pending` intent row for this (auction, member) —
/// the committed *authorization order* the execute claim replays.
///
/// Called by the order transactions of the bid, proxy, and pre-authorize
/// paths (see CONCURRENCY.md, "Why the card flows are order → execute →
/// bid"). The order's id seeds the Stripe idempotency key and
/// `requested_amount` fixes the create's parameters, so a crashed
/// attempt replays identically and converges at Stripe. Returns the
/// order's intent id.
///
/// Reuse rules for an existing pending row:
///
/// - Orders are immutable in place: a reused row keeps its stored amount, never
///   the fresh sizing, because a crashed attempt may have created a Stripe
///   object with it.
/// - Reuse requires the stored amount to cover this request — and, for a
///   `reduction` order, to match it exactly: an oversized reuse would deliver a
///   raise where the member asked for a resize down.
/// - An undersized stranded row would under-deliver (the execute holds the
///   stored amount and reports success, so a member-present "authorize 200"
///   that finds a stranded 50 order would leave the member believing they are
///   backed for 200), so it is canceled in place and a fresh order minted at
///   the current size in the same transaction. Webhook adoption of the canceled
///   row no-ops (guarded on `pending`), and a hold a crashed execute may have
///   minted at Stripe is resolved by its retried webhook
///   (`handle_intent_event`) or the orphaned-hold sweep's metadata probe.
/// - Concurrent pre-inserts collide on the pending partial unique index and the
///   loser evaluates the winner's row for reuse.
///
/// Lock contract: callers hold the `auction_user` pair lock (key-checked);
/// holds the community row FOR KEY SHARE until the transaction commits
/// (serializes order commits against `delete_community`'s wind-down
/// guard, see CONCURRENCY.md "communities row"); writes the single
/// pending intent row.
pub(crate) async fn ensure_pending_intent_tx(
    community_id: &CommunityId,
    auction_id: &AuctionId,
    user_id: &UserId,
    order: &AuthOrder,
    time_source: &TimeSource,
    locks: &mut TrackedTx<'_, '_>,
) -> Result<FundingIntentId, StoreError> {
    locks.expect_pair(auction_id, user_id)?;
    let tx = locks.tx();
    let now = time_source.now().to_sqlx();
    // Hold the community row FOR KEY SHARE until this transaction
    // commits. delete_community takes the row FOR UPDATE (the one
    // conflicting mode) before its in-flight-payments re-check, so an
    // order either commits strictly before that check — which then
    // sees the pending row and refuses the delete — or blocks here and
    // fails on the missing row after the delete, before any Stripe
    // call. credit-purchase minting gets the same serialization
    // implicitly from its community_id FK; this row's FK chain reaches
    // communities through auctions and sites, which only locks the
    // auction row, hence the explicit lock.
    let community_exists: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM communities WHERE id = $1 FOR KEY SHARE",
    )
    .bind(community_id)
    .fetch_optional(&mut **tx)
    .await?;
    if community_exists.is_none() {
        return Err(ApiError::CommunityNotFound.into());
    }

    let inserted: Option<FundingIntentId> = sqlx::query_scalar(
        "INSERT INTO funding_intents \
         (auction_id, user_id, status, origin, replaces_intent_id, \
          requested_amount, created_at, updated_at) \
         VALUES ($1, $2, 'pending', $3, $4, $5, $6, $6) \
         ON CONFLICT (auction_id, user_id) WHERE status = 'pending' \
         DO NOTHING \
         RETURNING id",
    )
    .bind(auction_id)
    .bind(user_id)
    .bind(order.origin)
    .bind(order.replaces_intent_id)
    .bind(order.requested_amount)
    .bind(now)
    .fetch_optional(&mut **tx)
    .await?;
    let id = match inserted {
        Some(id) => id,
        None => {
            // A pending row already exists (a prior attempt that never
            // reached a confirm outcome) — it is the key seed, reused so
            // a replayed create converges on the same Stripe object, but
            // only when its stored amount covers this request: executing
            // an undersized order would under-deliver while reporting
            // success. Read with fetch_optional: adoption (lock-free)
            // may promote the row between the insert's conflict and
            // this read, freeing the pending slot.
            let existing: Option<(FundingIntentId, Decimal)> = sqlx::query_as(
                "SELECT id, requested_amount FROM funding_intents \
                 WHERE auction_id = $1 AND user_id = $2 \
                   AND status = 'pending'",
            )
            .bind(auction_id)
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await?;
            match existing {
                // A reduction requires an exact match: executing a
                // larger stranded amount would deliver a raise where
                // the member asked for a resize down.
                Some((id, stored))
                    if stored >= order.requested_amount
                        && (!order.reduction
                            || stored == order.requested_amount) =>
                {
                    id
                }
                stale => {
                    // Cancel the undersized order in place (a guarded
                    // no-op if adoption just promoted it) and mint the
                    // order fresh at the current size. The pair lock
                    // serializes all pending-row inserters, so the slot
                    // is free; a violation errors loudly on the partial
                    // unique index.
                    if let Some((stale_id, stored)) = &stale {
                        cancel_stale_order_tx(stale_id, time_source, tx)
                            .await?;
                        tracing::info!(
                            intent_id = %stale_id,
                            stored_amount = %stored,
                            requested_amount = %order.requested_amount,
                            "stranded order can't cover the request; \
                             canceled in place and re-ordered",
                        );
                    }
                    sqlx::query_scalar(
                        "INSERT INTO funding_intents \
                         (auction_id, user_id, status, origin, \
                          replaces_intent_id, requested_amount, \
                          created_at, updated_at) \
                         VALUES ($1, $2, 'pending', $3, $4, $5, $6, $6) \
                         RETURNING id",
                    )
                    .bind(auction_id)
                    .bind(user_id)
                    .bind(order.origin)
                    .bind(order.replaces_intent_id)
                    .bind(order.requested_amount)
                    .bind(now)
                    .fetch_one(&mut **tx)
                    .await?
                }
            }
        }
    };
    Ok(id)
}

/// Whether a status guard also covers rows webhook adoption already
/// flipped to `authorized`. The finalize passes `Include`: its replay
/// must act on a row its own webhook adopted mid-flight. Adoption
/// itself and the plain decline path pass `Exclude`, so a row that
/// moved on makes their write a no-op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdoptedRows {
    Exclude,
    Include,
}

/// Outcome of [`activate_intent_tx`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivateOutcome {
    /// Promoted to the active authorization (any prior hold demoted).
    Promoted,
    /// The row already moved on; the whole transaction was a no-op.
    RowMovedOn,
    /// Activating would lower the live hold without the checkout
    /// reduction allowance; nothing was written. The caller owns the
    /// refused Stripe hold's release.
    ShrinkRefused,
}

/// Promote a confirmed intent to the row's single active authorization.
///
/// Called by the execute claim's finalize and by webhook adoption
/// (`adopt_confirmed_intent`); idempotent, so the two paths and a
/// finalize replay converge on the same end state. Demotes any other
/// active intent (`authorized` → `superseded`, worker cancels it at
/// Stripe), then flips this row to `authorized` + active with Stripe's
/// response amounts. Returns the [`ActivateOutcome`]: the target row is
/// locked and its status checked first, and a row that already moved on
/// makes the whole transaction a no-op — demoting first would strand the
/// funding row with no active authorization and hand the predecessor's
/// still-valid hold to the cancel worker.
///
/// Shrink guard: a promotion that would replace a larger live hold is
/// refused (`ShrinkRefused`) unless the row is a deliberate reduction —
/// a member checkout, or a member pre-authorize resize whose
/// `replaces_intent_id` still names the live hold — whose amount still
/// meets the replacement floor (`replacement_floor_tx`, which owns the
/// floor's rationale). Pre-Stripe guards (the execute claim's
/// stale-order cancel, the checkout mint's floor) apply the same rule,
/// but only this check runs after the hold exists, so only it decides.
/// Each side of the comparison holds its mutating lock: the hold
/// comparison the processing lock (activations serialize there), and the
/// floor measurement the member's account-row lock — the mutex every
/// commitment-growing and balance-spending path holds (bids commit under
/// it with no pair or processing lock), so the measured backing cannot
/// be claimed out from under the reduction.
///
/// [`AdoptedRows::Include`] widens the promotable set beyond
/// `pending`/`checkout_created` (checkout rows promote through webhook
/// adoption exactly like pending orders) for the finalize, whose replay
/// must converge a row its own webhook already adopted; adoption itself
/// passes `Exclude`, so adopting a row that has moved on is a
/// whole-transaction no-op (this also closes the gap where the lock-free
/// adoptability read grows stale before this transaction begins). For
/// the finalize the widening is a classification fix, not a state need:
/// an `authorized` row was activated by its own webhook with the same
/// PI's values, so the re-promote is an idempotent rewrite either way —
/// `Include` just reports the race as the success it is (`Promoted`)
/// instead of funneling a live hold into `RowMovedOn`, whose contract
/// (and the callers' arms built on it) assumes a dead row.
///
/// Lock contract: blocking-acquires the auction-processing lock, then the
/// promoted and demoted intent rows — the transaction's only writes; every
/// caller (the execute claim, which holds the pair lock, and webhook adoption,
/// which holds nothing) commits right after — no downstream code consumes the
/// held state. The processing lock closes one race: without it, a swap's demote
/// could land between a settlement/cancel pass's snapshot and its marks, and
/// the intent worker could Stripe-cancel the demoted row the pass was about to
/// mark `capture_pending`. Serialized, an activation lands wholly before the
/// pass or wholly after (caught by the worker's ended-auction catchall). See
/// CONCURRENCY.md for the holder criteria and nesting rule.
pub async fn activate_intent_tx(
    intent_id: &FundingIntentId,
    payment_intent_id: &str,
    authorized_amount: Decimal,
    capture_before: Timestamp,
    adopted: AdoptedRows,
    time_source: &TimeSource,
    ttx: &mut TrackedTx<'_, '_>,
) -> Result<ActivateOutcome, StoreError> {
    let (auction_id, user_id): (AuctionId, UserId) = sqlx::query_as(
        "SELECT auction_id, user_id FROM funding_intents WHERE id = $1",
    )
    .bind(intent_id)
    .fetch_one(&mut **ttx.tx())
    .await?;
    ttx.acquire_processing(&auction_id).await?;
    let tx = ttx.tx();

    // Lock the target and decide promotability before touching anything
    // else. Ordering is forced: the demote can't go first (see above),
    // and the promote can't either — the partial unique index on active
    // rows forbids two active intents even transiently.
    let (status, origin, replaces): (
        FundingIntentStatus,
        FundingIntentOrigin,
        Option<FundingIntentId>,
    ) = sqlx::query_as(
        "SELECT status, origin, replaces_intent_id FROM funding_intents \
         WHERE id = $1 FOR UPDATE",
    )
    .bind(intent_id)
    .fetch_one(&mut **tx)
    .await?;
    let promotable = matches!(
        status,
        FundingIntentStatus::Pending | FundingIntentStatus::CheckoutCreated
    ) || (adopted == AdoptedRows::Include
        && status == FundingIntentStatus::Authorized);
    if !promotable {
        return Ok(ActivateOutcome::RowMovedOn);
    }

    // The shrink guard (see the doc comment). Compares against the
    // *other* active row only — a finalize replay re-activating its own
    // adopted row must stay idempotent — and only a hold inside its
    // capture window counts (an expired one backs nothing).
    let current: Option<(FundingIntentId, Option<Decimal>, OptionalTimestamp)> =
        sqlx::query_as(
            "SELECT id, authorized_amount, capture_before FROM funding_intents \
             WHERE (auction_id, user_id) = ($2, $3) \
               AND is_active AND status = 'authorized' AND id != $1",
        )
        .bind(intent_id)
        .bind(auction_id)
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?;
    let live_current = current.and_then(|(id, amount, before)| {
        Option::<Timestamp>::from(before)
            .filter(|b| *b > time_source.now())
            .and(amount.map(|amount| (id, amount)))
    });
    if let Some((cur_id, cur)) = live_current
        && authorized_amount < cur
    {
        // Deliberate reductions come in two shapes: a member checkout
        // (whenever its session completes), and a member pre-authorize
        // resize whose order still targets exactly this hold — a
        // mismatched `replaces_intent_id` means the hold it meant to
        // shed was itself replaced mid-flight, and keep-larger wins.
        let member_resize = origin == FundingIntentOrigin::MemberPreauth
            && replaces == Some(cur_id);
        let reduction_covered =
            if origin == FundingIntentOrigin::MemberCheckout || member_resize {
                // Commitment and balance both move only under the member's
                // account-row lock (the funding gates' member mutex — the
                // bid path holds no pair or processing lock), so take that
                // row before measuring: otherwise an in-flight bid or
                // outflow gated against the larger hold could commit after
                // this read, stranding it above the reduced backing. Row
                // locks are level 3, after the processing lock, and neither
                // caller holds a prior account acquisition.
                let community_id: CommunityId = sqlx::query_scalar(
                    "SELECT s.community_id FROM auctions a \
                 JOIN sites s ON a.site_id = s.id WHERE a.id = $1",
                )
                .bind(auction_id)
                .fetch_one(&mut **ttx.tx())
                .await?;
                ttx.lock_account(&community_id, AccountOwner::Member(user_id))
                    .await?;
                authorized_amount
                    >= replacement_floor_tx(
                        &community_id,
                        &auction_id,
                        &user_id,
                        time_source,
                        ttx.tx(),
                    )
                    .await?
            } else {
                false
            };
        if !reduction_covered {
            return Ok(ActivateOutcome::ShrinkRefused);
        }
    }
    let tx = ttx.tx();

    let now = time_source.now().to_sqlx();
    sqlx::query(
        "UPDATE funding_intents \
         SET is_active = FALSE, \
             status = CASE WHEN status = 'authorized' \
                           THEN 'superseded'::funding_intent_status \
                           ELSE status END, \
             updated_at = $1 \
         WHERE (auction_id, user_id) = (SELECT auction_id, user_id \
                                        FROM funding_intents \
                                        WHERE id = $2) \
           AND is_active AND id != $2",
    )
    .bind(now)
    .bind(intent_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE funding_intents \
         SET payment_intent_id = $1, status = 'authorized', \
             is_active = TRUE, authorized_amount = $2, \
             capture_before = $3, authorized_at = $4, updated_at = $4 \
         WHERE id = $5",
    )
    .bind(payment_intent_id)
    .bind(authorized_amount)
    .bind(capture_before.to_sqlx())
    .bind(now)
    .bind(intent_id)
    .execute(&mut **tx)
    .await?;

    emit_funding_changed(intent_id, ttx.tx()).await?;
    Ok(ActivateOutcome::Promoted)
}

/// Record a refused confirm on its order row: `canceled`, deactivated,
/// with the decline fields set (failed raises are metadata, not state
/// transitions — the prior active authorization, if any, is untouched).
/// The Stripe intent id is linked when Stripe created one before
/// refusing. [`AdoptedRows::Include`] widens the status guard beyond
/// `pending` to rows a webhook adopted mid-flight; the call sites
/// explain which refusals need it.
pub(crate) async fn record_intent_decline(
    intent_id: &FundingIntentId,
    payment_intent_id: Option<&str>,
    decline_code: Option<&str>,
    adopted: AdoptedRows,
    time_source: &TimeSource,
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE funding_intents \
         SET status = 'canceled', is_active = FALSE, \
             payment_intent_id = COALESCE($1, payment_intent_id), \
             last_decline_at = $2, last_decline_code = $3, \
             updated_at = $2 \
         WHERE id = $4 \
           AND (status = 'pending' OR ($5 AND status = 'authorized'))",
    )
    .bind(payment_intent_id)
    .bind(time_source.now().to_sqlx())
    .bind(decline_code)
    .bind(intent_id)
    .bind(adopted == AdoptedRows::Include)
    .execute(executor)
    .await?;
    Ok(())
}

/// Cancel a `pending` order that should not execute: the execute claim found
/// a live hold at or above the order's amount (executing it would swap-demote
/// the larger hold), or the intent worker's aged-order arm found it stranded
/// unexecuted. Guarded: a no-op if the order already moved.
pub(crate) async fn cancel_stale_order_tx(
    intent_id: &FundingIntentId,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE funding_intents \
         SET status = 'canceled', updated_at = $1 \
         WHERE id = $2 AND status = 'pending'",
    )
    .bind(time_source.now().to_sqlx())
    .bind(intent_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Mark an intent terminally released after the worker canceled its hold at
/// Stripe (or found no Stripe object to cancel, or found it already canceled
/// there — a missed webhook), resetting worker backoff. `terminal_status` is
/// `canceled`, or `expired` when Stripe reported its own expiry. Guarded on
/// `prior_status` — the status the worker selected the row under — so a row a
/// webhook or racing claim moved meanwhile is left alone; returns whether the
/// mark applied.
pub(crate) async fn mark_intent_canceled_tx(
    intent_id: &FundingIntentId,
    prior_status: FundingIntentStatus,
    terminal_status: FundingIntentStatus,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<bool, StoreError> {
    debug_assert!(matches!(
        terminal_status,
        FundingIntentStatus::Canceled | FundingIntentStatus::Expired
    ));
    let updated = sqlx::query(
        "UPDATE funding_intents \
         SET status = $4, is_active = FALSE, \
             worker_failure_count = 0, worker_last_failed_at = NULL, \
             updated_at = $1 \
         WHERE id = $2 AND status = $3",
    )
    .bind(time_source.now().to_sqlx())
    .bind(intent_id)
    .bind(prior_status)
    .bind(terminal_status)
    .execute(&mut **tx)
    .await?;
    Ok(updated.rows_affected() > 0)
}

/// The Stripe cancel idempotency key shared by every path that cancels a
/// funding intent's PaymentIntent — the worker cancel, the execute
/// cancel-and-reject, the checkout release, the orphan sweep, convergence,
/// and the canceled-order webhook — so racing cancels collapse into one
/// Stripe-side cancel.
pub(crate) fn cancel_idempotency_key(intent_id: &FundingIntentId) -> String {
    format!("{intent_id}:cancel")
}

/// What [`cancel_hold`] found at Stripe: the hold canceled by this call, or
/// already canceled (a replay through the shared key, a Stripe-side cancel,
/// or the intent's own expiry) — gone either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CancelHoldOutcome {
    Canceled,
    AlreadyCanceled,
}

/// Cancel a hold's PaymentIntent at Stripe under the shared
/// [`cancel_idempotency_key`], tolerating an already-canceled intent. Any
/// other state conflict (e.g. captured from the dashboard) or transport
/// error propagates unchanged for the caller's own handling.
pub(crate) async fn cancel_hold(
    account_id: &str,
    payment_intent_id: &str,
    intent_id: &FundingIntentId,
    stripe_service: &crate::stripe_service::StripeService,
) -> Result<CancelHoldOutcome, crate::stripe_service::StripeCallError> {
    use crate::stripe_service::{LivePiStatus, StripeCallError};
    match stripe_service
        .cancel_payment_intent(
            account_id,
            payment_intent_id,
            &cancel_idempotency_key(intent_id),
        )
        .await
    {
        Ok(()) => Ok(CancelHoldOutcome::Canceled),
        Err(StripeCallError::StateConflict {
            live_status: LivePiStatus::Canceled,
            ..
        }) => Ok(CancelHoldOutcome::AlreadyCanceled),
        Err(e) => Err(e),
    }
}

/// Mark a `capture_pending` intent `captured` after the worker's successful
/// Stripe capture, resetting worker backoff.
pub(crate) async fn mark_intent_captured_tx(
    intent_id: &FundingIntentId,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE funding_intents \
         SET status = 'captured', is_active = FALSE, \
             worker_failure_count = 0, worker_last_failed_at = NULL, \
             updated_at = $1 \
         WHERE id = $2 AND status = 'capture_pending'",
    )
    .bind(time_source.now().to_sqlx())
    .bind(intent_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Mark a canceled no-PI intent row resolved by the orphaned-hold sweep,
/// recording the PaymentIntent the metadata probe found (None when the
/// probe confirmed no Stripe object exists). `reconciled_at` retires the
/// row from the sweep's candidate set; the status guard skips rows a
/// late webhook repurposed since the sweep's re-verify.
pub(crate) async fn mark_intent_reconciled_tx(
    intent_id: &FundingIntentId,
    payment_intent_id: Option<&str>,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE funding_intents \
         SET payment_intent_id = COALESCE(payment_intent_id, $1), \
             reconciled_at = $2, updated_at = $2 \
         WHERE id = $3 AND status = 'canceled'",
    )
    .bind(payment_intent_id)
    .bind(time_source.now().to_sqlx())
    .bind(intent_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Record an intent-worker failure for backoff re-selection (the
/// `worker_failure_count` columns; status unchanged).
pub(crate) async fn record_worker_failure_tx(
    intent_id: &FundingIntentId,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE funding_intents \
         SET worker_failure_count = worker_failure_count + 1, \
             worker_last_failed_at = $1, updated_at = $1 \
         WHERE id = $2",
    )
    .bind(time_source.now().to_sqlx())
    .bind(intent_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Handle a Connect-endpoint PaymentIntent event for its mirror row.
///
/// Called from the webhook dispatcher for every `payment_intent.*` event
/// on a community's connected account. Stripe is authoritative, and the
/// app accepts transitions it never initiated — communities have full
/// dashboards and can cancel or capture intents themselves.
///
/// Security: both row lookups are scoped to the event's envelope
/// account. Object contents (metadata included) are writable by the
/// connected account's owner, while the envelope `account` is
/// Stripe-attested — so an event may only touch rows of the community
/// that owns that account. Without the scope on the adoption branch, a
/// forged `funding_intent_id` in a foreign account's PaymentIntent could
/// adopt another community's pending row. Unlike `find_purchase`, even
/// the stored-id lookup stays scoped: an old-account intent is
/// uncapturable through the current account, so admitting a replaced
/// account's `amount_capturable_updated` would move rows toward
/// authorized/capture_pending and manufacture doomed capture work — the
/// scope quietly quarantines those rows instead. Events without an
/// envelope account are skipped.
///
/// The state-bearing events (`canceled`, `succeeded`) are pokes: the
/// payload only routes (id, metadata, attested account); the handler
/// retrieves the object live and hands the pair to `store::convergence`,
/// so delivery order cannot matter — every poke re-derives from live
/// state. The remaining arms are order-safe by construction and keep
/// applying the payload: `payment_failed` records decline metadata
/// (monotonic stamps, only present in the event), and
/// `amount_capturable_updated`'s non-adoption branch is a monotonic
/// redelivery guard. Retrieves happen lock-free before any transaction;
/// convergence's claim takes the pair try-lock and skips when a live
/// claim owns the pair (that claim converges on its own commit; our own
/// confirm fires `amount_capturable_updated` mid-claim, so a blocking
/// acquire would wait behind the very call that triggered it — see
/// CONCURRENCY.md's webhook rules).
///
/// Intent matching: look up by `payment_intent_id`; on a miss, an event
/// carrying our `funding_intent_id` metadata is adopted into its NULL-id
/// `pending` row — closing the crash window between a successful Stripe
/// create and the finalize write, for the case where no retry ever runs.
/// Adoption also fires when the webhook simply outruns our own confirm
/// response (Stripe doesn't sequence delivery behind the API return) —
/// benign in both orders: adoption retrieves the PaymentIntent and
/// activates with the authoritative per-charge window
/// (`adopt_confirmed_intent`), the returning finalize converges the
/// same facts via the idempotent `activate_intent_tx`, and a
/// late-delivered event takes the non-adoption branch below, which
/// never touches `capture_before`. Adoption's activate is guarded on
/// the row still being `pending`, so an adoption whose lock-free
/// adoptability read went stale is a whole no-op rather than a
/// clobber. Events without our metadata are expected foreign traffic
/// on the community's account (debug-skip). A confirm event for a
/// canceled no-PI row is the retried webhook of a crashed execute whose
/// order `ensure_pending_intent_tx` canceled in place before the event
/// arrived — adoption is closed, so its live hold is resolved on the
/// spot (`resolve_canceled_order_hold`) rather than waiting for the
/// orphaned-hold sweep. Any other our-metadata-but-no-adoptable-row
/// warns (a bug, a post-cascade event, or a cross-account forgery).
pub(crate) async fn handle_intent_event(
    event_type: &str,
    event_account: Option<&str>,
    obj: &serde_json::Value,
    stripe_service: &crate::stripe_service::StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let payment_intent_id = obj["id"].as_str().ok_or_else(|| {
        StoreError::StripeError("intent event without object id".into())
    })?;
    let Some(event_account) = event_account else {
        tracing::warn!(
            payment_intent_id,
            event_type,
            "intent webhook without an envelope account; skipping"
        );
        return Ok(());
    };

    type MatchedIntent =
        (FundingIntentId, i16, FundingIntentStatus, AuctionId, UserId);
    let matched: Option<MatchedIntent> = sqlx::query_as(
        "SELECT fi.id, c.currency_minor_units, fi.status, \
                fi.auction_id, fi.user_id \
         FROM funding_intents fi \
         JOIN auctions a ON fi.auction_id = a.id \
         JOIN sites s ON a.site_id = s.id \
         JOIN communities c ON s.community_id = c.id \
         WHERE fi.payment_intent_id = $1 \
           AND c.stripe_account_id = $2",
    )
    .bind(payment_intent_id)
    .bind(event_account)
    .fetch_optional(pool)
    .await?;

    let (intent_id, minor_units, prior_status, auction_id, user_id) =
        match matched {
            Some(row) => row,
            None => {
                let meta_id = obj["metadata"]["funding_intent_id"]
                    .as_str()
                    .and_then(|s| s.parse::<uuid::Uuid>().ok())
                    .map(FundingIntentId);
                let Some(meta_id) = meta_id else {
                    tracing::debug!(
                        payment_intent_id,
                        event_type,
                        "skipping intent event without our metadata"
                    );
                    return Ok(());
                };
                let by_meta: Option<(
                    FundingIntentId,
                    i16,
                    FundingIntentStatus,
                    bool,
                    bool,
                    AuctionId,
                    UserId,
                )> = sqlx::query_as(
                    "SELECT fi.id, c.currency_minor_units, fi.status, \
                        fi.payment_intent_id IS NOT NULL, \
                        fi.reconciled_at IS NOT NULL, \
                        fi.auction_id, fi.user_id \
                 FROM funding_intents fi \
                 JOIN auctions a ON fi.auction_id = a.id \
                 JOIN sites s ON a.site_id = s.id \
                 JOIN communities c ON s.community_id = c.id \
                 WHERE fi.id = $1 AND c.stripe_account_id = $2",
                )
                .bind(meta_id)
                .bind(event_account)
                .fetch_optional(pool)
                .await?;
                match by_meta {
                    Some((
                        id,
                        minor_units,
                        status @ (FundingIntentStatus::Pending
                        | FundingIntentStatus::CheckoutCreated),
                        _,
                        _,
                        auction_id,
                        user_id,
                    )) => (id, minor_units, status, auction_id, user_id),
                    Some((
                        id,
                        _,
                        FundingIntentStatus::Canceled,
                        has_pi,
                        reconciled,
                        _,
                        _,
                    )) if event_type
                        == "payment_intent.amount_capturable_updated"
                        && !has_pi =>
                    {
                        if !reconciled {
                            resolve_canceled_order_hold(
                                &id,
                                payment_intent_id,
                                event_account,
                                stripe_service,
                                time_source,
                                pool,
                            )
                            .await?;
                        }
                        return Ok(());
                    }
                    _ => {
                        tracing::warn!(
                            payment_intent_id,
                            event_type,
                            event_account,
                            funding_intent_id = %meta_id,
                            "intent event carries our metadata but no adoptable \
                             row on the event's account"
                        );
                        return Ok(());
                    }
                }
            }
        };

    match event_type {
        "payment_intent.amount_capturable_updated" => {
            if matches!(
                prior_status,
                FundingIntentStatus::Pending
                    | FundingIntentStatus::CheckoutCreated
            ) {
                adopt_confirmed_intent(
                    &intent_id,
                    payment_intent_id,
                    event_account,
                    minor_units,
                    stripe_service,
                    time_source,
                    pool,
                )
                .await?;
                return Ok(());
            }
            let capturable = payloads::from_minor_units(
                obj["amount_capturable"].as_i64().unwrap_or(0),
                minor_units,
            );
            if capturable <= Decimal::ZERO {
                // Also fires when a capture consumes the hold
                // (capturable drops to zero) — nothing to converge here;
                // capture state is the worker's own write (phase 6).
                return Ok(());
            }
            // Belt-and-suspenders monotonic converge: amounts only
            // grow (raises mint new intents, so this is redelivery
            // protection, not load-bearing).
            sqlx::query(
                "UPDATE funding_intents \
                 SET authorized_amount = GREATEST( \
                         COALESCE(authorized_amount, 0), $1), \
                     updated_at = $2 \
                 WHERE id = $3 AND status = 'authorized'",
            )
            .bind(capturable)
            .bind(time_source.now().to_sqlx())
            .bind(intent_id)
            .execute(pool)
            .await?;
        }
        "payment_intent.canceled" | "payment_intent.succeeded" => {
            // Poke-and-converge: the payload only routed us here; the
            // live object decides, so delivery order can't matter —
            // every poke re-derives from live state. Transient retrieve
            // failures propagate so Stripe redelivers. A checkout row
            // reached by metadata may not have its PaymentIntent linked
            // yet (that normally happens at completion); link it first
            // or the converge has nothing to read the live state
            // against.
            if prior_status == FundingIntentStatus::CheckoutCreated {
                sqlx::query(
                    "UPDATE funding_intents \
                     SET payment_intent_id = \
                             COALESCE(payment_intent_id, $1), \
                         updated_at = $2 \
                     WHERE id = $3",
                )
                .bind(payment_intent_id)
                .bind(time_source.now().to_sqlx())
                .bind(intent_id)
                .execute(pool)
                .await?;
            }
            let retrieved = stripe_service
                .retrieve_payment_intent(event_account, payment_intent_id)
                .await
                .map_err(StoreError::stripe)?;
            let report = super::convergence::converge_intent(
                &intent_id,
                &auction_id,
                &user_id,
                event_account,
                retrieved.as_ref(),
                super::convergence::ConvergePool::api(pool),
                time_source,
                stripe_service,
            )
            .await?;
            match report {
                super::convergence::ConvergeReport::Busy => {
                    // A live claim owns the pair and converges on its
                    // own commit; reconciliation backstops a crashed
                    // one. Ack rather than force a redelivery.
                    tracing::info!(
                        %intent_id,
                        payment_intent_id,
                        event_type,
                        "poke skipped: pair claim in progress"
                    );
                }
                super::convergence::ConvergeReport::Anomaly { detail } => {
                    tracing::error!(
                        %intent_id,
                        payment_intent_id,
                        event_type,
                        detail,
                        "webhook poke found an anomalous intent pair"
                    );
                }
                report => {
                    tracing::debug!(
                        %intent_id,
                        payment_intent_id,
                        event_type,
                        ?report,
                        "webhook poke converged"
                    );
                }
            }
        }
        "payment_intent.payment_failed" => {
            // In-session declines on a checkout row are retryable inside
            // Checkout (the session stays open), and recording decline
            // metadata would trip the saved-card decline pause for the
            // member's automatic raises — leave the row untouched.
            if prior_status == FundingIntentStatus::CheckoutCreated {
                tracing::debug!(
                    %intent_id,
                    payment_intent_id,
                    "in-checkout decline; session remains retryable"
                );
                return Ok(());
            }
            let decline = crate::stripe_service::DeclineInfo {
                code: obj["last_payment_error"]["code"]
                    .as_str()
                    .map(String::from),
                decline_code: obj["last_payment_error"]["decline_code"]
                    .as_str()
                    .map(String::from),
                message: None,
                payment_intent_id: None,
            };
            let code = decline.best_code();
            sqlx::query(
                "UPDATE funding_intents \
                 SET last_decline_at = $1, last_decline_code = $2, \
                     payment_intent_id = COALESCE(payment_intent_id, $3), \
                     status = CASE WHEN status = 'pending' \
                                   THEN 'canceled'::funding_intent_status \
                                   ELSE status END, \
                     updated_at = $1 \
                 WHERE id = $4",
            )
            .bind(time_source.now().to_sqlx())
            .bind(code)
            .bind(payment_intent_id)
            .bind(intent_id)
            .execute(pool)
            .await?;
        }
        other => {
            tracing::debug!(other, "unhandled intent event type");
        }
    }
    Ok(())
}

/// Cancel the live hold behind a confirm event whose order row was
/// canceled in place before the event arrived
/// (`ensure_pending_intent_tx` replaced an undersized stranded order
/// whose crashed execute had in fact minted a hold).
///
/// The hold is definitionally orphaned: the row id seeds the create's
/// idempotency key, so at most one PaymentIntent exists per row,
/// `canceled` is terminal, and both activation paths guard against it —
/// no future path can want this hold. Safe lock-free for the same
/// reason: a claim still holding the pair lock would mean the
/// cancel-in-place hasn't committed and the row would still read
/// `pending`.
///
/// Cancels via [`cancel_hold`] (the shared idempotency key), so this,
/// the worker cancel, and the sweep converge on one Stripe cancel.
/// Ordering matters: cancel first, reconcile after — recording the
/// PaymentIntent id retires the row from the sweep's candidate set,
/// which must not happen while the hold may still be live. A failed
/// cancel propagates for redelivery; a crash between cancel and
/// reconcile replays through the shared key.
async fn resolve_canceled_order_hold(
    intent_id: &FundingIntentId,
    payment_intent_id: &str,
    event_account: &str,
    stripe_service: &crate::stripe_service::StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    cancel_hold(event_account, payment_intent_id, intent_id, stripe_service)
        .await
        .map_err(|e| StoreError::StripeError(e.to_string()))?;
    let mut tx = pool.begin().await?;
    mark_intent_reconciled_tx(
        intent_id,
        Some(payment_intent_id),
        time_source,
        &mut tx,
    )
    .await?;
    tx.commit().await?;
    tracing::info!(
        %intent_id,
        payment_intent_id,
        "orphaned hold of a canceled order resolved by its retried webhook"
    );
    Ok(())
}

/// Adopt a confirmed authorization into its `pending` row from its
/// `amount_capturable_updated` event: the finalize never ran (crash
/// window) or its response hasn't returned yet.
///
/// The payload lacks the per-charge capture window, so the handler
/// retrieves the PaymentIntent (lock-free, before any transaction) and
/// decides from live state: activate with the authoritative window when
/// it covers the auction's deadline, otherwise leave the row `pending` —
/// the live finalize does its cancel-and-reject, and a crashed finalize
/// converges via the aged-pending arm plus the orphan sweep.
///
/// Those backstops also cover the retrieve's negative outcomes, which
/// leave the row untouched: a missing intent (`resource_missing` — an
/// unverifiable payload should not activate anything) and a status other
/// than awaiting-capture (the intent's own terminal events converge the
/// row). Transient retrieve errors propagate so Stripe redelivers.
pub(crate) async fn adopt_confirmed_intent(
    intent_id: &FundingIntentId,
    payment_intent_id: &str,
    event_account: &str,
    minor_units: i16,
    stripe_service: &crate::stripe_service::StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let retrieved = stripe_service
        .retrieve_payment_intent(event_account, payment_intent_id)
        .await
        .map_err(StoreError::stripe)?;
    let Some(pi) = retrieved else {
        tracing::warn!(
            %intent_id,
            payment_intent_id,
            event_account,
            "adoption event's PaymentIntent does not exist on the \
             event's account; leaving the row pending"
        );
        return Ok(());
    };
    if pi.status != crate::stripe_service::LivePiStatus::RequiresCapture
        || pi.amount_capturable_minor <= 0
    {
        tracing::info!(
            %intent_id,
            payment_intent_id,
            status = %pi.status,
            "adoption event's PaymentIntent is no longer awaiting \
             capture; leaving the row to its own events"
        );
        return Ok(());
    }
    let capturable =
        payloads::from_minor_units(pi.amount_capturable_minor, minor_units);
    let capture_before = capture_before_or_floor(
        pi.capture_before_epoch,
        intent_id,
        payment_intent_id,
        time_source,
    )?;

    let (auction_id, origin, auction_ended): (
        AuctionId,
        FundingIntentOrigin,
        bool,
    ) = sqlx::query_as(
        "SELECT fi.auction_id, fi.origin, a.end_at IS NOT NULL \
         FROM funding_intents fi \
         JOIN auctions a ON fi.auction_id = a.id \
         WHERE fi.id = $1",
    )
    .bind(intent_id)
    .fetch_one(pool)
    .await?;
    let covers = hold_window_covers_auction(
        &auction_id,
        capture_before,
        time_source.now(),
        pool,
    )
    .await?;
    // A checkout completion the auction can no longer use has no
    // finalize to defer to: the hold is canceled with the released
    // notice. Pending orders keep the leave-pending behavior — the live
    // finalize does its own cancel-and-reject, a crashed one converges
    // via the aged-order arm plus the orphan sweep, and an
    // ended-auction activation is released by the worker's
    // ended-auction catchall.
    if origin == FundingIntentOrigin::MemberCheckout
        && (auction_ended || !covers)
    {
        super::funding_checkout::release_late_checkout(
            intent_id,
            payment_intent_id,
            event_account,
            capturable,
            stripe_service,
            time_source,
            pool,
        )
        .await?;
        return Ok(());
    }
    if !covers {
        tracing::info!(
            %intent_id,
            payment_intent_id,
            %capture_before,
            "adoption's authorization window can't cover the auction \
             deadline; leaving the row pending for the finalize's \
             rejection"
        );
        return Ok(());
    }

    let mut ttx = TrackedTx::begin(pool).await?;
    let outcome = activate_intent_tx(
        intent_id,
        payment_intent_id,
        capturable,
        capture_before,
        AdoptedRows::Exclude,
        time_source,
        &mut ttx,
    )
    .await?;
    ttx.commit().await?;
    match (outcome, origin) {
        (ActivateOutcome::Promoted, _) => {
            tracing::info!(
                %intent_id,
                payment_intent_id,
                "adopted orphaned intent from webhook"
            );
        }
        (
            ActivateOutcome::ShrinkRefused,
            FundingIntentOrigin::MemberCheckout,
        ) => {
            // The live hold outgrew this session between mint and
            // payment (the mint's floor can't see the future); the
            // completion would under-back committed bids, so it gets
            // the same cancel-with-notice as a late completion.
            tracing::info!(
                %intent_id,
                payment_intent_id,
                "checkout completion would lower the live hold below \
                 commitment; releasing it"
            );
            super::funding_checkout::release_late_checkout(
                intent_id,
                payment_intent_id,
                event_account,
                capturable,
                stripe_service,
                time_source,
                pool,
            )
            .await?;
        }
        (ActivateOutcome::ShrinkRefused, _) => {
            // A pending order that lost the race to a larger hold:
            // leave it for its backstops (the aged-order arm cancels
            // the row, the orphan sweep resolves the Stripe hold) —
            // the leave-pending convention of the !covers arm above.
            tracing::info!(
                %intent_id,
                payment_intent_id,
                "adoption would lower the live hold; leaving the order \
                 pending for its backstops"
            );
        }
        (ActivateOutcome::RowMovedOn, FundingIntentOrigin::MemberCheckout) => {
            // A checkout row that moved on was canceled (wind-down, a
            // stale retire) — unlike a pending order, no other actor
            // owns its live hold, so release it here; the shared
            // cancel key makes a race with the webhook arm converge on
            // one Stripe cancel.
            let unresolved: Option<(bool,)> = sqlx::query_as(
                "SELECT reconciled_at IS NULL FROM funding_intents \
                 WHERE id = $1 AND status = 'canceled'",
            )
            .bind(intent_id)
            .fetch_optional(pool)
            .await?;
            if let Some((true,)) = unresolved {
                resolve_canceled_order_hold(
                    intent_id,
                    payment_intent_id,
                    event_account,
                    stripe_service,
                    time_source,
                    pool,
                )
                .await?;
            }
        }
        (ActivateOutcome::RowMovedOn, _) => {
            tracing::debug!(
                %intent_id,
                payment_intent_id,
                "intent row moved on before adoption could activate; no-op"
            );
        }
    }
    Ok(())
}

/// Enqueue the backing-lost notifications for a live authorization lost
/// out-of-band (canceled, or naturally expired when `expired`), in the converge
/// transaction. The member is always notified — an active hold means they
/// wanted it active until the auction concludes, whether or not a bid stands
/// this instant (bids trade between rounds, and pre-start holds back intended
/// bidding). The community's coleaders and leader are notified only when the
/// hold was actually backing bids with standing commitment — that's when a win
/// can degrade into member debt.
pub(crate) async fn notify_backing_lost_tx(
    intent_id: &FundingIntentId,
    expired: bool,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    use super::notifications::{Audience, NotificationParams};

    #[derive(sqlx::FromRow)]
    struct BackingLostContext {
        user_id: UserId,
        auction_id: AuctionId,
        community_id: CommunityId,
        community_name: String,
        currency_symbol: String,
        currency_minor_units: i16,
        authorized_amount: Option<Decimal>,
    }
    let ctx: BackingLostContext = sqlx::query_as(
        "SELECT fi.user_id, fi.auction_id, s.community_id, \
                c.name AS community_name, c.currency_symbol, \
                c.currency_minor_units, fi.authorized_amount \
         FROM funding_intents fi \
         JOIN auctions a ON fi.auction_id = a.id \
         JOIN sites s ON a.site_id = s.id \
         JOIN communities c ON s.community_id = c.id \
         WHERE fi.id = $1",
    )
    .bind(intent_id)
    .fetch_one(&mut **tx)
    .await?;

    let amount = payloads::format_amount(
        &ctx.currency_symbol,
        ctx.currency_minor_units,
        ctx.authorized_amount.unwrap_or_default(),
    );
    let params = |audience| NotificationParams::BackingLost {
        community_name: ctx.community_name.clone(),
        auction_id: ctx.auction_id,
        amount: amount.clone(),
        audience,
        expired,
    };
    let commitment =
        get_auction_commitment_tx(&ctx.user_id, &ctx.auction_id, tx).await?;
    let steward_params =
        (commitment > Decimal::ZERO).then(|| params(Audience::Steward));
    super::notifications::fan_out_to_member_and_stewards(
        &ctx.community_id,
        &ctx.user_id,
        &format!("backing_lost:{intent_id}"),
        &params(Audience::Member),
        steward_params.as_ref(),
        time_source,
        tx,
    )
    .await
}

/// Emit `FundingChanged` for the (auction, user) behind an intent row.
pub(crate) async fn emit_funding_changed(
    intent_id: &FundingIntentId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    let (auction_id, user_id): (AuctionId, UserId) = sqlx::query_as(
        "SELECT auction_id, user_id FROM funding_intents WHERE id = $1",
    )
    .bind(intent_id)
    .fetch_one(&mut **tx)
    .await?;
    crate::pubsub::emit(
        tx,
        &payloads::AuctionEvent::FundingChanged {
            auction_id,
            user_id,
        },
    )
    .await?;
    Ok(())
}

/// Emit `FundingChanged` for each live auction whose budget preview reads
/// the member's value on this space (the space's site's live auctions),
/// backed_credits communities only — the funding view's `preauth_target`
/// derives from `auction_budget`, so a value write changes it without
/// touching any funding row.
pub(crate) async fn emit_funding_changed_for_space(
    space_id: &SpaceId,
    user_id: &UserId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    emit_funding_changed_for_spaces(&[*space_id], user_id, tx).await
}

/// Bulk variant of [`emit_funding_changed_for_space`]: one event per
/// distinct live backed auction across all the given spaces' sites.
pub(crate) async fn emit_funding_changed_for_spaces(
    space_ids: &[SpaceId],
    user_id: &UserId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    let auction_ids: Vec<AuctionId> = sqlx::query_scalar(
        "SELECT DISTINCT a.id FROM auctions a \
         JOIN sites si ON a.site_id = si.id \
         JOIN spaces sp ON sp.site_id = si.id \
         JOIN communities c ON si.community_id = c.id \
         WHERE sp.id = ANY($1) AND a.end_at IS NULL \
           AND c.currency_mode = 'backed_credits'",
    )
    .bind(space_ids)
    .fetch_all(&mut **tx)
    .await?;
    for auction_id in auction_ids {
        crate::pubsub::emit(
            tx,
            &payloads::AuctionEvent::FundingChanged {
                auction_id,
                user_id: *user_id,
            },
        )
        .await?;
    }
    Ok(())
}

/// Emit `FundingChanged` for one auction if it is live and in a
/// backed_credits community — for writes to budget inputs keyed by
/// auction (proxy `max_items`), where the funding view's `preauth_target`
/// changes without any funding row being touched.
pub(crate) async fn emit_funding_changed_if_live_backed(
    auction_id: &AuctionId,
    user_id: &UserId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    let live_backed: bool = sqlx::query_scalar(
        "SELECT EXISTS ( \
            SELECT 1 FROM auctions a \
            JOIN sites si ON a.site_id = si.id \
            JOIN communities c ON si.community_id = c.id \
            WHERE a.id = $1 AND a.end_at IS NULL \
              AND c.currency_mode = 'backed_credits')",
    )
    .bind(auction_id)
    .fetch_one(&mut **tx)
    .await?;
    if live_backed {
        crate::pubsub::emit(
            tx,
            &payloads::AuctionEvent::FundingChanged {
                auction_id: *auction_id,
                user_id: *user_id,
            },
        )
        .await?;
    }
    Ok(())
}

/// The member's auction budget: the sum of their `max_items` largest user
/// values among the auction's available spaces. `max_items` comes from
/// their proxy settings for the auction; without a proxy row it is
/// assumed 1 — manual bidding is unbounded, but summing every valued
/// space would size budget holds far past what a single-space bidder
/// spends, and a member who does chase more spaces gets the ordinary
/// bid-driven raise.
pub(crate) async fn auction_budget(
    auction_id: &AuctionId,
    user_id: &UserId,
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<Decimal, StoreError> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(SUM(value), 0) FROM ( \
            SELECT uv.value FROM user_values uv \
            JOIN spaces s ON uv.space_id = s.id \
            JOIN auctions a ON s.site_id = a.site_id \
            WHERE a.id = $1 AND uv.user_id = $2 \
              AND s.is_available AND s.deleted_at IS NULL \
            ORDER BY uv.value DESC \
            LIMIT COALESCE((SELECT max_items FROM use_proxy_bidding \
                            WHERE auction_id = $1 AND user_id = $2), 1) \
         ) top_values",
    )
    .bind(auction_id)
    .bind(user_id)
    .fetch_one(executor)
    .await?)
}

/// The denomination facts plus the member's hold strategy — the subset
/// of `funding_flow::CardContext` that authorization sizing needs, kept
/// separate so an order transaction can load it with in-transaction
/// reads: loading the full context runs pool-side queries, and a
/// transaction must never wait on a second pool connection.
#[derive(Clone)]
pub(crate) struct SizingContext {
    pub budget_holds: bool,
    /// Lowercase ISO code for Stripe calls.
    pub currency: String,
    pub minor_units: i16,
    pub stripe_min_charge: Decimal,
    pub stripe_max_charge: Decimal,
}

/// Load a `SizingContext` with reads on `conn` — a transaction's
/// connection (the proxy claim's reactive order,
/// `get_auction_funding`'s preview) or a pool connection
/// (`load_card_context` at peek time).
pub(crate) async fn load_sizing_context(
    community_id: &CommunityId,
    user_id: &UserId,
    conn: &mut sqlx::PgConnection,
) -> Result<SizingContext, StoreError> {
    let currency_name: String = sqlx::query_scalar(
        "SELECT currency_name FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_one(&mut *conn)
    .await?;
    let denomination =
        payloads::denomination(&currency_name).ok_or_else(|| {
            anyhow::anyhow!(
                "backed community {community_id} has non-denominated \
                 currency {currency_name}"
            )
        })?;
    let budget_holds: bool =
        sqlx::query_scalar("SELECT budget_holds FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&mut *conn)
            .await?;
    Ok(SizingContext {
        budget_holds,
        currency: denomination.iso.to_lowercase(),
        minor_units: denomination.minor_units,
        stripe_min_charge: denomination.stripe_min_charge,
        stripe_max_charge: denomination.stripe_max_charge,
    })
}

/// The strategy-sized pre-authorization amount for one (auction, member)
/// — what a pre-authorize request without an explicit amount holds:
/// budget net of balance backing (budget strategy) or the
/// card-verifying minimum, clamped to the charge limits and ceiled to
/// minor units. `need_from_card` is the commitment's requirement beyond
/// balance, the sizing floor either way. None when the target is zero
/// (nothing to hold). Shared by the pre-authorize order plan and
/// `get_auction_funding`'s preview so the preview can't drift from what
/// the button does.
pub(crate) async fn strategy_preauth_amount(
    sizing: &SizingContext,
    need_from_card: Decimal,
    balance_backing: Decimal,
    auction_id: &AuctionId,
    user_id: &UserId,
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<Option<Decimal>, StoreError> {
    let budget = auction_budget(auction_id, user_id, executor).await?;
    let target = initial_auth_size(
        sizing.budget_holds,
        need_from_card,
        budget - balance_backing,
        sizing.stripe_min_charge,
    )
    .min(sizing.stripe_max_charge);
    if target <= Decimal::ZERO {
        return Ok(None);
    }
    Ok(Some(ceil_to_minor_units(
        target.max(sizing.stripe_min_charge),
        sizing.minor_units,
    )))
}

/// Round up to the currency's minor units. Sizing inputs can derive from
/// `user_values`, which aren't quantization-validated; extra headroom is
/// safe where a sub-minor-unit Stripe amount is not.
pub(crate) fn ceil_to_minor_units(
    amount: Decimal,
    minor_units: i16,
) -> Decimal {
    amount.round_dp_with_strategy(
        minor_units as u32,
        rust_decimal::RoundingStrategy::AwayFromZero,
    )
}

/// Initial authorization size per the member's hold strategy. `need` is
/// the card gap the triggering operation requires; `budget_net` is the
/// member's auction budget net of the balance backing available at sizing
/// time (budget strategy only); `stripe_min` is the denomination's
/// minimum charge (minimum-start strategy's opening size).
pub(crate) fn initial_auth_size(
    budget_holds: bool,
    need: Decimal,
    budget_net: Decimal,
    stripe_min: Decimal,
) -> Decimal {
    if budget_holds {
        need.max(budget_net)
    } else {
        need.max(stripe_min)
    }
}

/// Swap-reauth raise target: catch-up-or-double. Doubling (rather than
/// resizing to exact need) keeps the number of swaps logarithmic in the
/// bid trajectory; the headroom releases at settlement.
pub(crate) fn raise_target(prior: Decimal, needed: Decimal) -> Decimal {
    (prior * Decimal::TWO).max(needed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    #[test]
    fn sizing_helpers() {
        // Budget strategy: cover the larger of need and net budget
        assert_eq!(
            initial_auth_size(true, dec!(5), dec!(40), dec!(0.50)),
            dec!(40)
        );
        assert_eq!(
            initial_auth_size(true, dec!(50), dec!(40), dec!(0.50)),
            dec!(50)
        );
        // Minimum start: open at the denomination minimum
        assert_eq!(
            initial_auth_size(false, dec!(0.10), dec!(40), dec!(0.50)),
            dec!(0.50)
        );
        assert_eq!(
            initial_auth_size(false, dec!(3), dec!(40), dec!(0.50)),
            dec!(3)
        );
        // Raises catch up or double
        assert_eq!(raise_target(dec!(4), dec!(5)), dec!(8));
        assert_eq!(raise_target(dec!(4), dec!(21)), dec!(21));
        // Ceiling to minor units
        assert_eq!(ceil_to_minor_units(dec!(1.001), 2), dec!(1.01));
        assert_eq!(ceil_to_minor_units(dec!(1.25), 2), dec!(1.25));
    }
}
