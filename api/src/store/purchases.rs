//! Credit purchases (backed_credits mode): a member buys credits, or
//! settles debt from a failed settlement capture, through a Stripe
//! Checkout session charged directly on the community's connected
//! account.
//!
//! Flow: pre-insert a `credit_purchases` row (its id seeds the Checkout
//! session's idempotency key and rides in the session and
//! PaymentIntent metadata) → mint the session at Stripe → store the
//! session id and hand the member its URL. Completion arrives by
//! webhook poke: the payload only routes (session/intent id → purchase
//! row); the handler re-derives from live session and intent state via
//! [`converge_purchase`], so delivery order can't matter — every poke
//! lands on the truth. The one payload-applied arm is
//! `payment_intent.processing` (moves `created` → `processing` only,
//! monotone-forward and thus order-safe now that every terminal
//! transition is live-derived). No coordination locks: rows are
//! single-writer per event (webhook applier), and the issuance's entry
//! idempotency is the once-only guarantee.
//!
//! Webhook matching tries the stored Stripe id first — self-authorizing, since
//! the id was written by us and `data.object.id` is Stripe-serialized, which
//! lets late events from a since-replaced connected account still converge rows
//! (the poke retrieves on the event's envelope account, where the object lives)
//! — falling back to the `credit_purchase_id` metadata scoped to the event's
//! envelope account for the pre-link window. Rows whose webhooks were
//! permanently missed (a paid session with no delivered events, a stuck
//! `processing` row) are converged from live state by `probe_stuck_purchases`
//! before a new settlement is created, and by the hourly reconciliation
//! sweep ([`sweep_stuck_purchases`]) — the only path that reaches
//! a stuck top-up whose member never settles debt.
//!
//! Debt settlement never takes the balance positive by construction: the amount
//! is validated against the member's exact effective debt (balance plus pending
//! captures) at session creation, and at most one settlement session is
//! completable at a time. A settlement row leaves 'created' only once its
//! Stripe session is confirmed dead (expired by the pre-creation sweep, the
//! expiry webhook, or its own creator's post-mint abort) or paid, so a live row
//! always marks a possibly completable session. Creation expires superseded
//! sessions at Stripe up front (atomic against completion: a paid session can't
//! be expired, and the creation bails instead of double-charging) and re-checks
//! after minting for other live rows and debt drift, aborting its own session
//! if either appears. Concurrent creations that see each other both abort; the
//! member's retry succeeds serially. The residual anomaly, debt shrinking
//! between session creation and payment, is accepted and warn-logged at
//! issuance (the money was collected; the credits are theirs).

use anyhow::Context;
use payloads::{
    ApiError, CommunityId, CreditPurchaseId, CurrencyMode, PurchaseKind,
    PurchaseStatus, UserId,
};
use rust_decimal::Decimal;
use sqlx::PgPool;

use super::StoreError;
use crate::AppConfig;
use crate::stripe_service::{
    CheckoutSessionParams, ExpireSessionOutcome, LivePiStatus,
    LiveSessionStatus, SessionMode, StripeService,
};
use crate::time::TimeSource;
use jiff_sqlx::ToSqlx;

/// How long a minted row (a purchase here, a `checkout_created`
/// funding row in `funding_checkout`) may sit without a stored session
/// id before its creation is treated as failed or crashed mid-mint and
/// the row is retired. Sits comfortably above any request or Stripe
/// client timeout, so a session-less row this old cannot still be
/// mid-mint; even a pathologically late mint is caught by its
/// creator's post-store status check.
pub(crate) const MINT_GRACE_MINUTES: i64 = 15;

/// Whether top-up purchases are open: the payloads deployment gate,
/// with the mock-stripe feature and debug builds as test/dev escapes so
/// integration tests and the dev server can exercise the flow before
/// the gate flips. Debug builds match the UI's gate (the api Dockerfile
/// builds --release, so production never loosens). Debt settlement is
/// not gated (see module docs).
fn top_ups_enabled() -> bool {
    payloads::CREDIT_PURCHASES_ENABLED
        || cfg!(feature = "mock-stripe")
        || cfg!(debug_assertions)
}

