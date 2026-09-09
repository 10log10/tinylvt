//! Card-authorization orchestration for stripe-backed backed_credits
//! bidding: every sequence that sandwiches a Stripe call between
//! transactions. The member flows (the bid route, the pre-authorize
//! endpoint) and the proxy bidding claim share the order → execute
//! shape (the proxy's order transaction is the claim itself —
//! `order_proxy_bid_auth_tx` sizes the failed bid's gap under the
//! claim's pair lock); the intent worker's per-row claims (cancel,
//! capture) live here too — `scheduler` only selects work and
//! dispatches to this module.
//!
//! An *order* transaction (pair lock) verifies and sizes the card need once and
//! commits it as the `pending` intent row — the immutable authorization order.
//! The *execute* claim (`execute_auth_order`, fresh pair lock) replays the
//! order at Stripe and activates the result, committing as soon as the
//! authorization exists. Whatever consumes the new backing (the bid, the proxy
//! item's bidding claim) runs as its own later transaction — an authorization
//! without bids is a legal state, so the steps need no shared atomicity. The
//! full rationale and the pair lock's holder criteria live in
//! `api/CONCURRENCY.md`. The bid flow is attempt-first: `store::create_bid`
//! runs before any card machinery, so balance-covered bids never touch Stripe
//! and every bid-time validation rejection returns before an authorization
//! exists; only an `InsufficientBalance` rejection in backed mode opens the
//! order → execute sequence.
//!
//! Two module-wide rules: no data row lock ever spans a Stripe call (pre-call
//! the execute claim holds only the advisory lock), and no transaction ever
//! opens a second pool connection (the steps are strictly sequential).
//!
//! Stripe-spanning claims and blocking pair-lock waits run on the dedicated
//! worker pool (`WorkerPool`): they pin a connection for the call's (or wait's)
//! duration, and drawing them from the shared API pool would starve it at round
//! boundaries. That includes the ordinary bid transaction in backed_credits
//! mode, whose pair-lock wait can queue behind an execute claim; in other
//! modes no claim ever holds the lock across a network call, so the bid
//! stays on the shared pool.

use std::collections::HashMap;

use payloads::{
    ApiError, AuctionId, AuctionRoundId, CommunityId, FundingIntentOrigin,
    SpaceId, UserId,
};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::pubsub;
use crate::store::{
    self, StoreError,
    auction::planned_bid_amount_tx,
    funding::{
        self, ActivateOutcome, AdoptedRows, AuthOrder, FundingIntent,
        SizingContext, active_intent, auction_budget, balance_backing_tx,
        cancel_stale_order_tx, ceil_to_minor_units, ensure_pending_intent_tx,
        initial_auth_size, load_sizing_context, raise_target,
        record_intent_decline,
    },
    locks::{LockWait, TrackedTx},
};
use crate::stripe_service::{
    AuthorizationParams, DeclineInfo, LivePiStatus, StripeCallError,
    StripeService,
};
use crate::time::TimeSource;

/// The service dependencies every flow in this module threads: the
/// worker pool its claims and blocking waits run on, the clock, and the
/// Stripe client. Functions that deliberately lack one — the pure-DB
/// `cancel_aged_pending_order` (no Stripe call), the in-claim
/// `authorize_and_activate` (must not touch a second pool) — keep
/// explicit parameters so the signature still shows the absence.
#[derive(Clone, Copy)]
pub struct FlowDeps<'a> {
    pub worker_pool: &'a crate::WorkerPool,
    pub time_source: &'a TimeSource,
    pub stripe_service: &'a StripeService,
}

/// Place the bid for (space, round, user), minting or raising a card
/// authorization first when the bid exceeds balance backing
/// (backed_credits mode) — the `create_bid` route's entry point.
/// Balance-covered bids (and all bids in other modes) take the ordinary
/// single-transaction `store::create_bid` path with no claim.
///
/// The card path is the order → execute sequence (see the module docs),
/// then the same ordinary bid transaction the first attempt already ran
/// — an authorization without its bid is a legal state, so the bid
/// needs no atomicity with the claim, and a crashed gap re-converges on
/// the member's re-click: the first attempt simply succeeds against the
/// enlarged backing, with no second Stripe call.
pub async fn create_bid_with_funding(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
    deps: FlowDeps<'_>,
) -> Result<(), StoreError> {
    let (space, _) = store::get_validated_space(
        space_id,
        user_id,
        payloads::PermissionLevel::Member,
        pool,
    )
    .await?;
    let community_id =
        store::get_site_community_id(&space.site_id, pool).await?;
    let backed = funding::is_backed_mode(&community_id, pool).await?;
    // Only in backed mode can the bid transaction's blocking pair-lock
    // wait queue behind a Stripe-spanning claim, so only there does it
    // draw from the worker pool (see the module docs).
    let bid_pool = if backed { &deps.worker_pool.0 } else { pool };
    let time_source = deps.time_source;

    // Attempt first: balance-covered, informal-mode, and chore bids
    // succeed here with no card machinery touched, and every bid-time
    // validation rejection (caps, eligibility, liveness, ...) returns
    // before any authorization exists. Only the backed-mode shortfall
    // continues into the card path below.
    match store::create_bid(space_id, round_id, user_id, bid_pool, time_source)
        .await
    {
        Err(StoreError::Api(ApiError::InsufficientBalance)) if backed => {}
        result => return result,
    }
    // The failed attempt already proved the round exists.
    let auction_id: AuctionId = sqlx::query_scalar(
        "SELECT auction_id FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await?;

    // Card path. Prerequisites produce contextual errors prompting
    // setup/grant in-context; a community without a charges-enabled
    // account offers no card path, so the bid is honestly short.
    let ctx = load_card_context(&community_id, user_id, pool)
        .await
        .map_err(|e| match e {
            StoreError::Api(ApiError::CardPaymentsNotEnabled) => {
                StoreError::Api(ApiError::InsufficientBalance)
            }
            e => e,
        })?;

    // Order tx: verify and size the need once, under the pair lock, and
    // commit it as the pending intent — the authorization order the
    // execute claim replays. Member-present, so acquire blocking; on the
    // worker pool, since a blocking wait can queue behind a
    // Stripe-spanning claim for that call's duration.
    let sizing = ctx.sizing.clone();
    let order = place_auth_order(
        &deps.worker_pool.0,
        &community_id,
        &auction_id,
        user_id,
        LockWait::Block,
        time_source,
        |mut locks| async move {
            // Fresh plan under the lock: balance may have moved while we waited
            // (a double-click's second pass re-checks here, sees the first's
            // authorization, and orders nothing).
            let (_, gap) = bid_card_gap_tx(
                &space,
                &community_id,
                round_id,
                user_id,
                time_source,
                locks.tx(),
            )
            .await?;
            let Some(view) = gap else {
                return Ok((locks, None));
            };
            let amount = size_authorization(
                &sizing,
                &auction_id,
                user_id,
                &view,
                locks.tx(),
            )
            .await?;
            Ok((
                locks,
                Some(view.order(FundingIntentOrigin::BidFlow, amount)),
            ))
        },
    )
    .await?;

    if matches!(order, OrderOutcome::Ordered)
        && let AuthOrderOutcome::Declined(decline) = execute_auth_order(
            &ctx,
            &community_id,
            &auction_id,
            user_id,
            Presence::MemberPresent,
            deps,
        )
        .await?
    {
        // Failed raises are metadata: the decline record committed
        // with the order row, any prior auth is untouched.
        return Err(ApiError::CardDeclined {
            code: decline.best_code().map(String::from),
        }
        .into());
    }

    // Retry the bid against the committed backing. The first attempt's
    // InsufficientBalance was the expected card-path trigger; this one
    // means the balance genuinely moved between plan and bid — the
    // enlarged auth is persisted, so the member's re-click sizes a
    // smaller (usually zero) gap and succeeds.
    match store::create_bid(space_id, round_id, user_id, bid_pool, time_source)
        .await
    {
        Ok(()) => Ok(()),
        Err(StoreError::Api(ApiError::InsufficientBalance)) => {
            Err(ApiError::BalanceChangedDuringProcessing.into())
        }
        Err(e) => Err(e),
    }
}

/// Ensure the member's live authorization for an auction covers at
/// least `requested` (or the member's strategy-sized default), minting
/// or swap-raising as needed — the `authorize_funding` route's entry
/// point (the pre-authorize button). Member-present, so declines
/// surface directly as `CardDeclined`.
pub async fn authorize_funding(
    auction_id: &AuctionId,
    user_id: &UserId,
    requested: Option<Decimal>,
    pool: &PgPool,
    deps: FlowDeps<'_>,
) -> Result<(), StoreError> {
    let (auction, _) = store::auction::get_validated_auction(
        auction_id,
        user_id,
        payloads::PermissionLevel::Member,
        pool,
    )
    .await?;
    if auction.end_at.is_some() {
        return Err(ApiError::AuctionAlreadyEnded.into());
    }
    // Advance gate: authorization opens the uniform PREAUTH_ADVANCE
    // (24h) before a known start — the same advance the scheduled task
    // uses, leaving 12h of postponement slack before a minted hold is
    // invalidated. Start-unknown auctions allow authorization anytime
    // (the age scan recycles holds that linger past their viable
    // window).
    if let Some(start_at) = auction.start_at {
        let authorize_from = start_at
            .checked_sub(funding::preauth_advance())
            .map_err(anyhow::Error::from)?;
        if deps.time_source.now() < authorize_from {
            return Err(ApiError::PreauthNotYetOpen { authorize_from }.into());
        }
    }
    let community_id =
        store::get_site_community_id(&auction.site_id, pool).await?;
    if !funding::is_backed_mode(&community_id, pool).await? {
        return Err(ApiError::CardPaymentsNotEnabled.into());
    }
    let ctx = load_card_context(&community_id, user_id, pool).await?;
    if let Some(amount) = requested {
        if amount <= Decimal::ZERO {
            return Err(ApiError::AmountMustBePositive.into());
        }
        // Validate against the card-charge maximum before ordering: an
        // over-limit amount committed into a pending order would fail
        // at Stripe with a permanent error on every replay, poisoning
        // the member's card path until the aged-order arm cancels it.
        if amount > ctx.sizing.stripe_max_charge {
            return Err(ApiError::AmountTooLarge {
                max: ctx.sizing.stripe_max_charge,
            }
            .into());
        }
    }

    match run_preauth(
        &ctx,
        &community_id,
        auction_id,
        user_id,
        requested,
        FundingIntentOrigin::MemberPreauth,
        Presence::MemberPresent,
        deps,
    )
    .await?
    {
        AuthOrderOutcome::Declined(decline) => Err(ApiError::CardDeclined {
            code: decline.best_code().map(String::from),
        }
        .into()),
        _ => Ok(()),
    }
}

/// Whether a member is driving the request or it runs unattended (the
/// proxy bidding worker, the scheduled pre-auth task). Presence decides
/// both lock behavior — members block on the pair lock, workers try
/// and skip (see CONCURRENCY.md) — and how a decline reaches the
/// member: automatic contexts enqueue the card-action-needed
/// notification, member-present flows surface the error directly.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Presence {
    /// A member is waiting on the response.
    MemberPresent,
    /// A worker context with no member watching.
    Automatic,
}

