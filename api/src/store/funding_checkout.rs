//! Unsaved-card funding authorizations (backed_credits mode): a member
//! fronts their maximum spend for one auction through a payment-mode
//! Checkout session with manual capture, charged directly on the
//! community's connected account — inline card entry, 3DS on Stripe's
//! page, nothing stored, no card-charge grant. The completed session's
//! PaymentIntent becomes the auction's active authorization; the
//! uncaptured remainder releases at settlement or loss.
//!
//! The session's local mirror is a `funding_intents` row in
//! `checkout_created` (origin `member_checkout`) — the credit-purchase
//! row pattern, deliberately not the `pending` order machinery: an
//! order is an immutable server-confirmed instruction (replayed at
//! Stripe, aged out after an hour, canceled in place when undersized),
//! while a Checkout session lives up to 24h with nothing to replay, and
//! a cancel-in-place would strand a session the member is mid-payment
//! on.
//!
//! Flow: [`create_funding_checkout`] validates, retires the member's
//! stale predecessor for the auction (expiring its session at Stripe —
//! atomic against completion, so a paid session is never superseded),
//! pre-inserts the row (its id seeds the session idempotency key and
//! rides in session + PaymentIntent metadata), mints the session, and
//! returns its URL. Completion activates through the ordinary webhook
//! adoption path (`payment_intent.amount_capturable_updated` →
//! `funding::adopt_confirmed_intent` → `activate_intent_tx`), whose
//! supersede flip serves both raise-by-fresh-checkout and deliberate
//! hold reduction — the mint and the activation's shrink guard both
//! apply the replacement floor (raise the hold, or stay at or above the
//! auction's commitment less its balance backing). Two concurrently paid
//! sessions are safe — the second supersedes the first and the worker releases
//! it, so no purchase-style survivor election is needed. A completion the
//! auction can no longer use (ended, the hold's window can't cover the
//! deadline, or it would under-back committed bids) is canceled at
//! Stripe with a `CheckoutReleased` notice — there is no finalize to
//! defer to.
//! Session webhooks ([`handle_funding_session_event`]) and the hourly
//! reconciliation sweep converge stragglers through
//! [`converge_checkout`]; rows canceled from confirmed-dead sessions
//! are stamped reconciled so the orphan sweep skips them, while
//! wind-down cancels leave the stamp unset — a late completion then
//! resolves through the canceled-order-hold webhook arm. An open
//! checkout on an ended auction is left to session expiry plus the
//! sweep rather than canceled at conclusion, so a member who pays
//! near the end still gets the released notice instead of silence.

use payloads::{
    ApiError, AuctionId, CommunityId, CurrencyMode, FundingIntentId,
    FundingIntentStatus, UserId,
};
use rust_decimal::Decimal;
use sqlx::PgPool;

use super::funding::emit_funding_changed;
use super::{StoreError, funding};
use crate::AppConfig;
use crate::stripe_service::{
    CheckoutSessionParams, ExpireSessionOutcome, LiveSessionStatus,
    SessionMode, StripeService,
};
use crate::time::TimeSource;
use jiff_sqlx::ToSqlx;

use super::purchases::MINT_GRACE_MINUTES;