/// Start a credit purchase: validate, pre-insert the row, mint the
/// Checkout session, and return its URL for redirect.
pub async fn create_credit_purchase(
    actor: &super::ValidatedMember,
    kind: PurchaseKind,
    amount: Decimal,
    stripe_service: &StripeService,
    app_config: &AppConfig,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<String, StoreError> {
    let community_id = actor.0.community_id;
    let user_id = actor.0.user_id;

    #[derive(sqlx::FromRow)]
    struct PurchaseContext {
        name: String,
        currency_mode: CurrencyMode,
        currency_name: String,
        stripe_account_id: Option<String>,
        stripe_charges_enabled: bool,
        stripe_deauthorized_at: Option<jiff_sqlx::Timestamp>,
    }
    let ctx: PurchaseContext = sqlx::query_as(
        "SELECT name, currency_mode, currency_name, stripe_account_id, \
                stripe_charges_enabled, stripe_deauthorized_at \
         FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_one(pool)
    .await
    .context("Failed to load purchase context")?;

    let operational = super::connect::connected_account_operational(
        ctx.stripe_account_id.as_deref(),
        ctx.stripe_charges_enabled,
        ctx.stripe_deauthorized_at.map(|ts| ts.to_jiff()),
    );
    let account_id = match ctx.stripe_account_id {
        Some(account_id)
            if ctx.currency_mode == CurrencyMode::BackedCredits
                && operational =>
        {
            account_id
        }
        _ => return Err(ApiError::CardPaymentsNotEnabled.into()),
    };
    let denomination = payloads::denomination(&ctx.currency_name)
        .ok_or(StoreError::InvalidCurrencyConfiguration)?;

    let amount_minor =
        payloads::to_minor_units(amount, denomination.minor_units)
            .filter(|minor| *minor > 0)
            .ok_or(ApiError::InvalidPurchaseAmount)?;
    if amount > denomination.stripe_max_charge {
        return Err(ApiError::AmountTooLarge {
            max: denomination.stripe_max_charge,
        }
        .into());
    }
    if kind == PurchaseKind::TopUp {
        if !top_ups_enabled() {
            return Err(ApiError::CreditPurchasesDisabled.into());
        }
        if amount < denomination.stripe_min_charge {
            return Err(ApiError::InvalidPurchaseAmount.into());
        }
    }

    // Settlement pre-flight, ahead of the creation transaction: converge the
    // member's stuck purchase rows from live Stripe state (a permanently
    // missed webhook would otherwise block settlement forever), then kill
    // superseded sessions at Stripe so no earlier session can be completed
    // into a second payment once this one exists (a session already paid
    // bails the whole creation).
    if kind == PurchaseKind::DebtSettlement {
        probe_stuck_purchases(
            &community_id,
            &user_id,
            &account_id,
            stripe_service,
            time_source,
            pool,
        )
        .await?;
        expire_stale_settlement_sessions(
            &community_id,
            &user_id,
            &account_id,
            stripe_service,
            time_source,
            pool,
        )
        .await?;
    }

    let now = time_source.now();
    let mut tx = pool.begin().await?;
    if kind == PurchaseKind::DebtSettlement {
        // The checks below are fast-fail validation; the post-mint
        // survivor election is the authoritative guard against
        // concurrent creations.
        //
        // Re-derive the effective debt and require the requested
        // amount to clear it exactly, so the balance never crosses
        // zero. Below the minimum charge the action is unavailable
        // (the leader clears the remainder with treasury tools).
        let debt =
            effective_debt_conn(&community_id, &user_id, &mut tx).await?;
        if amount != debt {
            return Err(ApiError::DebtAmountMismatch.into());
        }
        if debt < denomination.stripe_min_charge {
            return Err(ApiError::InvalidPurchaseAmount.into());
        }
        // One settlement in flight at a time: a delayed method is
        // already collecting, so a second charge would overshoot.
        let processing: Option<(CreditPurchaseId,)> = sqlx::query_as(
            "SELECT id FROM credit_purchases \
             WHERE community_id = $1 AND user_id = $2 \
               AND kind = 'debt_settlement' AND status = 'processing'",
        )
        .bind(community_id)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;
        if processing.is_some() {
            return Err(ApiError::SettlementAlreadyProcessing.into());
        }
    }

    let (purchase_id,): (CreditPurchaseId,) = sqlx::query_as(
        "INSERT INTO credit_purchases \
         (community_id, user_id, kind, amount, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $5) RETURNING id",
    )
    .bind(community_id)
    .bind(user_id)
    .bind(kind)
    .bind(amount)
    .bind(now.to_sqlx())
    .fetch_one(&mut *tx)
    .await
    .context("Failed to insert credit purchase")?;
    // Commit before the Stripe call: no data row waits on the network.
    // A crash here leaves a 'created' row with no session — inert, and
    // superseded by the member's retry.
    tx.commit().await?;

    let return_url = format!(
        "{}/communities/{}/currency",
        app_config.base_url, community_id
    );
    let description = match kind {
        PurchaseKind::TopUp => {
            format!("Credits — {}", ctx.name)
        }
        PurchaseKind::DebtSettlement => {
            format!("Outstanding balance payment — {}", ctx.name)
        }
    };
    let fee = payloads::platform_fee(amount, denomination.minor_units);
    let fee_minor = if fee > Decimal::ZERO {
        Some(
            payloads::to_minor_units(fee, denomination.minor_units)
                .ok_or_else(|| {
                    anyhow::anyhow!("unquantized platform fee {fee}")
                })?,
        )
    } else {
        None
    };

    let session = match stripe_service
        .create_payment_checkout_session(CheckoutSessionParams {
            account_id: &account_id,
            mode: SessionMode::Purchase {
                purchase_id: &purchase_id,
                application_fee_minor: fee_minor,
            },
            amount_minor,
            currency: &denomination.iso.to_lowercase(),
            description: &description,
            success_url: &format!("{return_url}?purchase=success"),
            cancel_url: &format!("{return_url}?purchase=canceled"),
        })
        .await
    {
        Ok(session) => session,
        Err(e) => {
            // No session exists for this row, so no webhook will ever retire
            // it. Fail it now (best-effort) or it lingers as pending, and a
            // settlement row would block every later settlement until the
            // grace-period sweep.
            if let Err(mark_err) = set_purchase_status(
                &purchase_id,
                &[PurchaseStatus::Created],
                PurchaseStatus::Failed,
                None,
                time_source,
                pool,
            )
            .await
            {
                tracing::warn!(
                    %purchase_id,
                    "failed to retire purchase after mint failure: \
                     {mark_err:#}"
                );
            }
            return Err(StoreError::stripe(e));
        }
    };

    let (stored_status,): (PurchaseStatus,) = sqlx::query_as(
        "UPDATE credit_purchases \
         SET checkout_session_id = $1, updated_at = $2 WHERE id = $3 \
         RETURNING status",
    )
    .bind(&session.session_id)
    .bind(time_source.now().to_sqlx())
    .bind(purchase_id)
    .fetch_one(pool)
    .await
    .context("Failed to store checkout session id")?;

    // The sweep grace-retires session-less settlement rows older than the mint
    // window; a pathologically slow mint can land after its row was retired.
    // Kill the session rather than hand out a URL for a dead row.
    if kind == PurchaseKind::DebtSettlement
        && stored_status != PurchaseStatus::Created
    {
        abort_minted_settlement_session(
            &purchase_id,
            &account_id,
            &session.session_id,
            "row retired during mint",
            stripe_service,
            time_source,
            pool,
        )
        .await?;
        return Err(ApiError::SettlementAlreadyProcessing.into());
    }

    // Settlement survivor election. Two settlement creations can race past the
    // up-front sweep (which can't expire a session id that isn't stored yet)
    // and each commit a live row. Rows leave 'created' only once their session
    // is confirmed dead at Stripe (the sweep, the expiry webhook, an election
    // abort) or paid, so finding no other live row here proves every other
    // settlement session is dead or paid — and a payment either shows as a live
    // 'processing' row or has moved the debt, failing the re-check below. Of
    // two racers, the later committer always still sees the earlier row unless
    // its session is already dead, so at most one completable session survives
    // any interleaving.
    //
    // When both racers see each other, both abort and the member's retry
    // succeeds serially; an aborted creation never returns the Checkout URL
    // (the completion capability), so a dead attempt can't be paid. A shrunk
    // debt is caught the same way: the session is dead before the member can
    // pay a stale amount.
    if kind == PurchaseKind::DebtSettlement {
        let other_live: Option<(CreditPurchaseId,)> = sqlx::query_as(
            "SELECT id FROM credit_purchases \
             WHERE community_id = $1 AND user_id = $2 \
               AND kind = 'debt_settlement' AND id != $3 \
               AND status = ANY($4)",
        )
        .bind(community_id)
        .bind(user_id)
        .bind(purchase_id)
        .bind(PurchaseStatus::non_terminal())
        .fetch_optional(pool)
        .await?;
        let debt = effective_debt_conn(
            &community_id,
            &user_id,
            &mut *pool.acquire().await?,
        )
        .await?;
        if other_live.is_some() || debt != amount {
            let reason = if other_live.is_some() {
                "superseded by another live settlement"
            } else {
                "debt changed during mint"
            };
            abort_minted_settlement_session(
                &purchase_id,
                &account_id,
                &session.session_id,
                reason,
                stripe_service,
                time_source,
                pool,
            )
            .await?;
            return Err(ApiError::SettlementAlreadyProcessing.into());
        }
    }

    tracing::info!(
        %purchase_id, %community_id, %user_id, ?kind, %amount,
        "credit purchase checkout session created"
    );
    Ok(session.url)
}

/// Retire a settlement session its own creation rejected after minting: expire
/// it at Stripe (so it can never be completed) and mark the local row expired
/// once the expiry is confirmed. `reason` says why the session was rejected,
/// for the log only.
async fn abort_minted_settlement_session(
    purchase_id: &CreditPurchaseId,
    account_id: &str,
    session_id: &str,
    reason: &'static str,
    stripe_service: &StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let expired = match stripe_service
        .expire_checkout_session(account_id, session_id)
        .await
    {
        Ok(ExpireSessionOutcome::Expired) => {
            tracing::info!(
                %purchase_id,
                session_id,
                reason,
                "aborted settlement session after post-mint checks"
            );
            true
        }
        Ok(ExpireSessionOutcome::Completed) => {
            // The member paid in the window between mint and this
            // check. The session can't be killed and its payment is in
            // flight, so leave the row for the webhook to issue rather
            // than expiring it out from under an incoming success.
            tracing::warn!(
                %purchase_id,
                session_id,
                reason,
                "settlement session rejected post-mint was already \
                 paid; deferring to its webhook"
            );
            false
        }
        Err(e) => {
            tracing::warn!(
                %purchase_id,
                session_id,
                reason,
                "failed to expire settlement session rejected \
                 post-mint: {e:#}"
            );
            false
        }
    };
    if expired {
        set_purchase_status(
            purchase_id,
            &[PurchaseStatus::Created],
            PurchaseStatus::Expired,
            None,
            time_source,
            pool,
        )
        .await?;
    }
    Ok(())
}

/// Converge the member's non-terminal purchase rows from live Stripe
/// state before a new settlement is created.
///
/// Webhook delivery can be permanently missed (Stripe stops delivering
/// for a replaced account; a current-account delivery can be lost past
/// retries) and nothing else ages a stuck row out — a settlement row
/// would block every later settlement forever, with a paid session's
/// money collected and no credits issued.
///
/// One `converge_purchase` call per row covers every stuck shape at
/// once: a still-`created` row whose session was paid or died, and a
/// `processing` row whose intent settled or failed. Both kinds converge
/// (a stuck top-up heals opportunistically here too). A transient
/// retrieve failure leaves the row blocking rather than risk retiring a
/// live collection.
async fn probe_stuck_purchases(
    community_id: &CommunityId,
    user_id: &UserId,
    account_id: &str,
    stripe_service: &StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let rows: Vec<(CreditPurchaseId,)> = sqlx::query_as(
        "SELECT id FROM credit_purchases \
         WHERE community_id = $1 AND user_id = $2 \
           AND status = ANY($3)",
    )
    .bind(community_id)
    .bind(user_id)
    .bind(PurchaseStatus::non_terminal())
    .fetch_all(pool)
    .await?;
    for (purchase_id,) in rows {
        if let Err(e) = converge_purchase(
            purchase_id,
            account_id,
            stripe_service,
            time_source,
            pool,
        )
        .await
        {
            tracing::warn!(
                %purchase_id,
                "stuck-purchase probe failed: {e:#}"
            );
        }
    }
    Ok(())
}

/// Probe non-terminal purchase rows (`created`/`processing`) stale past
/// the worker-lag grace against live Stripe state, one
/// [`converge_purchase`] call per row — the safety net for permanently
/// missed webhook deliveries, run by the hourly reconciliation pass.
///
/// The settlement flow runs the same probe as pre-flight for the
/// settling member, but a stuck top-up has no such trigger: a member
/// who topped up and never settles debt would otherwise keep a phantom
/// in-progress purchase forever — or, for a paid session whose events
/// were all lost, money collected with no credits issued.
///
/// Probing on the community's current account is deliberate:
/// convergence retires rows whose objects died with a since-replaced
/// account. Only communities with a reachable account are selected;
/// convergence is idempotent and status-guarded, so racing a webhook
/// poke or a settlement pre-flight is safe.
pub(crate) async fn sweep_stuck_purchases(
    pool: &PgPool,
    time_source: &TimeSource,
    stripe_service: &StripeService,
    report: &mut super::reconciliation::ReconciliationReport,
) -> anyhow::Result<()> {
    use super::reconciliation::InvariantViolation;

    let grace = super::reconciliation::worker_lag_cutoff(time_source)?;
    let candidates: Vec<(CreditPurchaseId, CommunityId, String)> =
        sqlx::query_as(&format!(
            "SELECT cp.id, cp.community_id, c.stripe_account_id \
             FROM credit_purchases cp \
             JOIN communities c ON cp.community_id = c.id \
             WHERE cp.status = ANY($1) \
               AND cp.updated_at < $2 \
               AND {account_reachable}",
            account_reachable = super::connect::account_reachable_sql("c"),
        ))
        .bind(PurchaseStatus::non_terminal())
        .bind(grace.to_sqlx())
        .fetch_all(pool)
        .await?;

    report.purchase_candidates = candidates.len();
    for (purchase_id, community_id, account_id) in candidates {
        if let Err(e) = converge_purchase(
            purchase_id,
            &account_id,
            stripe_service,
            time_source,
            pool,
        )
        .await
        {
            report.violations.push((
                community_id,
                InvariantViolation::PurchaseProbeFailed {
                    purchase_id,
                    detail: format!("{e:#}"),
                },
            ));
        }
    }
    Ok(())
}

/// The session-stage decision for one (local × live session) pair —
/// what [`converge_purchase`] does before any PaymentIntent exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionAction {
    /// The member may still pay; leave the row.
    Waiting,
    /// The session can never be completed; retire the row as expired.
    RetireExpired,
    /// Payment was submitted: link the session's PaymentIntent and
    /// decide from it ([`decide_intent`]).
    IntentStage,
    /// A pair that should be impossible; report, change nothing.
    Anomaly,
}

/// The session-stage transition table: local purchase statuses down,
/// live Checkout-session state across. One exhaustive match over the
/// enum product, mirroring `convergence::decide` — extend it cell by
/// cell when a status is added; never with a catch-all across local
/// statuses. Only `created` rows consult the session (past `created`
/// the PaymentIntent is linked and authoritative), so every other
/// local status defers to the intent stage; those cells exist so a new
/// status forces a decision here too.
fn decide_session(
    local: PurchaseStatus,
    session: LiveSessionStatus,
) -> SessionAction {
    use LiveSessionStatus as S;
    use PurchaseStatus as L;
    use SessionAction as A;

    match (local, session) {
        // Open: the member may still pay; in-checkout declines stay
        // retryable inside the session.
        (L::Created, S::Open) => A::Waiting,
        // Expired is Stripe-side expiry; Missing a since-replaced
        // account — either way the session can never be completed.
        (L::Created, S::Expired | S::Missing) => A::RetireExpired,
        (L::Created, S::Complete) => A::IntentStage,
        (L::Created, S::Unknown) => A::Anomaly,

        (
            L::Processing | L::Succeeded | L::Failed | L::Expired,
            S::Open | S::Complete | S::Expired | S::Missing | S::Unknown,
        ) => A::IntentStage,
    }
}

/// The intent-stage decision for one (local × live PaymentIntent)
/// pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntentAction {
    /// Expected steady state; nothing to do.
    InSync,
    /// Transitional; the next poke or probe decides.
    Waiting,
    /// A pair that should be impossible; report, change nothing.
    Anomaly,
    /// Money was collected: issue via the idempotent
    /// [`issue_purchase`], which also books a late success on a
    /// locally failed/expired row.
    Issue,
    /// A delayed method is settling: `created` → `processing`.
    MarkProcessing,
    /// The submitted payment failed or was canceled; retire a
    /// non-terminal row as failed.
    MarkFailed,
    /// The intent is gone from the account (since-replaced): retire a
    /// non-terminal row as expired — unblocking settlement without
    /// disowning the payment.
    MarkExpired,
}