impl Presence {
    pub(crate) fn lock_wait(self) -> LockWait {
        match self {
            Presence::MemberPresent => LockWait::Block,
            Presence::Automatic => LockWait::Try,
        }
    }

    pub(crate) fn notifies_decline(self) -> bool {
        matches!(self, Presence::Automatic)
    }
}

/// Verify, size, and execute a pre-authorization for one (auction,
/// member).
///
/// The shared core of the member pre-authorize endpoint
/// (member-present, origin `member_preauth`) and the scheduler's T−24h
/// task (automatic, origin `scheduled_preauth`). Sizes per the member's
/// strategy when `requested` is None.
///
/// A member-present request resizes the hold exactly to the target in
/// either direction (reductions floored at the commitment's card need);
/// automatic origins only raise, no-opping (`NothingToDo`) when the
/// live authorization already covers the target, so re-running each
/// tick is cheap and idempotent.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_preauth(
    ctx: &CardContext,
    community_id: &CommunityId,
    auction_id: &AuctionId,
    user_id: &UserId,
    requested: Option<Decimal>,
    origin: FundingIntentOrigin,
    presence: Presence,
    deps: FlowDeps<'_>,
) -> Result<AuthOrderOutcome, StoreError> {
    // Order tx: verify and size the request once, under the pair lock,
    // and commit it as the pending intent (the authorization order).
    let time_source = deps.time_source;
    let order = place_auth_order(
        &deps.worker_pool.0,
        community_id,
        auction_id,
        user_id,
        presence.lock_wait(),
        time_source,
        |mut locks| async move {
            let commitment_here = store::currency::get_auction_commitment_tx(
                user_id,
                auction_id,
                locks.tx(),
            )
            .await?;
            let view = gap_view_tx(
                community_id,
                auction_id,
                user_id,
                commitment_here,
                time_source,
                locks.tx(),
            )
            .await?;

            // Target: the member's explicit amount, or the strategy size.
            // Clamped to the card-charge maximum (strategy-sized targets
            // derive from unvalidated user values): a capped hold is honest
            // partial backing, an over-limit order fails permanently at
            // Stripe on every replay.
            let target = match requested {
                Some(amount) => amount.min(ctx.sizing.stripe_max_charge),
                None => match funding::strategy_preauth_amount(
                    &ctx.sizing,
                    view.need_from_card,
                    view.balance_backing,
                    auction_id,
                    user_id,
                    &mut **locks.tx(),
                )
                .await?
                {
                    Some(amount) => amount,
                    // Nothing to hold — the target is zero.
                    None => return Ok((locks, None)),
                },
            };
            let prior = view.live_auth_amount();
            let amount = if origin == FundingIntentOrigin::MemberPreauth {
                // Member-present resize: sized exactly at the target in both
                // directions — catch up a short hold or shed an over-hold
                // with one swap (the activation flip supersedes the old hold
                // and the worker cancels it; never release backing first). A
                // reduction stays above what committed bids need beyond
                // balance: strategy targets by construction, explicit
                // amounts by the replacement floor (the checkout flow's
                // rule). Exact sizing also subsumes the decline rollover: a
                // live hold whose card turned unrecoverable re-authorizes
                // fresh at the current target rather than through the
                // doubling ratchet's over-hold.
                if target < prior && target < view.need_from_card {
                    return Err(ApiError::HoldReplacementTooSmall {
                        current: prior,
                        floor: view.need_from_card,
                    }
                    .into());
                }
                target.max(ctx.sizing.stripe_min_charge)
            } else {
                // Automatic origins stay monotonic — never shrinking a hold
                // the member may have sized deliberately — and raise by
                // catch-up-or-double so swap count stays logarithmic in the
                // bid trajectory. The covered no-op compares against the
                // target, not the sized raise: doubling must not turn a
                // covered request into a swap.
                if target <= prior {
                    return Ok((locks, None));
                }
                if prior > Decimal::ZERO {
                    raise_target(prior, target)
                } else {
                    target.max(ctx.sizing.stripe_min_charge)
                }
            };
            // The raise's doubling can overshoot the clamped target.
            let amount = ceil_to_minor_units(
                amount.min(ctx.sizing.stripe_max_charge),
                ctx.sizing.minor_units,
            );
            // Already covered exactly — nothing to order; the common
            // re-click lands here.
            if amount == prior {
                return Ok((locks, None));
            }
            Ok((locks, Some(view.order(origin, amount))))
        },
    )
    .await?;
    match order {
        OrderOutcome::LockMiss => return Ok(AuthOrderOutcome::LockMiss),
        OrderOutcome::NoOrder => return Ok(AuthOrderOutcome::NothingToDo),
        OrderOutcome::Ordered => {}
    }

    execute_auth_order(ctx, community_id, auction_id, user_id, presence, deps)
        .await
}

/// Outcome of an order transaction (`place_auth_order`).
pub(crate) enum OrderOutcome {
    /// Try-lock callers only: another claimant owns the pair; nothing
    /// was examined or written.
    LockMiss,
    /// The plan found no card need (covered, inapplicable, or paused);
    /// the transaction rolled back and nothing was ordered.
    NoOrder,
    /// A sized order is committed as the pending intent; execute it.
    Ordered,
}