/// Start an unsaved-card funding authorization: validate, retire the
/// member's stale checkout for this auction, pre-insert the
/// `checkout_created` intent row, mint the Checkout session, and return
/// its URL for redirect.
pub async fn create_funding_checkout(
    auction_id: &AuctionId,
    user_id: &UserId,
    amount: Decimal,
    app_config: &AppConfig,
    stripe_service: &StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<String, StoreError> {
    let (auction, _) = super::auction::get_validated_auction(
        auction_id,
        user_id,
        payloads::PermissionLevel::Member,
        pool,
    )
    .await?;
    if auction.end_at.is_some() {
        return Err(ApiError::AuctionAlreadyEnded.into());
    }
    // Same advance gate as the saved-card pre-authorize: no hold is
    // minted that the age rule is destined to cancel. The comparison is
    // against mint time; a session paid up to 24h later still passes
    // adoption's own window check or is released with notice.
    if let Some(start_at) = auction.start_at {
        let authorize_from = start_at
            .checked_sub(funding::preauth_advance())
            .map_err(anyhow::Error::from)?;
        if time_source.now() < authorize_from {
            return Err(ApiError::PreauthNotYetOpen { authorize_from }.into());
        }
    }

    let community_id =
        super::get_site_community_id(&auction.site_id, pool).await?;
    #[derive(sqlx::FromRow)]
    struct CheckoutContext {
        name: String,
        currency_mode: CurrencyMode,
        currency_name: String,
        stripe_account_id: Option<String>,
        stripe_charges_enabled: bool,
        stripe_deauthorized_at: Option<jiff_sqlx::Timestamp>,
    }
    let ctx: CheckoutContext = sqlx::query_as(
        "SELECT name, currency_mode, currency_name, stripe_account_id, \
                stripe_charges_enabled, stripe_deauthorized_at \
         FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_one(pool)
    .await?;
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
    if amount < denomination.stripe_min_charge {
        return Err(ApiError::InvalidPurchaseAmount.into());
    }
    if amount > denomination.stripe_max_charge {
        return Err(ApiError::AmountTooLarge {
            max: denomination.stripe_max_charge,
        }
        .into());
    }
    // Replacement floor: a completion supersedes the live hold
    // outright, so a smaller replacement must still meet what the bid
    // gate would demand of it — the auction's commitment less the
    // balance backing no other auction claims (the reduction grows this
    // auction's balance commitment, which must fit in that backing) —
    // the deliberate over-hold reduction. Refused here so the member
    // doesn't walk the Checkout flow first; this read is advisory (no
    // account lock) — activation re-checks under the processing and
    // account-row locks (the session can complete much later, after the
    // hold, commitment, or backing moved) and releases a completion
    // that no longer fits.
    let live = funding::active_intent(auction_id, user_id, pool)
        .await?
        .filter(|i| i.is_live_auth(time_source.now()));
    if let Some(current) = live.and_then(|i| i.authorized_amount)
        && amount < current
    {
        let mut tx = pool.begin().await?;
        let floor = funding::replacement_floor_tx(
            &community_id,
            auction_id,
            user_id,
            time_source,
            &mut tx,
        )
        .await?;
        if amount < floor {
            return Err(
                ApiError::HoldReplacementTooSmall { current, floor }.into()
            );
        }
    }

    retire_stale_checkout(
        auction_id,
        user_id,
        &account_id,
        stripe_service,
        time_source,
        pool,
    )
    .await?;

    let now = time_source.now();
    let mut tx = pool.begin().await?;
    // Hold the community row FOR KEY SHARE until the insert commits, so
    // the row lands strictly before delete_community's in-flight
    // re-check or the mint fails on the deleted community — the intent
    // FK chain reaches communities only through auctions and sites
    // (`ensure_pending_intent_tx` has the full rationale).
    let community_exists: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM communities WHERE id = $1 FOR KEY SHARE",
    )
    .bind(community_id)
    .fetch_optional(&mut *tx)
    .await?;
    if community_exists.is_none() {
        return Err(ApiError::CommunityNotFound.into());
    }
    // The partial unique index is the serialization point for
    // concurrent mints: the loser sees the winner's row (which the
    // retire pass above could not have missed unless it was inserted
    // meanwhile) and bails.
    let inserted: Option<FundingIntentId> = sqlx::query_scalar(
        "INSERT INTO funding_intents \
         (auction_id, user_id, status, origin, requested_amount, \
          created_at, updated_at) \
         VALUES ($1, $2, 'checkout_created', 'member_checkout', $3, $4, $4) \
         ON CONFLICT (auction_id, user_id) \
             WHERE status = 'checkout_created' \
         DO NOTHING \
         RETURNING id",
    )
    .bind(auction_id)
    .bind(user_id)
    .bind(amount)
    .bind(now.to_sqlx())
    .fetch_optional(&mut *tx)
    .await?;
    let Some(intent_id) = inserted else {
        return Err(ApiError::CheckoutAlreadyProcessing.into());
    };
    // Commit before the Stripe call: no data row waits on the network.
    // A crash here leaves a session-less row that the mint grace window
    // retires.
    tx.commit().await?;

    let return_url = format!("{}/auctions/{}", app_config.base_url, auction_id);
    let session = match stripe_service
        .create_payment_checkout_session(CheckoutSessionParams {
            account_id: &account_id,
            mode: SessionMode::Funding {
                intent_id: &intent_id,
            },
            amount_minor,
            currency: &denomination.iso.to_lowercase(),
            description: &format!(
                "Card authorization for bidding — {}",
                ctx.name
            ),
            success_url: &format!("{return_url}?funding=success"),
            cancel_url: &format!("{return_url}?funding=canceled"),
        })
        .await
    {
        Ok(session) => session,
        Err(e) => {
            // No session exists for this row, so no webhook will ever
            // retire it; retire it now (best-effort — the mint grace
            // window backstops) or the member is locked out until the
            // grace passes. Reconciled: confirmed no Stripe object.
            if let Err(mark_err) = sqlx::query(
                "UPDATE funding_intents \
                 SET status = 'canceled', reconciled_at = $1, \
                     updated_at = $1 \
                 WHERE id = $2 AND status = 'checkout_created'",
            )
            .bind(time_source.now().to_sqlx())
            .bind(intent_id)
            .execute(pool)
            .await
            {
                tracing::warn!(
                    %intent_id,
                    "failed to retire checkout row after mint failure: \
                     {mark_err:#}"
                );
            }
            return Err(StoreError::stripe(e));
        }
    };

    let (stored_status,): (FundingIntentStatus,) = sqlx::query_as(
        "UPDATE funding_intents \
         SET checkout_session_id = $1, updated_at = $2 \
         WHERE id = $3 \
         RETURNING status",
    )
    .bind(&session.session_id)
    .bind(time_source.now().to_sqlx())
    .bind(intent_id)
    .fetch_one(pool)
    .await?;
    // The row can move on mid-mint (a wind-down cancel, or the grace
    // sweep against a pathologically slow mint). Kill the session
    // rather than hand out a URL whose completion nothing owns.
    if stored_status != FundingIntentStatus::CheckoutCreated {
        let outcome = stripe_service
            .expire_checkout_session(&account_id, &session.session_id)
            .await
            .map_err(StoreError::stripe)?;
        tracing::warn!(
            %intent_id,
            session_id = %session.session_id,
            ?outcome,
            "checkout row retired during mint; killed the fresh session"
        );
        return Err(ApiError::CheckoutAlreadyProcessing.into());
    }

    tracing::info!(
        %intent_id, %auction_id, %user_id, %amount,
        "funding checkout session created"
    );
    Ok(session.url)
}

/// Retire the member's previous checkout for this auction so at most
/// one completable session exists per (auction, member).
///
/// A stored session is expired at Stripe first — atomic against
/// completion: a session the member already paid reports `Completed`
/// and this bails with `CheckoutAlreadyProcessing` (that payment's
/// webhook activates it). A session-less row within the mint grace
/// window marks a mint still in flight and also bails; past the window
/// its creation crashed and the row is retired.
async fn retire_stale_checkout(
    auction_id: &AuctionId,
    user_id: &UserId,
    account_id: &str,
    stripe_service: &StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let existing: Option<(
        FundingIntentId,
        Option<String>,
        jiff_sqlx::Timestamp,
    )> = sqlx::query_as(
        "SELECT id, checkout_session_id, created_at FROM funding_intents \
         WHERE auction_id = $1 AND user_id = $2 \
           AND status = 'checkout_created'",
    )
    .bind(auction_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let Some((intent_id, session_id, created_at)) = existing else {
        return Ok(());
    };

    let Some(session_id) = session_id else {
        let grace_cutoff =
            time_source.now() - jiff::Span::new().minutes(MINT_GRACE_MINUTES);
        if created_at.to_jiff() >= grace_cutoff {
            return Err(ApiError::CheckoutAlreadyProcessing.into());
        }
        // Reconciled: no session was ever minted, so no Stripe object
        // can exist for this row (the create's idempotency key seed is
        // the row id, and its session would have been stored).
        sqlx::query(
            "UPDATE funding_intents \
             SET status = 'canceled', reconciled_at = $1, updated_at = $1 \
             WHERE id = $2 AND status = 'checkout_created'",
        )
        .bind(time_source.now().to_sqlx())
        .bind(intent_id)
        .execute(pool)
        .await?;
        tracing::warn!(
            %intent_id,
            "retired a checkout row that never stored a session id \
             (creation failed or crashed mid-mint)"
        );
        return Ok(());
    };

    let outcome = stripe_service
        .expire_checkout_session(account_id, &session_id)
        .await
        .map_err(StoreError::stripe)?;
    match outcome {
        ExpireSessionOutcome::Expired => {
            retire_row_for_dead_session(&intent_id, time_source, pool).await?;
            tracing::info!(
                %intent_id,
                session_id,
                "expired superseded funding checkout session"
            );
            Ok(())
        }
        ExpireSessionOutcome::Completed => {
            tracing::info!(
                %intent_id,
                session_id,
                "previous funding checkout session was already paid; \
                 deferring to its webhook"
            );
            Err(ApiError::CheckoutAlreadyProcessing.into())
        }
    }
}

/// Cancel a `checkout_created` row whose session is confirmed dead at
/// Stripe (expired, or missing on a since-replaced account). Reconciled:
/// a dead session was never completed, so no hold exists to recover.
async fn retire_row_for_dead_session(
    intent_id: &FundingIntentId,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE funding_intents \
         SET status = 'canceled', reconciled_at = $1, updated_at = $1 \
         WHERE id = $2 AND status = 'checkout_created'",
    )
    .bind(time_source.now().to_sqlx())
    .bind(intent_id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() > 0 {
        emit_funding_changed(intent_id, &mut tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Whether a Checkout session id belongs to a funding intent row — the
/// session-event dispatch's stored-id fallback, mirroring the intent
/// dispatch's `intent_belongs_to_purchase`.
pub(crate) async fn session_belongs_to_funding(
    session_id: &str,
    pool: &PgPool,
) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM funding_intents \
         WHERE checkout_session_id = $1)",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await?)
}

/// Handle a Checkout-session webhook event for a funding row (routed
/// here by the `funding_intent_id` session metadata or a stored session
/// id) as a poke: the live session and intent state decide via
/// [`converge_checkout`], so delivery order can't matter.
///
/// The stored session id is backfilled first for the pre-store window
/// (a completed event racing the mint's own session-id write).
/// Transient retrieve failures propagate so Stripe redelivers.
pub(crate) async fn handle_funding_session_event(
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
    let Some(intent_id) =
        find_checkout_intent(event_account, &obj["metadata"], session_id, pool)
            .await?
    else {
        tracing::debug!(
            session_id,
            event_type,
            "skipping session event for unknown funding intent"
        );
        return Ok(());
    };

    match event_type {
        "checkout.session.completed" | "checkout.session.expired" => {
            let Some(event_account) = event_account else {
                tracing::warn!(
                    session_id,
                    event_type,
                    "funding session event without an envelope account; \
                     cannot retrieve live state"
                );
                return Ok(());
            };
            sqlx::query(
                "UPDATE funding_intents \
                 SET checkout_session_id = COALESCE(checkout_session_id, $1), \
                     updated_at = $2 \
                 WHERE id = $3",
            )
            .bind(session_id)
            .bind(time_source.now().to_sqlx())
            .bind(intent_id)
            .execute(pool)
            .await?;
            converge_checkout(
                &intent_id,
                event_account,
                stripe_service,
                time_source,
                pool,
            )
            .await?;
        }
        other => {
            tracing::debug!(other, "unhandled funding session event type");
        }
    }
    Ok(())
}

/// Locate the funding row for a session event via the shared two-step
/// lookup — the stored session id first (self-authorizing — we wrote
/// it), falling back to the session's `funding_intent_id` metadata
/// scoped to the event's envelope account for the pre-store window.
/// `connect::resolve_webhook_row` owns the logic and the
/// account-scoping rationale.
async fn find_checkout_intent(
    event_account: Option<&str>,
    metadata: &serde_json::Value,
    session_id: &str,
    pool: &PgPool,
) -> Result<Option<FundingIntentId>, StoreError> {
    super::connect::resolve_webhook_row(
        event_account,
        metadata,
        "funding_intent_id",
        session_id,
        "SELECT id FROM funding_intents WHERE checkout_session_id = $1",
        "SELECT fi.id FROM funding_intents fi \
         JOIN auctions a ON fi.auction_id = a.id \
         JOIN sites s ON a.site_id = s.id \
         JOIN communities c ON s.community_id = c.id \
         WHERE fi.id = $1 AND c.stripe_account_id = $2",
        pool,
    )
    .await
}

/// Converge stale `checkout_created` funding rows from live session
/// state — the backstop for permanently missed session/intent webhooks
/// (the stuck-purchase sweep's counterpart for the unsaved-card flow),
/// run by the hourly reconciliation pass. Selection past the worker-lag
/// grace keeps routine completions on the webhook path; a still-open
/// session converges to a no-op until it expires or is paid.
pub(crate) async fn sweep_stuck_checkouts(
    pool: &PgPool,
    time_source: &TimeSource,
    stripe_service: &StripeService,
    report: &mut super::reconciliation::ReconciliationReport,
) -> anyhow::Result<()> {
    use super::reconciliation::InvariantViolation;

    let grace = super::reconciliation::worker_lag_cutoff(time_source)?;
    let candidates: Vec<(FundingIntentId, CommunityId, String)> =
        sqlx::query_as(&format!(
            "SELECT fi.id, s.community_id, c.stripe_account_id \
             FROM funding_intents fi \
             JOIN auctions a ON fi.auction_id = a.id \
             JOIN sites s ON a.site_id = s.id \
             JOIN communities c ON s.community_id = c.id \
             WHERE fi.status = 'checkout_created' \
               AND fi.updated_at < $1 \
               AND {account_reachable}",
            account_reachable = super::connect::account_reachable_sql("c"),
        ))
        .bind(grace.to_sqlx())
        .fetch_all(pool)
        .await?;

    report.checkout_candidates = candidates.len();
    for (intent_id, community_id, account_id) in candidates {
        if let Err(e) = converge_checkout(
            &intent_id,
            &account_id,
            stripe_service,
            time_source,
            pool,
        )
        .await
        {
            report.violations.push((
                community_id,
                InvariantViolation::CheckoutProbeFailed {
                    intent_id,
                    detail: format!("{e:#}"),
                },
            ));
        }
    }
    Ok(())
}

/// Converge one `checkout_created` row from live Stripe state — the
/// shared decision used by the session-event pokes and the hourly
/// reconciliation sweep.
///
/// An open session leaves the row (the member may still pay; in-session
/// declines stay retryable); a dead or missing session retires it; a
/// completed session links its PaymentIntent and hands the pair to the
/// intent-level convergence table, which routes a live hold to adoption
/// (activating it, or releasing a too-late one with notice). Rows that
/// already left `checkout_created` no-op.
pub async fn converge_checkout(
    intent_id: &FundingIntentId,
    account_id: &str,
    stripe_service: &StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let row: Option<(
        FundingIntentStatus,
        Option<String>,
        jiff_sqlx::Timestamp,
        AuctionId,
        UserId,
    )> = sqlx::query_as(
        "SELECT status, checkout_session_id, created_at, auction_id, \
                user_id \
         FROM funding_intents WHERE id = $1",
    )
    .bind(intent_id)
    .fetch_optional(pool)
    .await?;
    let Some((status, session_id, created_at, auction_id, user_id)) = row
    else {
        return Ok(());
    };
    if status != FundingIntentStatus::CheckoutCreated {
        return Ok(());
    }
    let Some(session_id) = session_id else {
        let grace_cutoff =
            time_source.now() - jiff::Span::new().minutes(MINT_GRACE_MINUTES);
        if created_at.to_jiff() < grace_cutoff {
            sqlx::query(
                "UPDATE funding_intents \
                 SET status = 'canceled', reconciled_at = $1, \
                     updated_at = $1 \
                 WHERE id = $2 AND status = 'checkout_created'",
            )
            .bind(time_source.now().to_sqlx())
            .bind(intent_id)
            .execute(pool)
            .await?;
            tracing::warn!(
                %intent_id,
                "retired a checkout row that never stored a session id \
                 (creation failed or crashed mid-mint)"
            );
        }
        return Ok(());
    };

    let session = stripe_service
        .retrieve_checkout_session(account_id, &session_id)
        .await
        .map_err(StoreError::stripe)?;
    let session_status = session
        .as_ref()
        .map(|s| s.status)
        .unwrap_or(LiveSessionStatus::Missing);
    match session_status {
        LiveSessionStatus::Open => Ok(()),
        LiveSessionStatus::Missing => {
            retire_row_for_dead_session(intent_id, time_source, pool).await?;
            tracing::warn!(
                %intent_id,
                session_id,
                "funding checkout session is gone from the account \
                 (since-replaced account); retired"
            );
            Ok(())
        }
        LiveSessionStatus::Expired => {
            retire_row_for_dead_session(intent_id, time_source, pool).await?;
            tracing::info!(
                %intent_id,
                session_id,
                "funding checkout session expired; row retired"
            );
            Ok(())
        }
        LiveSessionStatus::Complete => {
            let Some(payment_intent_id) =
                session.and_then(|s| s.payment_intent_id)
            else {
                tracing::warn!(
                    %intent_id,
                    session_id,
                    "completed funding session reports no PaymentIntent"
                );
                return Ok(());
            };
            sqlx::query(
                "UPDATE funding_intents \
                 SET payment_intent_id = COALESCE(payment_intent_id, $1), \
                     updated_at = $2 \
                 WHERE id = $3",
            )
            .bind(&payment_intent_id)
            .bind(time_source.now().to_sqlx())
            .bind(intent_id)
            .execute(pool)
            .await?;
            let retrieved = stripe_service
                .retrieve_payment_intent(account_id, &payment_intent_id)
                .await
                .map_err(StoreError::stripe)?;
            let report = super::convergence::converge_intent(
                intent_id,
                &auction_id,
                &user_id,
                account_id,
                retrieved.as_ref(),
                super::convergence::ConvergePool::api(pool),
                time_source,
                stripe_service,
            )
            .await?;
            tracing::debug!(
                %intent_id,
                payment_intent_id,
                ?report,
                "completed funding session converged"
            );
            Ok(())
        }
        LiveSessionStatus::Unknown => {
            tracing::warn!(
                %intent_id,
                session_id,
                "funding checkout session in unrecognized state; leaving"
            );
            Ok(())
        }
    }
}

/// The member's open checkout amount for an auction, if any — the
/// funding view's "authorization in progress" display.
pub(crate) async fn checkout_pending_amount(
    auction_id: &AuctionId,
    user_id: &UserId,
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<Option<Decimal>, StoreError> {
    Ok(sqlx::query_scalar(
        "SELECT requested_amount FROM funding_intents \
         WHERE auction_id = $1 AND user_id = $2 \
           AND status = 'checkout_created'",
    )
    .bind(auction_id)
    .bind(user_id)
    .fetch_optional(executor)
    .await?)
}

/// Cancel a departing member's open checkouts (all communities when
/// `community_id` is None), in the membership-deletion transaction. The
/// Stripe-side sessions stay payable until their own expiry — no Stripe
/// call belongs in this transaction — so a late completion mints a hold
/// on a canceled row, which the canceled-order-hold webhook arm then
/// releases; the unset `reconciled_at` keeps that path armed.
pub(crate) async fn cancel_member_checkouts_tx(
    community_id: Option<&CommunityId>,
    user_id: &UserId,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    let canceled: Vec<(FundingIntentId,)> = sqlx::query_as(
        "UPDATE funding_intents fi \
         SET status = 'canceled', updated_at = $1 \
         FROM auctions a \
         JOIN sites s ON a.site_id = s.id \
         WHERE fi.auction_id = a.id AND fi.user_id = $2 \
           AND ($3::uuid IS NULL OR s.community_id = $3) \
           AND fi.status = 'checkout_created' \
         RETURNING fi.id",
    )
    .bind(time_source.now().to_sqlx())
    .bind(user_id)
    .bind(community_id)
    .fetch_all(&mut **tx)
    .await?;
    for (intent_id,) in canceled {
        tracing::info!(
            %intent_id, %user_id,
            "canceled departing member's open funding checkout"
        );
        emit_funding_changed(&intent_id, tx).await?;
    }
    Ok(())
}

/// Release a completed checkout authorization the auction can no longer
/// use — ended, or the hold's card window can't cover the deadline (the
/// checkout counterpart of adoption's leave-pending, which defers to a
/// finalize that doesn't exist here).
///
/// Cancels the hold at Stripe first (via `funding::cancel_hold`'s
/// shared idempotency key; a failure propagates for webhook
/// redelivery), then terminalizes the row with the `CheckoutReleased`
/// notice — the member may have closed the tab believing they were
/// backed.
pub(crate) async fn release_late_checkout(
    intent_id: &FundingIntentId,
    payment_intent_id: &str,
    account_id: &str,
    authorized_amount: Decimal,
    stripe_service: &StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    super::funding::cancel_hold(
        account_id,
        payment_intent_id,
        intent_id,
        stripe_service,
    )
    .await
    .map_err(|e| StoreError::StripeError(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct ReleaseCtx {
        auction_id: AuctionId,
        user_id: UserId,
        community_name: String,
        currency_symbol: String,
        currency_minor_units: i16,
    }
    let ctx: ReleaseCtx = sqlx::query_as(
        "SELECT fi.auction_id, fi.user_id, c.name AS community_name, \
                c.currency_symbol, c.currency_minor_units \
         FROM funding_intents fi \
         JOIN auctions a ON fi.auction_id = a.id \
         JOIN sites s ON a.site_id = s.id \
         JOIN communities c ON s.community_id = c.id \
         WHERE fi.id = $1",
    )
    .bind(intent_id)
    .fetch_one(pool)
    .await?;

    let now = time_source.now().to_sqlx();
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE funding_intents \
         SET status = 'canceled', \
             payment_intent_id = COALESCE(payment_intent_id, $1), \
             reconciled_at = $2, updated_at = $2 \
         WHERE id = $3 AND status = 'checkout_created'",
    )
    .bind(payment_intent_id)
    .bind(now)
    .bind(intent_id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() > 0 {
        super::notifications::enqueue_notification_tx(
            &ctx.user_id,
            &format!("checkout_released:{intent_id}"),
            &super::notifications::NotificationParams::CheckoutReleased {
                community_name: ctx.community_name,
                auction_id: ctx.auction_id,
                amount: payloads::format_amount(
                    &ctx.currency_symbol,
                    ctx.currency_minor_units,
                    authorized_amount,
                ),
            },
            time_source,
            &mut *tx,
        )
        .await?;
        emit_funding_changed(intent_id, &mut tx).await?;
    }
    tx.commit().await?;
    tracing::info!(
        %intent_id,
        payment_intent_id,
        "released a checkout authorization the auction can no longer use"
    );
    Ok(())
}