/// The intent-stage transition table: local purchase statuses down,
/// live PaymentIntent state across; same conventions as
/// [`decide_session`].
fn decide_intent(local: PurchaseStatus, live: LivePiStatus) -> IntentAction {
    use IntentAction as A;
    use LivePiStatus as P;
    use PurchaseStatus as L;

    match (local, live) {
        // Created: reached only with a completed session (open sessions
        // stop at the session stage), so requires_payment_method means
        // the submitted payment failed after completion — not an
        // in-checkout decline. requires_confirmation/requires_action is
        // post-submission 3DS still resolving on Stripe's page.
        // Purchases charge automatically; manual capture never appears.
        (L::Created, P::Succeeded) => A::Issue,
        (L::Created, P::Processing) => A::MarkProcessing,
        (L::Created, P::Canceled | P::RequiresPaymentMethod) => A::MarkFailed,
        (L::Created, P::Missing) => A::MarkExpired,
        (L::Created, P::RequiresConfirmation | P::RequiresAction) => A::Waiting,
        (L::Created, P::RequiresCapture | P::Unknown) => A::Anomaly,

        // Processing: a delayed method is settling; it resolves forward
        // to succeeded or requires_payment_method, never backwards into
        // the member-present states.
        (L::Processing, P::Succeeded) => A::Issue,
        (L::Processing, P::Processing) => A::InSync,
        (L::Processing, P::Canceled | P::RequiresPaymentMethod) => {
            A::MarkFailed
        }
        (L::Processing, P::Missing) => A::MarkExpired,
        (
            L::Processing,
            P::RequiresConfirmation
            | P::RequiresAction
            | P::RequiresCapture
            | P::Unknown,
        ) => A::Anomaly,

        // Succeeded: credits issued; missing tolerates a since-replaced
        // account. (`converge_purchase` short-circuits succeeded rows
        // before any retrieve; these cells are the spec.)
        (L::Succeeded, P::Succeeded | P::Missing) => A::InSync,
        (
            L::Succeeded,
            P::RequiresPaymentMethod
            | P::RequiresConfirmation
            | P::RequiresAction
            | P::Processing
            | P::RequiresCapture
            | P::Canceled
            | P::Unknown,
        ) => A::Anomaly,

        // Failed: retired after a post-completion failure; the intent
        // parks at requires_payment_method forever. A late success is
        // the money arriving anyway — book it.
        (L::Failed, P::Succeeded) => A::Issue,
        (L::Failed, P::Processing) => A::Waiting,
        (L::Failed, P::Canceled | P::RequiresPaymentMethod | P::Missing) => {
            A::InSync
        }
        (
            L::Failed,
            P::RequiresConfirmation
            | P::RequiresAction
            | P::RequiresCapture
            | P::Unknown,
        ) => A::Anomaly,

        // Expired: the session died (abandoned, superseded, or its
        // account replaced); its intent's member-present states are
        // inert residue of the dead session. A late success books like
        // Failed's.
        (L::Expired, P::Succeeded) => A::Issue,
        (L::Expired, P::Processing) => A::Waiting,
        (
            L::Expired,
            P::Canceled
            | P::Missing
            | P::RequiresPaymentMethod
            | P::RequiresConfirmation
            | P::RequiresAction,
        ) => A::InSync,
        (L::Expired, P::RequiresCapture | P::Unknown) => A::Anomaly,
    }
}