/// Run one order transaction for (auction, member): begin on `pool`,
/// acquire the pair lock per `wait`, and run `plan` — each flow's own
/// verification and sizing under the lock — committing its result via
/// `ensure_pending_intent_tx` as the pending intent, the immutable
/// authorization order `execute_auth_order` replays. A `None` plan
/// rolls back and orders nothing. The flows (bid route, pre-authorize)
/// differ only in their plan (round liveness, coverage targets); the
/// proxy path orders inside its bidding claim instead
/// (`order_proxy_bid_auth_tx`).
///
/// `plan` takes the `TrackedTx` by value and hands it back (an error
/// return drops it, rolling back): a `&mut` argument would make the
/// closure bound higher-ranked over the borrow's lifetime, which async
/// closures can't satisfy through a spawned task's `Send` check — the
/// by-value shape keeps every lifetime concrete.
async fn place_auth_order<F>(
    pool: &PgPool,
    community_id: &CommunityId,
    auction_id: &AuctionId,
    user_id: &UserId,
    wait: LockWait,
    time_source: &TimeSource,
    plan: impl FnOnce(TrackedTx<'static, 'static>) -> F,
) -> Result<OrderOutcome, StoreError>
where
    F: Future<
        Output = Result<
            (TrackedTx<'static, 'static>, Option<AuthOrder>),
            StoreError,
        >,
    >,
{
    let mut locks = TrackedTx::begin(pool).await?;
    if !locks.acquire_pair(auction_id, user_id, wait).await? {
        return Ok(OrderOutcome::LockMiss);
    }
    let (mut locks, planned) = plan(locks).await?;
    let Some(order) = planned else {
        locks.rollback().await?;
        return Ok(OrderOutcome::NoOrder);
    };
    ensure_pending_intent_tx(
        community_id,
        auction_id,
        user_id,
        &order,
        time_source,
        &mut locks,
    )
    .await?;
    locks.commit().await?;
    Ok(OrderOutcome::Ordered)
}

/// Order the authorization a proxy bid's funding shortfall calls for,
/// inside the bidding claim's transaction — the reactive counterpart of
/// the member-present bid flow's order transaction.
///
/// The claim's failed `create_bid_tx` proved the concrete bid is
/// placeable (eligibility and liveness passed) and needs card backing;
/// this re-derives its gap under the same snapshot (`bid_card_gap_tx`,
/// the shape the bid flow uses), sizes the raise, and commits the
/// pending intent `execute_auth_order` replays after the claim commits.
///
/// Returns whether an order was written — false when automatic raises
/// are paused by a recent decline in the funding lineage (a declining
/// card would otherwise churn futile swap attempts round after round;
/// member-present attempts clear it on success), or when the gap
/// unexpectedly reads closed.
///
/// Lock contract: runs inside the bidding claim's savepoint — the pair
/// lock is held (key-checked at the intent write); writes the single
/// pending intent row.
pub(crate) async fn order_proxy_bid_auth_tx(
    community_id: &CommunityId,
    space: &store::Space,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    time_source: &TimeSource,
    locks: &mut TrackedTx<'_, '_>,
) -> Result<bool, StoreError> {
    let (round, gap) = bid_card_gap_tx(
        space,
        community_id,
        round_id,
        user_id,
        time_source,
        locks.tx(),
    )
    .await?;
    let Some(view) = gap else {
        tracing::warn!(
            space_id = ?space.id,
            user_id = ?user_id,
            "proxy bid failed on funding but its gap reads closed; \
             skipping the order",
        );
        return Ok(false);
    };

    let auction_id = round.auction_id;
    if let Some(latest) =
        store::funding::latest_intent(&auction_id, user_id, &mut **locks.tx())
            .await?
        && latest.last_decline_at.is_some()
    {
        return Ok(false);
    }

    let sizing = load_sizing_context(community_id, user_id, locks.tx()).await?;
    let amount =
        size_authorization(&sizing, &auction_id, user_id, &view, locks.tx())
            .await?;
    ensure_pending_intent_tx(
        community_id,
        &auction_id,
        user_id,
        &view.order(FundingIntentOrigin::BidFlow, amount),
        time_source,
        locks,
    )
    .await?;
    tracing::info!(
        space_id = ?space.id,
        user_id = ?user_id,
        %amount,
        "proxy bid short on funding; authorization ordered",
    );
    Ok(true)
}

/// Outcome of executing a member's committed authorization order.
pub(crate) enum AuthOrderOutcome {
    /// Try-lock callers only: another claimant owns the pair.
    LockMiss,
    /// No executable order — none pending (a racer consumed it), or it
    /// no longer raises backing and was canceled.
    NothingToDo,
    /// The confirm was refused; the decline is recorded on the order.
    Declined(DeclineInfo),
    /// The authorization is live.
    Authorized,
}

/// Execute the (auction, member)'s pending authorization order, if any.
///
/// Under a fresh pair-lock claim on the worker pool, re-read the order
/// and the live authorization, and — when the order still raises
/// backing — create+confirm it at Stripe with the committed amount and
/// activate it. The order's id and amount are both committed state, so
/// the Stripe call's parameters are a pure function of the row: a crash
/// anywhere in this claim replays identically and converges at Stripe
/// via the idempotency key.
///
/// An order staled by an interloping raise (the pair lock drops between
/// the order and execute transactions) is canceled instead of executed
/// — executing it would swap-demote the larger hold.
///
/// Lock contract: acquires the pair lock (blocking for member-present
/// callers, try for the proxy worker), then via `activate_intent_tx`
/// the processing lock and intent rows — this transaction writes intent
/// rows only.
pub(crate) async fn execute_auth_order(
    ctx: &CardContext,
    community_id: &CommunityId,
    auction_id: &AuctionId,
    user_id: &UserId,
    presence: Presence,
    deps: FlowDeps<'_>,
) -> Result<AuthOrderOutcome, StoreError> {
    let time_source = deps.time_source;
    let mut claim = TrackedTx::begin(&deps.worker_pool.0).await?;
    if !claim
        .acquire_pair(auction_id, user_id, presence.lock_wait())
        .await?
    {
        return Ok(AuthOrderOutcome::LockMiss);
    }

    let order: Option<(
        payloads::FundingIntentId,
        Decimal,
        FundingIntentOrigin,
        Option<payloads::FundingIntentId>,
    )> = sqlx::query_as(
        "SELECT id, requested_amount, origin, replaces_intent_id \
         FROM funding_intents \
         WHERE auction_id = $1 AND user_id = $2 \
           AND status = 'pending'",
    )
    .bind(auction_id)
    .bind(user_id)
    .fetch_optional(&mut **claim.tx())
    .await?;
    let Some((intent_id, amount, origin, replaces)) = order else {
        return Ok(AuthOrderOutcome::NothingToDo);
    };

    let live = active_intent(auction_id, user_id, &mut **claim.tx())
        .await?
        .filter(|i| i.is_live_auth(time_source.now()));
    let live_amount = live
        .as_ref()
        .and_then(|i| i.authorized_amount)
        .unwrap_or(Decimal::ZERO);
    if amount <= live_amount {
        // The exception is a member-preauth reduction ordered against
        // exactly the hold that is still live (`replaces_intent_id`) —
        // the deliberate resize. Anything else is stale: a raise the
        // live hold outgrew while the order waited (checkout adoption
        // takes no pair lock), or a reduction whose target hold was
        // itself replaced — keep-larger wins there.
        let deliberate_reduction = amount < live_amount
            && origin == FundingIntentOrigin::MemberPreauth
            && replaces.is_some()
            && replaces == live.as_ref().map(|i| i.id);
        if !deliberate_reduction {
            cancel_stale_order_tx(&intent_id, time_source, claim.tx()).await?;
            claim.commit().await?;
            tracing::info!(
                %intent_id,
                %amount,
                live = %live_amount,
                "authorization order superseded by a larger live hold; \
                 canceled"
            );
            return Ok(AuthOrderOutcome::NothingToDo);
        }
    }

    let outcome = authorize_and_activate(
        ctx,
        community_id,
        auction_id,
        user_id,
        &intent_id,
        amount,
        presence,
        time_source,
        deps.stripe_service,
        &mut claim,
    )
    .await?;
    claim.commit().await?;
    match outcome {
        ActivationOutcome::Completed => Ok(AuthOrderOutcome::Authorized),
        ActivationOutcome::Declined(decline) => {
            Ok(AuthOrderOutcome::Declined(decline))
        }
        // The cancel-in-place committed with the claim; the operation
        // still failed.
        ActivationOutcome::Unexecutable(msg) => {
            Err(StoreError::StripeError(msg))
        }
    }
}

/// The synthetic decline code recorded when a fresh authorization's
/// capture window can't cover the auction's fixed deadline plus capture
/// margin (a card network window below the assumed floor, or an auction
/// configured longer than the card supports). Decline-shaped on
/// purpose: manual flows get a contextual error and automatic retries
/// pause, uniformly with real declines.
pub(crate) const HOLD_WINDOW_TOO_SHORT: &str = "hold_window_too_short";

/// Outcome of `authorize_and_activate`, committed with the claim
/// transaction.
pub(crate) enum ActivationOutcome {
    /// The authorization is live (or the row had already converged
    /// terminally mid-flight; nothing further to record either way).
    Completed,
    /// The confirm was refused; the decline is recorded on the order.
    Declined(DeclineInfo),
    /// The order's create can never succeed (a permanent request
    /// error, e.g. `amount_too_large`); the order row was canceled in
    /// place — not a decline: no pause, no card-problem email. The
    /// caller commits the claim, then surfaces the error.
    Unexecutable(String),
}

/// Create+confirm an authorization of `amount` at Stripe and activate
/// its intent row in the claim transaction — the execute claim's final
/// leg (`execute_auth_order`).
///
/// Returns `Declined` on a refused confirm, recorded on the pending row
/// in the same transaction. The flows decide whether a decline bails
/// (manual) or continues placing balance-backed bids (proxy); an
/// `Automatic` presence additionally enqueues the card-action-needed
/// notification (member-present flows surface the error directly
/// instead). Transient/conflict Stripe errors propagate as
/// `StripeError` (the claim rolls back and the retry replays by
/// idempotency key); permanent request errors terminalize the order
/// (`Unexecutable`).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn authorize_and_activate(
    ctx: &CardContext,
    community_id: &CommunityId,
    auction_id: &AuctionId,
    user_id: &UserId,
    intent_id: &payloads::FundingIntentId,
    amount: Decimal,
    presence: Presence,
    time_source: &TimeSource,
    stripe_service: &StripeService,
    claim: &mut TrackedTx<'_, '_>,
) -> Result<ActivationOutcome, StoreError> {
    let amount_minor = payloads::to_minor_units(amount, ctx.sizing.minor_units)
        .ok_or_else(|| {
            anyhow::anyhow!("authorization amount {amount} not quantized")
        })?;
    let metadata = HashMap::from([
        ("funding_intent_id".to_string(), intent_id.to_string()),
        ("auction_id".to_string(), auction_id.to_string()),
        ("user_id".to_string(), user_id.to_string()),
        ("community_id".to_string(), community_id.to_string()),
    ]);

    let result = stripe_service
        .create_authorization(AuthorizationParams {
            connected_account_id: &ctx.connected_account_id,
            platform_customer_id: &ctx.customer_id,
            platform_payment_method_id: &ctx.payment_method_id,
            amount_minor,
            currency: &ctx.sizing.currency,
            metadata,
            idempotency_seed: &intent_id.to_string(),
        })
        .await;

    match result {
        Ok(auth) => {
            let capture_before = funding::capture_before_or_floor(
                auth.capture_before_epoch,
                intent_id,
                &auth.payment_intent_id,
                time_source,
            )?;
            // Short-window safety net: the window must cover the
            // auction's deadline (start + runtime, fixed at creation
            // and never shortened by an arriving auth; now-based while
            // the start is unknown) plus capture margin. Later-minted
            // auths need less remaining window, so a short-window card
            // may legally back late-auction raises even when it can't
            // pre-authorize — same predicate, no special case.
            let window_ok = funding::hold_window_covers_auction(
                auction_id,
                capture_before,
                time_source.now(),
                &mut **claim.tx(),
            )
            .await?;
            // Our own confirm's `amount_capturable_updated` webhook can
            // be delivered — and adopt the row — before the confirm
            // response returns. Adoption evaluates the same window
            // predicate from the same retrieved window, so on a
            // disagreement (only the seconds between the two `now`s
            // while the start is unknown) an adopted activation is
            // accepted rather than canceled out from under the member —
            // noise against the capture margin.
            let adopted: bool = !window_ok
                && sqlx::query_scalar(
                    "SELECT status = 'authorized' FROM funding_intents \
                     WHERE id = $1",
                )
                .bind(intent_id)
                .fetch_one(&mut **claim.tx())
                .await?;
            if !window_ok && !adopted {
                // Cancel-and-reject: release the fresh hold at Stripe,
                // record a decline-shaped refusal on the order. A Stripe
                // failure here propagates — the claim rolls back, and
                // the retry replays create-by-key to the same object and
                // cancels again.
                //
                // The record's guard includes `authorized` for the
                // sliver where adoption activates the row between the
                // read just above and this write: the Stripe object is
                // canceled either way, and the later `canceled` webhook
                // would converge the status but never write the decline
                // metadata that pauses automatic retries. (The genuine
                // decline path below keeps the narrow `pending` guard:
                // declined confirms fire no
                // `amount_capturable_updated`, so no adoption can
                // precede them, and a first-arriving `payment_failed`
                // webhook already records the same metadata.)
                funding::cancel_hold(
                    &ctx.connected_account_id,
                    &auth.payment_intent_id,
                    intent_id,
                    stripe_service,
                )
                .await
                .map_err(|e| StoreError::StripeError(e.to_string()))?;
                record_intent_decline(
                    intent_id,
                    Some(&auth.payment_intent_id),
                    Some(HOLD_WINDOW_TOO_SHORT),
                    AdoptedRows::Include,
                    time_source,
                    &mut **claim.tx(),
                )
                .await?;
                if presence.notifies_decline() {
                    enqueue_decline_notification(
                        community_id,
                        auction_id,
                        user_id,
                        intent_id,
                        Some(HOLD_WINDOW_TOO_SHORT),
                        time_source,
                        claim.tx(),
                    )
                    .await?;
                }
                tracing::warn!(
                    %intent_id,
                    payment_intent_id = %auth.payment_intent_id,
                    %capture_before,
                    "authorization window can't cover the auction \
                     deadline; canceled and recorded as a decline"
                );
                return Ok(ActivationOutcome::Declined(DeclineInfo {
                    code: Some(HOLD_WINDOW_TOO_SHORT.to_string()),
                    decline_code: None,
                    message: None,
                    payment_intent_id: Some(auth.payment_intent_id),
                }));
            }
            if !window_ok {
                tracing::info!(
                    %intent_id,
                    payment_intent_id = %auth.payment_intent_id,
                    %capture_before,
                    "short-window auth was already adopted by its \
                     webhook; accepting the adoption"
                );
            }
            match funding::activate_intent_tx(
                intent_id,
                &auth.payment_intent_id,
                payloads::from_minor_units(
                    auth.amount_minor,
                    ctx.sizing.minor_units,
                ),
                capture_before,
                AdoptedRows::Include,
                time_source,
                claim,
            )
            .await?
            {
                ActivateOutcome::RowMovedOn => {
                    // The row converged to a terminal status mid-flight
                    // (an out-of-band cancel's webhook adopted it); the
                    // Stripe object is canceled or converging via its
                    // own events, so there is nothing to record here.
                    tracing::warn!(
                        %intent_id,
                        payment_intent_id = %auth.payment_intent_id,
                        "intent row reached a terminal status before \
                         activation; leaving it as converged"
                    );
                    return Ok(ActivationOutcome::Completed);
                }
                ActivateOutcome::ShrinkRefused => {
                    // A larger hold activated between the claim's
                    // stale-order check and here — checkout adoption
                    // takes no pair lock, so it can interleave with the
                    // Stripe confirm. (A member resize whose floor rose
                    // or whose target hold was replaced mid-flight
                    // lands here too.) The member is better backed than
                    // this order would leave them: cancel the fresh
                    // hold and the order in place, decline-free (no
                    // card problem, no automatic-raise pause).
                    funding::cancel_hold(
                        &ctx.connected_account_id,
                        &auth.payment_intent_id,
                        intent_id,
                        stripe_service,
                    )
                    .await
                    .map_err(|e| StoreError::StripeError(e.to_string()))?;
                    cancel_stale_order_tx(intent_id, time_source, claim.tx())
                        .await?;
                    tracing::info!(
                        %intent_id,
                        payment_intent_id = %auth.payment_intent_id,
                        "order superseded by a larger live hold during \
                         execution; canceled the fresh hold"
                    );
                    return Ok(ActivationOutcome::Completed);
                }
                ActivateOutcome::Promoted => {}
            }
            tracing::info!(
                %intent_id,
                payment_intent_id = %auth.payment_intent_id,
                amount_minor = auth.amount_minor,
                "funding authorization activated"
            );
            Ok(ActivationOutcome::Completed)
        }
        Err(StripeCallError::Declined(decline)) => {
            record_intent_decline(
                intent_id,
                decline.payment_intent_id.as_deref(),
                decline.best_code(),
                AdoptedRows::Exclude,
                time_source,
                &mut **claim.tx(),
            )
            .await?;
            if presence.notifies_decline() {
                enqueue_decline_notification(
                    community_id,
                    auction_id,
                    user_id,
                    intent_id,
                    decline.best_code(),
                    time_source,
                    claim.tx(),
                )
                .await?;
            }
            tracing::warn!(
                %intent_id,
                code = ?decline.code,
                decline_code = ?decline.decline_code,
                "funding authorization declined"
            );
            Ok(ActivationOutcome::Declined(decline))
        }
        Err(StripeCallError::PermanentRequest(e)) => {
            // Retrying is futile — the same parameters fail the same
            // way on every replay — so terminalize the order in place
            // (upfront amount validation makes this arm unexpected;
            // reaching it means a validation gap or malformed row).
            cancel_stale_order_tx(intent_id, time_source, claim.tx()).await?;
            tracing::error!(
                %intent_id,
                "authorization order unexecutable; canceled in place: {e:#}"
            );
            Ok(ActivationOutcome::Unexecutable(format!(
                "authorization order {intent_id} unexecutable: {e:#}"
            )))
        }
        Err(e) => Err(StoreError::StripeError(e.to_string())),
    }
}