/// Converge one purchase row against live Stripe state — the shared
/// decision used by the webhook pokes (`payment_intent.succeeded|
/// payment_failed|canceled`, `checkout.session.*`),
/// `probe_stuck_purchases`, and the hourly reconciliation sweep. Every
/// caller re-derives from live state, so delivery order can't matter.
///
/// `account_id` scopes the retrieves: the event's signature-attested
/// envelope account for pokes (the account the object lives on, letting
/// late old-account deliveries still converge), the community's current
/// account for the probe and the sweep.
///
/// The decision is layered, not a single product: a `created` row consults
/// its Checkout session first ([`decide_session`]); once payment is
/// submitted the PaymentIntent is authoritative ([`decide_intent`]), which
/// also covers healing a missed `checkout.session.completed` by linking the
/// session's intent before deciding from it.
pub(crate) async fn converge_purchase(
    purchase_id: CreditPurchaseId,
    account_id: &str,
    stripe_service: &StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        status: PurchaseStatus,
        checkout_session_id: Option<String>,
        payment_intent_id: Option<String>,
    }
    let row: Option<Row> = sqlx::query_as(
        "SELECT status, checkout_session_id, payment_intent_id \
         FROM credit_purchases WHERE id = $1",
    )
    .bind(purchase_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(());
    };
    if row.status == PurchaseStatus::Succeeded {
        return Ok(());
    }

    let mut payment_intent_id = row.payment_intent_id.clone();
    if row.status == PurchaseStatus::Created {
        let Some(session_id) = &row.checkout_session_id else {
            // Mint in flight or crashed pre-store; the grace sweep owns
            // these.
            return Ok(());
        };
        let session = stripe_service
            .retrieve_checkout_session(account_id, session_id)
            .await
            .map_err(StoreError::stripe)?;
        let session_status = session
            .as_ref()
            .map(|s| s.status)
            .unwrap_or(LiveSessionStatus::Missing);
        match decide_session(row.status, session_status) {
            SessionAction::Waiting => return Ok(()),
            SessionAction::Anomaly => {
                tracing::warn!(
                    %purchase_id,
                    session_id,
                    status = session_status.as_str(),
                    "checkout session in unrecognized state; leaving"
                );
                return Ok(());
            }
            SessionAction::RetireExpired => {
                if set_purchase_status(
                    &purchase_id,
                    &[PurchaseStatus::Created],
                    PurchaseStatus::Expired,
                    None,
                    time_source,
                    pool,
                )
                .await?
                {
                    tracing::info!(
                        %purchase_id,
                        session_id,
                        "checkout session dead at Stripe; purchase retired"
                    );
                }
                return Ok(());
            }
            SessionAction::IntentStage => {
                payment_intent_id = payment_intent_id
                    .or_else(|| session.and_then(|s| s.payment_intent_id));
                let Some(intent_id) = &payment_intent_id else {
                    tracing::warn!(
                        %purchase_id,
                        session_id,
                        "completed session reports no PaymentIntent"
                    );
                    return Ok(());
                };
                sqlx::query(
                    "UPDATE credit_purchases \
                     SET payment_intent_id = COALESCE(payment_intent_id, $1), \
                         updated_at = $2 \
                     WHERE id = $3",
                )
                .bind(intent_id)
                .bind(time_source.now().to_sqlx())
                .bind(purchase_id)
                .execute(pool)
                .await?;
            }
        }
    }

    let Some(payment_intent_id) = payment_intent_id else {
        // Both 'processing' writers store the intent id in the same
        // update, so a missing id is an anomaly; failed/expired rows
        // without one simply have nothing to converge from.
        if row.status == PurchaseStatus::Processing {
            tracing::warn!(
                %purchase_id,
                "processing purchase row has no payment intent id"
            );
        }
        return Ok(());
    };
    let retrieved = stripe_service
        .retrieve_payment_intent(account_id, &payment_intent_id)
        .await
        .map_err(StoreError::stripe)?;
    let live_status = retrieved
        .as_ref()
        .map(|i| i.status)
        .unwrap_or(LivePiStatus::Missing);
    match decide_intent(row.status, live_status) {
        IntentAction::InSync => {}
        IntentAction::Waiting => {
            tracing::debug!(
                %purchase_id,
                payment_intent_id,
                status = live_status.as_str(),
                "purchase intent in transitional state; leaving"
            );
        }
        IntentAction::Anomaly => {
            tracing::warn!(
                %purchase_id,
                payment_intent_id,
                local_status = %row.status,
                live_status = live_status.as_str(),
                "purchase (local × live) pair has no legal transition; \
                 leaving"
            );
        }
        IntentAction::Issue => {
            let Some(intent) = retrieved else {
                // decide_intent never returns Issue for Missing.
                return Ok(());
            };
            issue_purchase(
                purchase_id,
                &payment_intent_id,
                intent.amount_received_minor,
                time_source,
                pool,
            )
            .await?;
        }
        IntentAction::MarkProcessing => {
            if set_purchase_status(
                &purchase_id,
                &[PurchaseStatus::Created],
                PurchaseStatus::Processing,
                None,
                time_source,
                pool,
            )
            .await?
            {
                tracing::info!(
                    %purchase_id,
                    payment_intent_id,
                    "purchase payment processing (delayed method settling)"
                );
            }
        }
        IntentAction::MarkFailed => {
            if set_purchase_status(
                &purchase_id,
                &PurchaseStatus::non_terminal(),
                PurchaseStatus::Failed,
                None,
                time_source,
                pool,
            )
            .await?
            {
                tracing::info!(
                    %purchase_id,
                    payment_intent_id,
                    status = live_status.as_str(),
                    "purchase intent failed at Stripe; purchase retired"
                );
            }
        }
        IntentAction::MarkExpired => {
            if set_purchase_status(
                &purchase_id,
                &PurchaseStatus::non_terminal(),
                PurchaseStatus::Expired,
                None,
                time_source,
                pool,
            )
            .await?
            {
                tracing::warn!(
                    %purchase_id,
                    payment_intent_id,
                    "purchase intent is gone from the account \
                     (since-replaced account); retired"
                );
            }
        }
    }
    Ok(())
}