/// Enqueue the card-action-needed notification for a decline in an
/// automatic (off-session) context, in the claim transaction — the
/// member hears once that automatic card holds are paused (the dedup
/// key is the declined order's intent id; the decline pause prevents
/// repeat attempts, so repeat emails need no further guard).
async fn enqueue_decline_notification(
    community_id: &CommunityId,
    auction_id: &AuctionId,
    user_id: &UserId,
    intent_id: &payloads::FundingIntentId,
    decline_code: Option<&str>,
    time_source: &TimeSource,
    claim: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    let community_name: String =
        sqlx::query_scalar("SELECT name FROM communities WHERE id = $1")
            .bind(community_id)
            .fetch_one(&mut **claim)
            .await?;
    store::notifications::enqueue_notification_tx(
        user_id,
        &format!("decline:{intent_id}"),
        &store::notifications::NotificationParams::CardActionNeeded {
            community_name,
            auction_id: *auction_id,
            decline_code: decline_code.map(String::from),
        },
        time_source,
        &mut **claim,
    )
    .await
}

/// An intent row owing a Stripe call (a cancel or a capture), as listed
/// lock-free by `scheduler::process_intent_work`.
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct WorkerIntent {
    pub(crate) id: payloads::FundingIntentId,
    pub(crate) status: payloads::FundingIntentStatus,
    pub(crate) payment_intent_id: Option<String>,
    pub(crate) capture_amount: Option<Decimal>,
    #[sqlx(try_from = "payloads::OptionalTimestamp")]
    pub(crate) capture_before: Option<jiff::Timestamp>,
    pub(crate) origin: payloads::FundingIntentOrigin,
    pub(crate) authorized_amount: Option<Decimal>,
    pub(crate) auction_id: payloads::AuctionId,
    pub(crate) community_id: payloads::CommunityId,
    pub(crate) user_id: payloads::UserId,
    pub(crate) stripe_account_id: Option<String>,
    pub(crate) currency_name: String,
}

/// Open the worker's per-row claim: begin on the worker pool, take the
/// pair try-lock, and re-verify the row still holds the status it was
/// selected with (a webhook or concurrent claim may have converged or
/// repurposed it since the lock-free selection). `None` means no claim
/// — lock contention or a moved-on row; the caller returns without
/// touching anything, and the row is re-found next tick if still due.
async fn claim_intent(
    intent: &WorkerIntent,
    worker_pool: &crate::WorkerPool,
) -> Result<Option<TrackedTx<'static, 'static>>, StoreError> {
    let mut locks = TrackedTx::begin(&worker_pool.0).await?;
    if !locks
        .acquire_pair(&intent.auction_id, &intent.user_id, LockWait::Try)
        .await?
    {
        return Ok(None);
    }
    let status_unchanged: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM funding_intents \
         WHERE id = $1 AND status = $2)",
    )
    .bind(intent.id)
    .bind(intent.status)
    .fetch_one(&mut **locks.tx())
    .await?;
    if !status_unchanged {
        return Ok(None);
    }
    Ok(Some(locks))
}

/// The worker claim's shared failure tail: record backoff on the row —
/// so it resurfaces at a bounded cadence instead of being re-selected
/// and error-logged every tick — commit the claim, and rethrow.
async fn fail_claim<T>(
    err: anyhow::Error,
    intent_id: &payloads::FundingIntentId,
    time_source: &TimeSource,
    mut locks: TrackedTx<'_, '_>,
) -> anyhow::Result<T> {
    store::funding::record_worker_failure_tx(
        intent_id,
        time_source,
        locks.tx(),
    )
    .await?;
    locks.commit().await?;
    Err(err)
}

/// Cancel one intent's authorization at Stripe and mark its row
/// `canceled`, inside a pair-lock claim on the worker pool.
///
/// Handles the `superseded`/`release_pending` arms, the ended-auction
/// catchall, and the pre-start age scan of
/// `scheduler::process_intent_work` (the age arm re-verifies pre-start
/// and short-window under the claim, and a canceled `member_preauth` hold
/// enqueues the authorize-again notice — scheduled holds cancel
/// silently, since the T−24h task re-mints them).
///
/// A Stripe failure records worker backoff instead. A row whose capture
/// window has already passed converges to `expired` locally, with no
/// Stripe call.
///
/// Lock contract: acquires the pair try-lock; the cancel mark is a single
/// status-guarded row update, so no account or funding locks are taken.
pub(crate) async fn cancel_worker_intent(
    intent: &WorkerIntent,
    deps: FlowDeps<'_>,
) -> anyhow::Result<()> {
    let time_source = deps.time_source;
    let Some(mut locks) = claim_intent(intent, deps.worker_pool).await? else {
        return Ok(());
    };

    // Re-read what the claim's status check can't cover: for the age
    // arm the auction may have started (or a raise swapped the hold)
    // since selection.
    #[derive(sqlx::FromRow)]
    struct CancelRecheck {
        is_active: bool,
        #[sqlx(try_from = "payloads::OptionalTimestamp")]
        capture_before: Option<jiff::Timestamp>,
        #[sqlx(try_from = "payloads::OptionalTimestamp")]
        auction_start_at: Option<jiff::Timestamp>,
        #[sqlx(try_from = "payloads::OptionalTimestamp")]
        auction_end_at: Option<jiff::Timestamp>,
    }
    let current: Option<CancelRecheck> = sqlx::query_as(
        "SELECT fi.is_active, fi.capture_before, \
                a.start_at AS auction_start_at, a.end_at AS auction_end_at \
         FROM funding_intents fi \
         JOIN auctions a ON fi.auction_id = a.id \
         WHERE fi.id = $1",
    )
    .bind(intent.id)
    .fetch_optional(&mut **locks.tx())
    .await?;
    let Some(current) = current else {
        return Ok(());
    };
    let mut age_cancel = false;
    if intent.status == payloads::FundingIntentStatus::Authorized {
        if !current.is_active {
            return Ok(());
        }
        if current.auction_end_at.is_some() {
            tracing::warn!(
                intent_id = ?intent.id,
                auction_id = ?intent.auction_id,
                "authorization activated after its auction ended; releasing"
            );
        } else {
            // Age arm: cancel only while the auction is still pre-start
            // and the hold's window still can't cover the deadline —
            // the same predicate as the selection and the mint site
            // (`hold_window_covers_auction`).
            let now = time_source.now();
            if current.auction_start_at.is_some_and(|s| s <= now) {
                return Ok(());
            }
            let Some(capture_before) = current.capture_before else {
                return Ok(());
            };
            if funding::hold_window_covers_auction(
                &intent.auction_id,
                capture_before,
                now,
                &mut **locks.tx(),
            )
            .await?
            {
                return Ok(());
            }
            age_cancel = true;
        }
    }

    // A hold whose capture window has passed is already released at
    // Stripe (or died with a replaced connected account) — no cancel
    // call can change anything, so converge locally. Without this,
    // rows whose PaymentIntent no call can reach (e.g. stranded by a
    // connected-account replacement) would retry the cancel forever.
    if current
        .capture_before
        .is_some_and(|t| t <= time_source.now())
    {
        store::funding::mark_intent_canceled_tx(
            &intent.id,
            intent.status,
            payloads::FundingIntentStatus::Expired,
            time_source,
            locks.tx(),
        )
        .await?;
        pubsub::emit(
            locks.tx(),
            &payloads::AuctionEvent::FundingChanged {
                auction_id: intent.auction_id,
                user_id: intent.user_id,
            },
        )
        .await?;
        locks.commit().await?;
        tracing::info!(
            intent_id = ?intent.id,
            payment_intent_id = ?intent.payment_intent_id,
            prior_status = %intent.status,
            "hold window already expired; converged locally without a \
             Stripe call"
        );
        return Ok(());
    }

    let (Some(payment_intent_id), Some(account_id)) =
        (&intent.payment_intent_id, &intent.stripe_account_id)
    else {
        // Degenerate row (no Stripe object to cancel) — nothing holds.
        store::funding::mark_intent_canceled_tx(
            &intent.id,
            intent.status,
            payloads::FundingIntentStatus::Canceled,
            time_source,
            locks.tx(),
        )
        .await?;
        locks.commit().await?;
        return Ok(());
    };

    match deps
        .stripe_service
        .cancel_payment_intent(
            account_id,
            payment_intent_id,
            &funding::cancel_idempotency_key(&intent.id),
        )
        .await
    {
        Ok(()) => {
            store::funding::mark_intent_canceled_tx(
                &intent.id,
                intent.status,
                payloads::FundingIntentStatus::Canceled,
                time_source,
                locks.tx(),
            )
            .await?;
            if age_cancel
                && intent.origin == payloads::FundingIntentOrigin::MemberPreauth
            {
                // Member-initiated holds notify once ("authorize again
                // when the start is announced"); scheduled holds cancel
                // silently — the T−24h task re-mints them, and the
                // member hears only if that re-auth declines.
                funding::notify_authorization_expiring_tx(
                    &intent.id,
                    &intent.auction_id,
                    &intent.community_id,
                    &intent.user_id,
                    intent.authorized_amount.unwrap_or_default(),
                    time_source,
                    locks.tx(),
                )
                .await?;
            }
            pubsub::emit(
                locks.tx(),
                &payloads::AuctionEvent::FundingChanged {
                    auction_id: intent.auction_id,
                    user_id: intent.user_id,
                },
            )
            .await?;
            locks.commit().await?;
            tracing::info!(
                intent_id = ?intent.id,
                %payment_intent_id,
                prior_status = %intent.status,
                age_cancel,
                "authorization canceled"
            );
            Ok(())
        }
        // Not in a cancelable state — already canceled or expired
        // (missed webhook), or captured from the dashboard. Retrieve
        // live truth and let the convergence table own the transition,
        // uniformly with the webhook pokes and reconciliation repair.
        Err(StripeCallError::StateConflict { .. }) => {
            converge_worker_conflict(
                intent,
                account_id,
                payment_intent_id,
                locks,
                deps,
            )
            .await
        }
        Err(e) => fail_claim(e.into(), &intent.id, time_source, locks).await,
    }
}

/// Converge a worker-claimed intent row after a cancel/capture call
/// came back `StateConflict`: retrieve the PaymentIntent live (only the
/// advisory pair lock is held), apply the convergence table's
/// transition inside the same claim, commit, then run the shared
/// post-commit follow-ups.
///
/// The worker must not use the standalone `converge_intent` wrapper
/// here — it would try-lock the pair on a second connection and always
/// miss against this claim. An `Anomaly` records worker backoff in the
/// claim so the row resurfaces at a bounded cadence instead of
/// error-logging every tick.
async fn converge_worker_conflict(
    intent: &WorkerIntent,
    account_id: &str,
    payment_intent_id: &str,
    mut locks: TrackedTx<'_, '_>,
    deps: FlowDeps<'_>,
) -> anyhow::Result<()> {
    let time_source = deps.time_source;
    let live = match deps
        .stripe_service
        .retrieve_payment_intent(account_id, payment_intent_id)
        .await
    {
        Ok(live) => live,
        Err(e) => {
            return fail_claim(e, &intent.id, time_source, locks).await;
        }
    };
    let outcome = store::convergence::converge_intent_tx(
        &intent.id,
        live.as_ref(),
        time_source,
        &mut locks,
    )
    .await?;
    // Anomalies and still-transitional pairs (e.g. a capture stuck
    // `processing`) resurface on worker backoff, not at tick rate —
    // the same pacing the plain-error arm records.
    if matches!(
        outcome,
        store::convergence::ConvergeOutcome::Anomaly { .. }
            | store::convergence::ConvergeOutcome::Waiting
    ) {
        store::funding::record_worker_failure_tx(
            &intent.id,
            time_source,
            locks.tx(),
        )
        .await?;
    }
    locks.commit().await?;
    let report = store::convergence::finish_converge(
        outcome,
        &intent.id,
        account_id,
        store::convergence::ConvergePool::worker(deps.worker_pool),
        time_source,
        deps.stripe_service,
    )
    .await?;
    match report {
        store::convergence::ConvergeReport::Anomaly { detail } => {
            tracing::error!(
                intent_id = ?intent.id,
                %payment_intent_id,
                prior_status = %intent.status,
                detail,
                "worker call conflicted and convergence found an \
                 anomalous pair"
            );
        }
        report => {
            tracing::info!(
                intent_id = ?intent.id,
                %payment_intent_id,
                prior_status = %intent.status,
                ?report,
                "worker call conflicted; converged from live state"
            );
        }
    }
    Ok(())
}

/// Cancel one stranded `pending` authorization order — a row whose
/// execute never ran (crash between order and execute, a proxy try-lock
/// miss on the round's last pass, a card removed after ordering) —
/// inside a pair-lock claim on the worker pool.
///
/// Left alone, the row's stale amount is reusable forever: orders are
/// immutable in place, so `ensure_pending_intent_tx` would hand a
/// much-later small raise the old large amount (it cancels undersized
/// rows itself, but a covering row it reuses).
///
/// Pure DB, no Stripe call: an execute that did reach Stripe converges
/// via webhook adoption well inside the age threshold, and the canceled
/// no-PI row feeds the orphaned-hold sweep, which resolves any
/// Stripe-side hold by metadata probe. The next genuine need orders
/// fresh.
///
/// Lock contract: acquires the pair try-lock; the cancel is a single
/// status-guarded row update.
pub(crate) async fn cancel_aged_pending_order(
    intent: &WorkerIntent,
    worker_pool: &crate::WorkerPool,
    time_source: &TimeSource,
) -> anyhow::Result<()> {
    let Some(mut locks) = claim_intent(intent, worker_pool).await? else {
        return Ok(());
    };

    cancel_stale_order_tx(&intent.id, time_source, locks.tx()).await?;
    locks.commit().await?;
    tracing::info!(
        intent_id = ?intent.id,
        auction_id = ?intent.auction_id,
        user_id = ?intent.user_id,
        "pending authorization order aged out unexecuted; canceled"
    );
    Ok(())
}

/// Capture one `capture_pending` intent at Stripe (with the platform
/// fee) and finalize in the claim tx: a treasury→member
/// `stripe_payment` issuance entry returns the settlement debit, and
/// the row is marked `captured`.
///
/// A Stripe failure records worker backoff and retries next tick;
/// pre-Stripe validation failures (a malformed row) record the same
/// backoff, staying non-terminal — the member genuinely owes the amount
/// — with reconciliation's overdue-capture check flagging the row once
/// its window passes.
///
/// An expired capture window does not preempt the Stripe call: a
/// capture that succeeded at Stripe before a crashed commit converges
/// to `captured` via the idempotency-key replay, while a hold that
/// genuinely expired comes back canceled from Stripe (`HoldGone`) —
/// terminal, so the row fails and the member's allocations re-fit
/// (`store::funding::fail_capture_tx`).
///
/// Lock contract: acquires the pair try-lock, then on the success path the
/// treasury and member account locks (issuance entry) before the intent row
/// mark; the gone-hold path follows `fail_capture_tx`'s contract instead.
pub(crate) async fn capture_intent(
    intent: &WorkerIntent,
    deps: FlowDeps<'_>,
) -> anyhow::Result<()> {
    let time_source = deps.time_source;
    let Some(mut locks) = claim_intent(intent, deps.worker_pool).await? else {
        return Ok(());
    };

    // An expired window is still attempted: auth sizing guarantees the
    // window covers the auction's runtime, so getting here late means
    // delayed retries — possibly of a capture that already succeeded at
    // Stripe before a crashed commit, which the idempotency-key replay
    // converges to `captured`. A hold that genuinely expired comes back
    // canceled from Stripe and fails terminally below.
    if intent
        .capture_before
        .is_some_and(|t| t <= time_source.now())
    {
        tracing::warn!(
            intent_id = ?intent.id,
            user_id = ?intent.user_id,
            amount = ?intent.capture_amount,
            capture_before = ?intent.capture_before,
            "capture window expired; attempting capture anyway"
        );
    }

    let CaptureParams {
        payment_intent_id,
        account_id,
        amount,
        amount_minor,
        fee,
        fee_minor,
    } = match capture_params(intent) {
        Ok(params) => params,
        Err(e) => {
            return fail_claim(e, &intent.id, time_source, locks).await;
        }
    };

    match deps
        .stripe_service
        .capture_payment_intent(
            account_id,
            payment_intent_id,
            amount_minor,
            fee_minor,
            &format!("{}:capture", intent.id),
        )
        .await
    {
        // Not awaiting capture — the hold was canceled or expired at
        // Stripe (missed webhook; fails the capture to member debt) or
        // was already captured out-of-band from the dashboard (books
        // the collected amount). Retrieve live truth and let the
        // convergence table own the transition.
        Err(StripeCallError::StateConflict { .. }) => {
            converge_worker_conflict(
                intent,
                account_id,
                payment_intent_id,
                locks,
                deps,
            )
            .await
        }
        Ok(()) => {
            store::funding::mark_intent_captured_tx(
                &intent.id,
                time_source,
                locks.tx(),
            )
            .await?;
            funding::book_capture_tx(
                &intent.id,
                &intent.auction_id,
                &intent.community_id,
                &intent.user_id,
                amount,
                payment_intent_id,
                time_source,
                &mut locks,
            )
            .await?;
            locks.commit().await?;
            tracing::info!(
                intent_id = ?intent.id,
                %payment_intent_id,
                %amount,
                %fee,
                "authorization captured and credited"
            );
            Ok(())
        }
        Err(e) => fail_claim(e.into(), &intent.id, time_source, locks).await,
    }
}