/// Apply a guarded status transition to a purchase row: move it to `to`
/// only while its current status is in `from`, optionally linking the
/// PaymentIntent id (COALESCE — never overwrites an existing link).
/// Returns whether a row moved (false = another actor already advanced
/// it). `from` is typed so the guard lists derive from
/// [`PurchaseStatus`] instead of hand-kept SQL literals.
async fn set_purchase_status(
    purchase_id: &CreditPurchaseId,
    from: &[PurchaseStatus],
    to: PurchaseStatus,
    link_intent: Option<&str>,
    time_source: &TimeSource,
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<bool, StoreError> {
    let updated = sqlx::query(
        "UPDATE credit_purchases \
         SET status = $1, \
             payment_intent_id = COALESCE(payment_intent_id, $2), \
             updated_at = $3 \
         WHERE id = $4 AND status = ANY($5)",
    )
    .bind(to)
    .bind(link_intent)
    .bind(time_source.now().to_sqlx())
    .bind(purchase_id)
    .bind(from)
    .execute(executor)
    .await?;
    Ok(updated.rows_affected() > 0)
}

/// Retire the member's abandoned settlement attempts before a new one
/// is created.
///
/// Still-'created' rows with a stored session id get their session
/// expired at Stripe first — Stripe only expires open sessions, atomic
/// against completion: a session the member already paid reports
/// `Completed`, and this bails with `SettlementAlreadyProcessing`
/// instead of letting a new session be minted (that payment settles the
/// debt when its webhook arrives, through the normal 'created' →
/// succeeded path, since the row is left untouched). A row is marked
/// expired locally only once its session is confirmed dead at Stripe,
/// so a live 'created' row always marks a possibly completable session.
///
/// Rows with no stored session id belong to an in-flight mint and are left
/// alone, unless older than the mint grace window — then their creation failed
/// or crashed before returning a URL (the completion capability), so they're
/// retired rather than left to block settlements forever.
async fn expire_stale_settlement_sessions(
    community_id: &CommunityId,
    user_id: &UserId,
    account_id: &str,
    stripe_service: &StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let grace_cutoff =
        time_source.now() - jiff::Span::new().minutes(MINT_GRACE_MINUTES);
    let orphaned = sqlx::query(
        "UPDATE credit_purchases \
         SET status = 'expired', updated_at = $1 \
         WHERE community_id = $2 AND user_id = $3 \
           AND kind = 'debt_settlement' AND status = 'created' \
           AND checkout_session_id IS NULL AND created_at < $4",
    )
    .bind(time_source.now().to_sqlx())
    .bind(community_id)
    .bind(user_id)
    .bind(grace_cutoff.to_sqlx())
    .execute(pool)
    .await?;
    if orphaned.rows_affected() > 0 {
        tracing::warn!(
            %community_id,
            %user_id,
            count = orphaned.rows_affected(),
            "retired settlement rows that never stored a session id \
             (creation failed or crashed mid-mint)"
        );
    }

    let stale: Vec<(CreditPurchaseId, String)> = sqlx::query_as(
        "SELECT id, checkout_session_id FROM credit_purchases \
         WHERE community_id = $1 AND user_id = $2 \
           AND kind = 'debt_settlement' AND status = 'created' \
           AND checkout_session_id IS NOT NULL",
    )
    .bind(community_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    for (purchase_id, session_id) in stale {
        let outcome = stripe_service
            .expire_checkout_session(account_id, &session_id)
            .await
            .map_err(StoreError::stripe)?;
        match outcome {
            ExpireSessionOutcome::Expired => {
                set_purchase_status(
                    &purchase_id,
                    &[PurchaseStatus::Created],
                    PurchaseStatus::Expired,
                    None,
                    time_source,
                    pool,
                )
                .await?;
                tracing::info!(
                    %purchase_id,
                    session_id,
                    "expired superseded settlement session"
                );
            }
            ExpireSessionOutcome::Completed => {
                tracing::info!(
                    %purchase_id,
                    session_id,
                    "abandoned settlement session was already paid; \
                     deferring to its webhook"
                );
                return Err(ApiError::SettlementAlreadyProcessing.into());
            }
        }
    }
    Ok(())
}

/// The member's credit purchases in a community: non-terminal rows
/// (`created`/`processing`) first, then the rest newest-first.
pub async fn list_credit_purchases(
    actor: &super::ValidatedMember,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::CreditPurchase>, StoreError> {
    let rows: Vec<(
        CreditPurchaseId,
        PurchaseKind,
        PurchaseStatus,
        Decimal,
        jiff_sqlx::Timestamp,
    )> = sqlx::query_as(
        "SELECT id, kind, status, amount, created_at \
         FROM credit_purchases \
         WHERE user_id = $1 AND community_id = $2 \
         ORDER BY (status = ANY($3)) DESC, \
                  created_at DESC, id DESC LIMIT 50",
    )
    .bind(actor.0.user_id)
    .bind(actor.0.community_id)
    .bind(PurchaseStatus::non_terminal())
    .fetch_all(pool)
    .await
    .context("Failed to list credit purchases")?;

    Ok(rows
        .into_iter()
        .map(|(id, kind, status, amount, created_at)| {
            payloads::responses::CreditPurchase {
                id,
                kind,
                status,
                amount,
                created_at: created_at.to_jiff(),
            }
        })
        .collect())
}

/// The member's repayable debt: how negative their balance is beyond
/// what pending captures will restore (zero when the effective balance
/// is non-negative). The pending term reuses funding's
/// `pending_captures` so debt derivation and settlement sizing share
/// one definition of "pending".
async fn effective_debt_conn(
    community_id: &CommunityId,
    user_id: &UserId,
    conn: &mut sqlx::PgConnection,
) -> Result<Decimal, StoreError> {
    let (balance,): (Decimal,) = sqlx::query_as(
        "SELECT balance_cached FROM accounts \
         WHERE community_id = $1 AND owner_type = 'member_main' \
           AND owner_id = $2",
    )
    .bind(community_id)
    .bind(user_id)
    .fetch_one(&mut *conn)
    .await
    .context("Failed to load member balance")?;
    let pending =
        super::funding::pending_captures(community_id, user_id, &mut *conn)
            .await?;
    Ok(Decimal::ZERO.max(-(balance + pending)))
}

/// Handle a Checkout-session webhook event (`checkout.session.completed`,
/// `checkout.session.expired`) as a poke: the payload only identifies
/// the purchase (stored session id, falling back to the session
/// metadata's `credit_purchase_id`); the live session and intent state
/// decide via [`converge_purchase`], so delivery order can't matter.
///
/// The stored session id is backfilled first for the pre-store window
/// (a completed event racing the mint's own session-id write).
/// Transient retrieve failures propagate so Stripe redelivers.
pub(crate) async fn handle_session_event(
    event_type: &str,
    event_account: Option<&str>,
    obj: &serde_json::Value,
    stripe_service: &StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let session_id = obj["id"].as_str().ok_or_else(|| {
        StoreError::StripeError("session event without object id".into())
    })?;
    let Some(purchase_id) = find_purchase(
        event_account,
        &obj["metadata"],
        session_id,
        "SELECT id FROM credit_purchases WHERE checkout_session_id = $1",
        pool,
    )
    .await?
    else {
        tracing::debug!(
            session_id,
            event_type,
            "skipping session event for unknown purchase"
        );
        return Ok(());
    };

    match event_type {
        "checkout.session.completed" | "checkout.session.expired" => {
            let Some(event_account) = event_account else {
                tracing::warn!(
                    session_id,
                    event_type,
                    "session event without an envelope account; cannot \
                     retrieve live state"
                );
                return Ok(());
            };
            sqlx::query(
                "UPDATE credit_purchases \
                 SET checkout_session_id = COALESCE(checkout_session_id, $1), \
                     updated_at = $2 \
                 WHERE id = $3",
            )
            .bind(session_id)
            .bind(time_source.now().to_sqlx())
            .bind(purchase_id)
            .execute(pool)
            .await?;
            converge_purchase(
                purchase_id,
                event_account,
                stripe_service,
                time_source,
                pool,
            )
            .await?;
        }
        other => {
            tracing::debug!(other, "unhandled session event type");
        }
    }
    Ok(())
}

/// Handle a PaymentIntent webhook event for a credit purchase (routed
/// here by the `credit_purchase_id` metadata or a stored intent id).
///
/// `succeeded`/`payment_failed`/`canceled` are pokes: the payload only
/// routes, and [`converge_purchase`] re-derives from live state — so a
/// stale event can't strand a row (the failed-then-processing ordering)
/// and in-checkout declines stay retryable (an open session returns the
/// member to payment; only a completed session's failure retires the
/// row).
///
/// `processing` stays payload-applied: it moves `created` →
/// `processing` only, monotone-forward and thus order-safe, and keeping
/// it payload-driven preserves the pre-link window where the intent id
/// arrives via metadata before any session state is stored.
pub(crate) async fn handle_purchase_intent_event(
    event_type: &str,
    event_account: Option<&str>,
    obj: &serde_json::Value,
    stripe_service: &StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let payment_intent_id = obj["id"].as_str().ok_or_else(|| {
        StoreError::StripeError("intent event without object id".into())
    })?;
    let Some(purchase_id) = find_purchase(
        event_account,
        &obj["metadata"],
        payment_intent_id,
        "SELECT id FROM credit_purchases WHERE payment_intent_id = $1",
        pool,
    )
    .await?
    else {
        tracing::warn!(
            payment_intent_id,
            event_type,
            "purchase intent event carries metadata but no matching row"
        );
        return Ok(());
    };

    match event_type {
        "payment_intent.processing" => {
            if set_purchase_status(
                &purchase_id,
                &[PurchaseStatus::Created],
                PurchaseStatus::Processing,
                Some(payment_intent_id),
                time_source,
                pool,
            )
            .await?
            {
                tracing::info!(
                    %purchase_id,
                    payment_intent_id,
                    "purchase payment processing (delayed method settling)"
                );
            }
        }
        "payment_intent.succeeded"
        | "payment_intent.payment_failed"
        | "payment_intent.canceled" => {
            let Some(event_account) = event_account else {
                tracing::warn!(
                    payment_intent_id,
                    event_type,
                    "purchase intent event without an envelope account; \
                     cannot retrieve live state"
                );
                return Ok(());
            };
            // Backfill the intent linkage for the pre-link window, then
            // poke: live state decides.
            sqlx::query(
                "UPDATE credit_purchases \
                 SET payment_intent_id = COALESCE(payment_intent_id, $1), \
                     updated_at = $2 \
                 WHERE id = $3",
            )
            .bind(payment_intent_id)
            .bind(time_source.now().to_sqlx())
            .bind(purchase_id)
            .execute(pool)
            .await?;
            converge_purchase(
                purchase_id,
                event_account,
                stripe_service,
                time_source,
                pool,
            )
            .await?;
        }
        other => {
            tracing::debug!(other, "unhandled purchase intent event type");
        }
    }
    Ok(())
}

/// Whether an intent id is already linked to a purchase row. Lets the
/// dispatcher route a metadata-cleared PaymentIntent event (whose
/// `credit_purchase_id` a community removed at the dashboard) to the
/// purchase handler via the stored `payment_intent_id`, instead of the
/// funding handler silently dropping it. Unscoped like
/// `find_purchase`'s stored-id branch: the id match itself proves the
/// event refers to our object.
pub(crate) async fn intent_belongs_to_purchase(
    payment_intent_id: &str,
    pool: &PgPool,
) -> Result<bool, StoreError> {
    let row: Option<(CreditPurchaseId,)> = sqlx::query_as(
        "SELECT id FROM credit_purchases WHERE payment_intent_id = $1",
    )
    .bind(payment_intent_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

/// Resolve a webhook object to a purchase row via the shared two-step
/// lookup — the stored Stripe id matched by `stored_id_sql` (the
/// session id at mint, the intent id by a previously authorized event),
/// falling back to the `credit_purchase_id` metadata scoped to the
/// event's account. `connect::resolve_webhook_row` owns the logic and
/// the account-scoping rationale.
async fn find_purchase(
    event_account: Option<&str>,
    metadata: &serde_json::Value,
    stripe_id: &str,
    stored_id_sql: &str,
    pool: &PgPool,
) -> Result<Option<CreditPurchaseId>, StoreError> {
    super::connect::resolve_webhook_row(
        event_account,
        metadata,
        "credit_purchase_id",
        stripe_id,
        stored_id_sql,
        "SELECT cp.id FROM credit_purchases cp \
         JOIN communities c ON cp.community_id = c.id \
         WHERE cp.id = $1 AND c.stripe_account_id = $2",
        pool,
    )
    .await
}

/// Issue the credits for a succeeded purchase and finalize the row, in
/// one transaction.
///
/// `received_minor` is the amount the intent actually collected
/// (Stripe's `amount_received`, in minor units), from the webhook
/// payload or a live retrieve. The row lock serializes redundant
/// deliveries; the entry's idempotency key is the once-only guarantee
/// across them.
///
/// A success landing on a locally expired/failed row still issues (the
/// money was collected) with a warning, as does a debt settlement that
/// leaves the balance positive (the debt shrank since the session was
/// created). A success collecting an amount other than the row's face
/// value issues nothing and retires the row as failed.
async fn issue_purchase(
    purchase_id: CreditPurchaseId,
    payment_intent_id: &str,
    received_minor: i64,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let mut ttx = super::locks::TrackedTx::begin(pool).await?;

    #[derive(sqlx::FromRow)]
    struct PurchaseRow {
        community_id: CommunityId,
        user_id: UserId,
        kind: PurchaseKind,
        status: PurchaseStatus,
        amount: Decimal,
        currency_minor_units: i16,
    }
    let row: PurchaseRow = sqlx::query_as(
        "SELECT cp.community_id, cp.user_id, cp.kind, cp.status, \
                cp.amount, c.currency_minor_units \
         FROM credit_purchases cp \
         JOIN communities c ON cp.community_id = c.id \
         WHERE cp.id = $1 FOR UPDATE OF cp",
    )
    .bind(purchase_id)
    .fetch_one(&mut **ttx.tx())
    .await
    .context("Failed to load purchase for issuance")?;

    if row.status == PurchaseStatus::Succeeded {
        return Ok(());
    }

    // Issue the credits the money paid for, not the face value we
    // stored: a mis-routed, replayed, or forged event that collected
    // less than the row's amount must not mint the full amount.
    // amount_received is authoritative — the settled total after any
    // partial capture.
    let expected_minor =
        payloads::to_minor_units(row.amount, row.currency_minor_units)
            .ok_or(StoreError::InvalidCurrencyConfiguration)?;
    if received_minor != expected_minor {
        // Don't issue an amount the payment didn't collect: the row's
        // amount is what the rest of the system reasons about (the
        // settlement debt equality, the issuance entry, the balance
        // check below), so issuing anything else would leave that
        // state incoherent. Retiring the row rather than leaving it
        // pending releases the settlement lock and stops it showing as
        // in-flight; the collected money needs manual reconciliation,
        // hence the error log. Acking is right either way — retrying
        // won't change the collected amount. Unguarded (`from` = ALL):
        // the row lock serializes, and the retirement applies whatever
        // the prior status.
        set_purchase_status(
            &purchase_id,
            &PurchaseStatus::ALL,
            PurchaseStatus::Failed,
            Some(payment_intent_id),
            time_source,
            &mut **ttx.tx(),
        )
        .await?;
        ttx.commit().await?;
        tracing::error!(
            %purchase_id,
            payment_intent_id,
            received_minor,
            expected_minor,
            "purchase succeeded for an amount other than its face value; \
             refusing to issue and marking failed"
        );
        return Ok(());
    }
    if matches!(row.status, PurchaseStatus::Expired | PurchaseStatus::Failed) {
        tracing::warn!(
            %purchase_id,
            prior_status = %row.status,
            "purchase succeeded after local expiry/failure; issuing anyway"
        );
    }

    super::currency::create_purchase_issuance_entry_tx(
        &row.community_id,
        &row.user_id,
        row.amount,
        payment_intent_id,
        &purchase_id,
        row.kind,
        time_source,
        &mut ttx,
    )
    .await?;

    // Unguarded (`from` = ALL): the row lock serializes, and a late
    // success deliberately overwrites a local expired/failed.
    set_purchase_status(
        &purchase_id,
        &PurchaseStatus::ALL,
        PurchaseStatus::Succeeded,
        Some(payment_intent_id),
        time_source,
        &mut **ttx.tx(),
    )
    .await?;

    if row.kind == PurchaseKind::DebtSettlement {
        let (balance,): (Decimal,) = sqlx::query_as(
            "SELECT balance_cached FROM accounts \
             WHERE community_id = $1 AND owner_type = 'member_main' \
               AND owner_id = $2",
        )
        .bind(row.community_id)
        .bind(row.user_id)
        .fetch_one(&mut **ttx.tx())
        .await?;
        if balance > Decimal::ZERO {
            tracing::warn!(
                %purchase_id,
                %balance,
                "debt settlement left a positive balance (debt shrank \
                 after the session was created)"
            );
        }
    }

    ttx.commit().await?;
    tracing::info!(
        %purchase_id,
        payment_intent_id,
        user_id = %row.user_id,
        community_id = %row.community_id,
        amount = %row.amount,
        kind = %row.kind,
        "credit purchase succeeded and issued"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every cell of the (local × live session) product returns its
    /// intended classification — the table written down as a test, so a
    /// changed cell is a conscious edit here too (mirroring
    /// `convergence::tests::transition_table`).
    #[test]
    fn session_transition_table() {
        use LiveSessionStatus as S;
        use PurchaseStatus as L;
        use SessionAction as A;

        const SESSIONS: [LiveSessionStatus; 5] =
            [S::Open, S::Complete, S::Expired, S::Missing, S::Unknown];

        let expected = |local: L, session: S| -> A {
            match (local, session) {
                (L::Created, S::Open) => A::Waiting,
                (L::Created, S::Complete) => A::IntentStage,
                (L::Created, S::Expired | S::Missing) => A::RetireExpired,
                (L::Created, S::Unknown) => A::Anomaly,
                _ => A::IntentStage,
            }
        };

        for local in PurchaseStatus::ALL {
            for session in SESSIONS {
                assert_eq!(
                    decide_session(local, session),
                    expected(local, session),
                    "cell ({local}, {session})"
                );
            }
        }
    }

    /// Every cell of the (local × live PaymentIntent) product returns
    /// its intended classification.
    #[test]
    fn intent_transition_table() {
        use IntentAction as A;
        use LivePiStatus as P;
        use PurchaseStatus as L;

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
                (L::Created, P::Succeeded) => A::Issue,
                (L::Created, P::Processing) => A::MarkProcessing,
                (L::Created, P::Canceled | P::RequiresPaymentMethod) => {
                    A::MarkFailed
                }
                (L::Created, P::Missing) => A::MarkExpired,
                (L::Created, P::RequiresConfirmation | P::RequiresAction) => {
                    A::Waiting
                }

                (L::Processing, P::Succeeded) => A::Issue,
                (L::Processing, P::Processing) => A::InSync,
                (L::Processing, P::Canceled | P::RequiresPaymentMethod) => {
                    A::MarkFailed
                }
                (L::Processing, P::Missing) => A::MarkExpired,

                (L::Succeeded, P::Succeeded | P::Missing) => A::InSync,

                (L::Failed | L::Expired, P::Succeeded) => A::Issue,
                (L::Failed | L::Expired, P::Processing) => A::Waiting,
                (
                    L::Failed,
                    P::Canceled | P::RequiresPaymentMethod | P::Missing,
                ) => A::InSync,
                (
                    L::Expired,
                    P::Canceled
                    | P::Missing
                    | P::RequiresPaymentMethod
                    | P::RequiresConfirmation
                    | P::RequiresAction,
                ) => A::InSync,

                _ => A::Anomaly,
            }
        };

        for local in PurchaseStatus::ALL {
            for live in LIVES {
                assert_eq!(
                    decide_intent(local, live),
                    expected(local, live),
                    "cell ({local}, {live})"
                );
            }
        }
    }
}