/// Stripe call inputs validated out of a `capture_pending` row by
/// [`capture_params`].
struct CaptureParams<'a> {
    payment_intent_id: &'a str,
    account_id: &'a str,
    amount: Decimal,
    amount_minor: i64,
    fee: Decimal,
    fee_minor: Option<i64>,
}

/// Validate a `capture_pending` row's Stripe call inputs: the Stripe
/// ids and amount are present, and the amount and platform fee quantize
/// to the currency's minor units. Failures are invariant violations —
/// schema CHECKs guarantee the row's own columns, and backed-credits
/// denominations are immutable — but the caller still records worker
/// backoff on them, so a malformed row resurfaces at a bounded cadence
/// instead of being re-selected and error-logged every tick.
fn capture_params(intent: &WorkerIntent) -> anyhow::Result<CaptureParams<'_>> {
    let (Some(payment_intent_id), Some(account_id), Some(amount)) = (
        &intent.payment_intent_id,
        &intent.stripe_account_id,
        intent.capture_amount,
    ) else {
        anyhow::bail!(
            "capture_pending intent {:?} missing payment intent, account, \
             or amount",
            intent.id
        );
    };
    let minor_units = payloads::denomination(&intent.currency_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "non-denominated currency {} on capture",
                intent.currency_name
            )
        })?
        .minor_units;
    let amount_minor = payloads::to_minor_units(amount, minor_units)
        .ok_or_else(|| {
            anyhow::anyhow!("unquantized capture amount {amount}")
        })?;
    let fee = payloads::platform_fee(amount, minor_units);
    let fee_minor =
        if fee > Decimal::ZERO {
            Some(payloads::to_minor_units(fee, minor_units).ok_or_else(
                || anyhow::anyhow!("unquantized platform fee {fee}"),
            )?)
        } else {
            None
        };
    Ok(CaptureParams {
        payment_intent_id,
        account_id,
        amount,
        amount_minor,
        fee,
        fee_minor,
    })
}

/// A canceled intent row with no recorded PaymentIntent, as listed
/// lock-free by the reconciliation pass — possibly the local residue of
/// an execute claim that crashed after Stripe created the hold (and
/// whose adoption webhook was lost), but routinely just an order
/// canceled before any Stripe call (staled by a larger live hold, or
/// the worker's degenerate-row branch).
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct OrphanCandidate {
    pub(crate) id: payloads::FundingIntentId,
    pub(crate) auction_id: payloads::AuctionId,
    pub(crate) user_id: payloads::UserId,
    pub(crate) stripe_account_id: String,
    pub(crate) created_at: jiff_sqlx::Timestamp,
}

/// What one orphan-candidate sweep did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OrphanOutcome {
    /// Lock contention or the re-verify failed; re-found next pass.
    Skipped,
    /// The row's Stripe-side state was resolved and `reconciled_at`
    /// set; `hold_canceled` when a live hold was found and released.
    Resolved { hold_canceled: bool },
}

/// Resolve one orphan candidate: probe Stripe read-only for a
/// PaymentIntent carrying the row's id in its `funding_intent_id`
/// metadata (every create sets it), then cancel a found live hold,
/// record a found intent's id, or confirm absence — and mark the row
/// reconciled so it is examined once.
///
/// The probe replaces the plan doc's replay-by-idempotency-key design:
/// replaying the create for a row whose execute never reached Stripe
/// would mint a real fresh authorization on the member's card just to
/// cancel it.
///
/// Lock contract: acquires the pair try-lock (the probe decides a
/// Stripe mutation — the cancel); the reconcile mark is a single
/// status-guarded row update, so no account or funding locks are taken.
pub(crate) async fn sweep_orphaned_hold(
    candidate: &OrphanCandidate,
    deps: FlowDeps<'_>,
) -> anyhow::Result<OrphanOutcome> {
    let time_source = deps.time_source;
    let mut locks = TrackedTx::begin(&deps.worker_pool.0).await?;
    if !locks
        .acquire_pair(&candidate.auction_id, &candidate.user_id, LockWait::Try)
        .await?
    {
        return Ok(OrphanOutcome::Skipped);
    }

    // Re-verify under the claim: a late webhook may have recorded the
    // PaymentIntent (payment_failed's COALESCE) or another instance's
    // sweep may have resolved the row since selection.
    let current: Option<(payloads::FundingIntentStatus, Option<String>)> =
        sqlx::query_as(
            "SELECT status, payment_intent_id FROM funding_intents \
             WHERE id = $1 AND reconciled_at IS NULL",
        )
        .bind(candidate.id)
        .fetch_optional(&mut **locks.tx())
        .await?;
    if !matches!(
        current,
        Some((payloads::FundingIntentStatus::Canceled, None))
    ) {
        return Ok(OrphanOutcome::Skipped);
    }

    // The intent's create — if it ever ran — happened between the row's
    // insert and shortly after (executes run promptly; a generous upper
    // bound costs nothing).
    let created = candidate.created_at.to_jiff();
    let window_start = created
        .checked_sub(jiff::Span::new().hours(1))
        .map_err(anyhow::Error::from)?;
    let window_end = created
        .checked_add(jiff::Span::new().hours(48))
        .map_err(anyhow::Error::from)?;
    let found = match deps
        .stripe_service
        .find_payment_intent_by_metadata(
            &candidate.stripe_account_id,
            "funding_intent_id",
            &candidate.id.to_string(),
            window_start.as_second(),
            window_end.as_second(),
        )
        .await
    {
        Ok(found) => found,
        Err(e) => {
            return fail_claim(e, &candidate.id, time_source, locks).await;
        }
    };

    let Some(found) = found else {
        // No Stripe object was ever created with this row as seed — the
        // routine case (stale-order and degenerate cancels).
        store::funding::mark_intent_reconciled_tx(
            &candidate.id,
            None,
            time_source,
            locks.tx(),
        )
        .await?;
        locks.commit().await?;
        tracing::info!(
            intent_id = ?candidate.id,
            "orphan probe found no PaymentIntent; row reconciled"
        );
        return Ok(OrphanOutcome::Resolved {
            hold_canceled: false,
        });
    };

    let mut hold_canceled = false;
    match found.status {
        // A live hold the crashed execute left behind: cancel it (same
        // key as the worker cancel, so the paths converge at Stripe).
        LivePiStatus::RequiresCapture => {
            match funding::cancel_hold(
                &candidate.stripe_account_id,
                &found.payment_intent_id,
                &candidate.id,
                deps.stripe_service,
            )
            .await
            {
                Ok(funding::CancelHoldOutcome::Canceled) => {
                    hold_canceled = true;
                    tracing::info!(
                        intent_id = ?candidate.id,
                        payment_intent_id = %found.payment_intent_id,
                        "orphaned hold recovered and canceled"
                    );
                }
                // Canceled between the probe and this call (a racing
                // cancel or Stripe's own expiry): gone either way.
                Ok(funding::CancelHoldOutcome::AlreadyCanceled) => {
                    tracing::info!(
                        intent_id = ?candidate.id,
                        payment_intent_id = %found.payment_intent_id,
                        "orphaned hold was already canceled at Stripe"
                    );
                }
                Err(e) => {
                    return fail_claim(
                        e.into(),
                        &candidate.id,
                        time_source,
                        locks,
                    )
                    .await;
                }
            }
        }
        // Money moved outside our flow — reconciliation surfaces it
        // loudly (the entry-matching invariant keeps flagging the
        // mismatch); no silent convergence.
        LivePiStatus::Succeeded => {
            tracing::error!(
                intent_id = ?candidate.id,
                payment_intent_id = %found.payment_intent_id,
                "orphan probe found a SUCCEEDED PaymentIntent for a \
                 canceled intent row; manual reconciliation needed"
            );
        }
        // Already canceled at Stripe, or a decline's residue
        // (requires_payment_method holds nothing) — record the linkage
        // only.
        other => {
            tracing::info!(
                intent_id = ?candidate.id,
                payment_intent_id = %found.payment_intent_id,
                stripe_status = %other,
                "orphan probe found an inert PaymentIntent; recording"
            );
        }
    }
    store::funding::mark_intent_reconciled_tx(
        &candidate.id,
        Some(&found.payment_intent_id),
        time_source,
        locks.tx(),
    )
    .await?;
    locks.commit().await?;
    Ok(OrphanOutcome::Resolved { hold_canceled })
}

/// Everything a Stripe authorization needs beyond amounts, loaded at peek
/// time. Loading fails with the contextual error for the first missing
/// prerequisite (card, grant).
pub(crate) struct CardContext {
    pub connected_account_id: String,
    pub customer_id: String,
    pub payment_method_id: String,
    pub sizing: SizingContext,
}

/// Load the member's `CardContext` for a community, or the contextual
/// error for the first missing prerequisite
/// (`CardPaymentsNotEnabled` / `SavedCardRequired` /
/// `CardChargeGrantRequired`). Called ahead of the order transaction by
/// every card-path entry point (bid flow, pre-authorize, the proxy leg);
/// the bid flow
/// remaps `CardPaymentsNotEnabled` to `InsufficientBalance`, since a
/// community without a card path leaves the bid simply short on
/// balance.
pub(crate) async fn load_card_context(
    community_id: &CommunityId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<CardContext, StoreError> {
    use payloads::responses::CardAvailability;

    match funding::card_availability(community_id, user_id, pool).await? {
        CardAvailability::Available => {}
        CardAvailability::CommunityNotChargesEnabled => {
            return Err(ApiError::CardPaymentsNotEnabled.into());
        }
        CardAvailability::NoSavedCard => {
            return Err(ApiError::SavedCardRequired.into());
        }
        CardAvailability::NotGranted => {
            return Err(ApiError::CardChargeGrantRequired.into());
        }
    }

    let mut conn = pool.acquire().await?;
    let connected_account_id: String = sqlx::query_scalar(
        "SELECT stripe_account_id FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_one(&mut *conn)
    .await?;
    let (customer_id, payment_method_id): (String, String) = sqlx::query_as(
        "SELECT stripe_customer_id, payment_method_id \
         FROM user_payment_profiles WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&mut *conn)
    .await?;
    let sizing = load_sizing_context(community_id, user_id, &mut conn).await?;

    Ok(CardContext {
        connected_account_id,
        customer_id,
        payment_method_id,
        sizing,
    })
}

/// The member's funding gap picture for one auction, as computed by
/// `gap_view_tx`: how much backing the target requires beyond what
/// unclaimed balance and the live authorization can deliver. Consumed
/// by every card-path order plan.
pub(crate) struct GapView {
    /// The live authorization, if any (active, authorized, in-window).
    pub live_auth: Option<FundingIntent>,
    /// Backing achievable from balance alone.
    pub balance_backing: Decimal,
    /// Requirement beyond balance — what the card must cover in total.
    pub need_from_card: Decimal,
}

impl GapView {
    pub fn live_auth_amount(&self) -> Decimal {
        self.live_auth
            .as_ref()
            .and_then(|i| i.authorized_amount)
            .unwrap_or(Decimal::ZERO)
    }

    /// Requirement beyond balance AND the live authorization.
    pub fn gap(&self) -> Decimal {
        self.need_from_card - self.live_auth_amount()
    }

    /// Package a sized `amount` as the authorization order to commit
    /// (`ensure_pending_intent_tx`), deriving its relationship to the
    /// live hold — the replaced intent and the reduction flag — from
    /// this view, the one the amount was sized against.
    pub fn order(
        &self,
        origin: FundingIntentOrigin,
        amount: Decimal,
    ) -> AuthOrder {
        AuthOrder {
            origin,
            replaces_intent_id: self.live_auth.as_ref().map(|i| i.id),
            requested_amount: amount,
            reduction: amount < self.live_auth_amount(),
        }
    }
}

/// Compute a `GapView` for a target backing requirement `required`
/// (commitment + planned bids), reading balance backing and the live
/// authorization. Called from the order plans under the pair lock.
pub(crate) async fn gap_view_tx(
    community_id: &CommunityId,
    auction_id: &AuctionId,
    user_id: &UserId,
    required: Decimal,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<GapView, StoreError> {
    let balance_backing = balance_backing_tx(
        community_id,
        auction_id,
        user_id,
        time_source.now(),
        tx,
    )
    .await?;
    let live_auth = active_intent(auction_id, user_id, &mut **tx)
        .await?
        .filter(|i| i.is_live_auth(time_source.now()));
    Ok(GapView {
        live_auth,
        balance_backing,
        need_from_card: (required - balance_backing).max(Decimal::ZERO),
    })
}

/// The bid flow's plan reads on the current transaction: reject a closed round,
/// then compute the card gap the planned bid leaves after balance and the live
/// authorization. Returns the round plus Some(view) only when a positive bid
/// needs the card path (chore bids never do). Shared by the bid flow's order
/// transaction and the proxy claim's reactive order
/// (`order_proxy_bid_auth_tx`), so the two can't drift.
async fn bid_card_gap_tx(
    space: &store::Space,
    community_id: &CommunityId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(store::AuctionRound, Option<GapView>), StoreError> {
    let round = round_if_open(round_id, time_source, tx).await?;
    let bid_amount = planned_bid_amount_tx(space, &round, tx).await?;
    if bid_amount <= Decimal::ZERO {
        return Ok((round, None));
    }
    let commitment_here = store::currency::get_auction_commitment_tx(
        user_id,
        &round.auction_id,
        tx,
    )
    .await?;
    let view = gap_view_tx(
        community_id,
        &round.auction_id,
        user_id,
        commitment_here + bid_amount,
        time_source,
        tx,
    )
    .await?;
    if view.gap() <= Decimal::ZERO {
        return Ok((round, None));
    }
    Ok((round, Some(view)))
}

/// Size the authorization a bid-driven gap (`view`) requires: initial
/// auths follow the member's strategy, raises catch up or double. The
/// result is ceiled to minor units and floored at the denomination
/// minimum. Called by the bid flow's order transaction and the proxy
/// bidding claim's reactive order (`order_proxy_bid_auth_tx`); the
/// pre-authorize flow sizes via `funding::strategy_preauth_amount`
/// instead.
pub(crate) async fn size_authorization(
    sizing: &SizingContext,
    auction_id: &AuctionId,
    user_id: &UserId,
    view: &GapView,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Decimal, StoreError> {
    let need = view.need_from_card;
    let prior = view.live_auth_amount();
    let amount = if prior > Decimal::ZERO {
        raise_target(prior, need)
    } else {
        let budget = auction_budget(auction_id, user_id, &mut **tx).await?;
        initial_auth_size(
            sizing.budget_holds,
            need,
            budget - view.balance_backing,
            sizing.stripe_min_charge,
        )
    };
    // Clamped to the card-charge maximum: an over-limit order would
    // fail permanently at Stripe on every replay; a capped hold is
    // honest partial backing (the bid then fails on funds rather than
    // poisoning the card path).
    Ok(ceil_to_minor_units(
        amount
            .max(sizing.stripe_min_charge)
            .min(sizing.stripe_max_charge),
        sizing.minor_units,
    ))
}

/// Read the round and reject if it isn't currently open for bidding —
/// the plan phase's economy gate, sparing a doomed bid its Stripe raise.
/// Correctness still lives in `create_bid_tx`'s own checks at finalize.
async fn round_if_open(
    round_id: &AuctionRoundId,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<store::AuctionRound, StoreError> {
    let round = sqlx::query_as::<_, store::AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::AuctionRoundNotFound)?;
    let now = time_source.now();
    if now < round.start_at {
        return Err(ApiError::RoundNotStarted.into());
    }
    if now >= round.end_at {
        return Err(ApiError::RoundEnded.into());
    }
    Ok(round)
}
