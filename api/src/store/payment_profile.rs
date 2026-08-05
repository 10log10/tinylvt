//! Platform-side saved payment methods for members (stripe-backed
//! backed_credits mode).
//!
//! Each user gets one platform-level Stripe customer (created lazily,
//! race-guarded) and at most one stored card, saved through a Checkout session
//! in setup mode so 3DS/SCA happens on Stripe's page. `user_payment_profiles`
//! holds display metadata only — the card lives in Stripe. Persistence
//! converges through one adopt routine with two triggers: the platform
//! webhook's `checkout.session.completed` (mode=setup) and the return-redirect
//! poll (`get_payment_profile` syncs when the row has a customer but no card),
//! so UX never depends on webhook latency. Adopting a new card detaches the
//! previously stored one — one stored method per user by construction.

use anyhow::Context;
use jiff_sqlx::ToSqlx;
use payloads::{ApiError, UserId};
use sqlx::PgPool;

use super::StoreError;
use crate::AppConfig;
use crate::stripe_service::{CardPaymentMethod, StripeService};
use crate::time::TimeSource;

/// Start a Checkout setup session for saving a card; returns the
/// Stripe-hosted URL. Creates the user's platform customer on first use.
pub async fn create_card_setup_session(
    user_id: &UserId,
    stripe_service: &StripeService,
    app_config: &AppConfig,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<String, StoreError> {
    let customer_id =
        get_or_create_customer(user_id, stripe_service, time_source, pool)
            .await?;

    let success_url =
        format!("{}/profile?card_setup=success", app_config.base_url);
    let cancel_url =
        format!("{}/profile?card_setup=canceled", app_config.base_url);

    let url = stripe_service
        .create_setup_checkout_session(
            &customer_id,
            user_id,
            &success_url,
            &cancel_url,
        )
        .await
        .map_err(StoreError::stripe)?;

    Ok(url)
}

/// The user's payment settings. When a customer exists, syncs the card
/// from Stripe first — the return-redirect poll path that makes both
/// first-save and replace display independent of webhook latency (a
/// replace leaves the old card stored, so "no card stored" can't gate
/// the sync). Costs one Stripe list call per profile view, only for
/// users who have started card setup.
pub async fn get_payment_profile(
    user_id: &UserId,
    stripe_service: &StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<payloads::responses::UserPaymentProfile, StoreError> {
    let customer_id: Option<String> = sqlx::query_scalar(
        "SELECT stripe_customer_id \
         FROM user_payment_profiles WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to get payment profile")?;

    if let Some(customer_id) = &customer_id {
        sync_card_from_stripe(
            user_id,
            customer_id,
            stripe_service,
            time_source,
            pool,
        )
        .await?;
    }

    #[derive(sqlx::FromRow)]
    struct StoredCard {
        card_brand: Option<String>,
        card_last4: Option<String>,
        card_exp_month: Option<i16>,
        card_exp_year: Option<i16>,
    }

    let stored: Option<StoredCard> = sqlx::query_as(
        "SELECT card_brand, card_last4, card_exp_month, card_exp_year \
         FROM user_payment_profiles WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to re-read payment profile")?;
    let budget_holds: bool =
        sqlx::query_scalar("SELECT budget_holds FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await
            .context("Failed to read hold strategy")?;

    let card = stored.and_then(|s| {
        Some(payloads::responses::SavedCard {
            brand: s.card_brand?,
            last4: s.card_last4?,
            exp_month: s.card_exp_month?,
            exp_year: s.card_exp_year?,
        })
    });

    Ok(payloads::responses::UserPaymentProfile { card, budget_holds })
}

/// Remove the saved card: detach in Stripe, clear the row. Allowed even
/// while the card backs live authorizations — it's the member's global
/// escape hatch from further card activity, without hunting down which
/// auctions in which communities are minting holds. Existing holds are
/// unaffected (capture settles the authorization on the connected
/// account's cloned method, not the saved card), standing bids stay
/// binding, and the raise machinery degrades to the no-card path: bids
/// can no longer be raised beyond current backing. The UI confirms with
/// a modal spelling out those consequences.
pub async fn remove_payment_method(
    user_id: &UserId,
    stripe_service: &StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let payment_method_id: Option<String> = sqlx::query_scalar(
        "SELECT payment_method_id FROM user_payment_profiles \
         WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to get payment profile")?
    .flatten();
    let Some(payment_method_id) = payment_method_id else {
        return Err(ApiError::NoSavedPaymentMethod.into());
    };

    stripe_service
        .detach_payment_method(&payment_method_id)
        .await
        .map_err(StoreError::stripe)?;

    // Compare-and-clear on the method we actually detached. A card
    // adoption committing between the read above and this clear (the
    // profile-view sync runs on every view, so concurrency here is
    // ordinary) would otherwise have its row wiped while its method
    // stays attached at Stripe — and the next sync would silently
    // re-adopt it, so Remove would appear to succeed and then undo
    // itself.
    let cleared = sqlx::query(
        "UPDATE user_payment_profiles \
         SET payment_method_id = NULL, card_brand = NULL, \
             card_last4 = NULL, card_exp_month = NULL, \
             card_exp_year = NULL, consented_at = NULL, updated_at = $1 \
         WHERE user_id = $2 AND payment_method_id = $3",
    )
    .bind(time_source.now().to_sqlx())
    .bind(user_id)
    .bind(&payment_method_id)
    .execute(pool)
    .await
    .context("Failed to clear payment profile")?;

    if cleared.rows_affected() == 0 {
        // A replacement landed first. The detach above still applied to
        // the old method, which is what the member asked to remove; the
        // newly stored card is untouched and correct.
        tracing::info!(
            %user_id,
            payment_method_id,
            "detached payment method was already replaced; row left alone"
        );
    } else {
        tracing::info!(%user_id, "saved payment method removed");
    }
    Ok(())
}

/// Grant or revoke a community's permission to charge the member's
/// saved card (merchant-initiated holds). Revocation never releases
/// existing backing — standing auths back binding bids and release via
/// settlement/cancel; it only stops new authorizations and raises.
/// Emits `CardChargeGrantChanged` so open auction pages refetch the
/// grant-dependent sections.
pub async fn update_card_charge_grant(
    actor: &super::ValidatedMember,
    grant: payloads::requests::ChargeGrant,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let community_id = actor.0.community_id;
    let user_id = actor.0.user_id;
    let now = time_source.now().to_sqlx();
    let mut tx = pool.begin().await.context("Failed to begin transaction")?;
    sqlx::query(
        "UPDATE community_members \
         SET card_charges_granted_at = CASE WHEN $1 THEN $2 END, \
             updated_at = $2 \
         WHERE community_id = $3 AND user_id = $4",
    )
    .bind(grant == payloads::requests::ChargeGrant::Granted)
    .bind(now)
    .bind(community_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .context("Failed to update card charge grant")?;
    crate::pubsub::emit(
        &mut tx,
        &payloads::AuctionEvent::CardChargeGrantChanged { user_id },
    )
    .await?;
    tx.commit().await.context("Failed to commit grant update")?;
    tracing::info!(
        %user_id, %community_id, ?grant,
        "card charge grant updated"
    );
    Ok(())
}

/// Whether the member has granted this community permission to charge
/// their saved card (read straight off the validated membership row).
pub fn card_charge_granted(actor: &super::ValidatedMember) -> bool {
    actor.0.card_charges_granted_at.is_some()
}

/// Update the user's authorization sizing strategy.
pub async fn update_hold_strategy(
    user_id: &UserId,
    budget_holds: bool,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE users SET budget_holds = $1, updated_at = $2 WHERE id = $3",
    )
    .bind(budget_holds)
    .bind(time_source.now().to_sqlx())
    .bind(user_id)
    .execute(pool)
    .await
    .context("Failed to update hold strategy")?;
    tracing::info!(%user_id, budget_holds, "hold strategy updated");
    Ok(())
}

/// Handle `checkout.session.completed` with `mode = "setup"` from the
/// platform webhook endpoint: resolve the user from the session's
/// metadata and adopt their newest card.
pub async fn handle_setup_session_completed(
    pool: &PgPool,
    time_source: &TimeSource,
    stripe_service: &StripeService,
    session: &serde_json::Value,
) -> Result<(), StoreError> {
    let user_id: UserId = session["metadata"]["user_id"]
        .as_str()
        .and_then(|s| s.parse::<uuid::Uuid>().ok())
        .map(UserId)
        .ok_or_else(|| {
            StoreError::StripeError(
                "setup session without user_id metadata".into(),
            )
        })?;
    let customer_id = session["customer"].as_str().ok_or_else(|| {
        StoreError::StripeError("setup session without customer".into())
    })?;

    sync_card_from_stripe(
        &user_id,
        customer_id,
        stripe_service,
        time_source,
        pool,
    )
    .await
}

/// Handle `payment_method.detached` from the platform webhook: clear the
/// stored card if it was the detached method. Our own remove/replace
/// flows already clear or overwrite the row (this write is a no-op then);
/// the event matters for detachments made outside the app, which would
/// otherwise leave stale display metadata that the profile sync can't
/// correct (it no-ops on an empty method list).
pub async fn handle_payment_method_detached(
    pool: &PgPool,
    time_source: &TimeSource,
    payment_method: &serde_json::Value,
) -> Result<(), StoreError> {
    let Some(payment_method_id) = payment_method["id"].as_str() else {
        return Err(StoreError::StripeError(
            "payment_method.detached without object id".into(),
        ));
    };
    let cleared = sqlx::query(
        "UPDATE user_payment_profiles \
         SET payment_method_id = NULL, card_brand = NULL, \
             card_last4 = NULL, card_exp_month = NULL, \
             card_exp_year = NULL, consented_at = NULL, updated_at = $1 \
         WHERE payment_method_id = $2",
    )
    .bind(time_source.now().to_sqlx())
    .bind(payment_method_id)
    .execute(pool)
    .await
    .context("Failed to clear detached payment method")?;
    if cleared.rows_affected() > 0 {
        tracing::info!(
            payment_method_id,
            "stored card cleared after out-of-band detach"
        );
    }
    Ok(())
}

/// Get or create the user's platform-level Stripe customer, persisting
/// the id race-guarded (first insert wins; the loser's customer is
/// orphaned in Stripe, harmlessly).
async fn get_or_create_customer(
    user_id: &UserId,
    stripe_service: &StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<String, StoreError> {
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT stripe_customer_id FROM user_payment_profiles \
         WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to read payment profile")?;
    if let Some(id) = existing {
        return Ok(id);
    }

    let username: String =
        sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await
            .context("Failed to get user")?;

    let customer_id = stripe_service
        .create_user_customer(&username, user_id)
        .await
        .map_err(StoreError::stripe)?;

    let rows = sqlx::query(
        "INSERT INTO user_payment_profiles \
         (user_id, stripe_customer_id, created_at, updated_at) \
         VALUES ($1, $2, $3, $3) \
         ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(&customer_id)
    .bind(time_source.now().to_sqlx())
    .execute(pool)
    .await
    .context("Failed to persist payment profile")?;

    if rows.rows_affected() == 0 {
        // A concurrent request won the race — use their customer.
        let winner: String = sqlx::query_scalar(
            "SELECT stripe_customer_id FROM user_payment_profiles \
             WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .context("Failed to read winning customer id")?;
        Ok(winner)
    } else {
        tracing::info!(
            %user_id,
            customer_id = %customer_id,
            "Stripe customer persisted for user"
        );
        Ok(customer_id)
    }
}

/// Adopt the customer's newest card from Stripe: persist its display
/// metadata and stamp `consented_at` (the merchant-initiated-charge
/// consent captured at setup). If a different card was stored before,
/// detach it — one stored method per user. No-op when Stripe has no
/// card (setup not completed) or the stored card is already current.
///
/// Runs on every profile view, so concurrent calls are routine. The
/// adoption is a compare-and-swap on the previously stored method: a
/// caller whose read is stale by write time stores nothing and skips
/// the detach, leaving the winner (which observed the same predecessor)
/// to retire it.
async fn sync_card_from_stripe(
    user_id: &UserId,
    customer_id: &str,
    stripe_service: &StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let methods = stripe_service
        .list_card_payment_methods(customer_id)
        .await
        .map_err(StoreError::stripe)?;
    let Some(newest) = methods.first() else {
        return Ok(());
    };

    let stored: Option<String> = sqlx::query_scalar(
        "SELECT payment_method_id FROM user_payment_profiles \
         WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to read stored payment method")?
    .flatten();
    if stored.as_deref() == Some(newest.id.as_str()) {
        return Ok(());
    }

    // Compare-and-swap against the value read above: concurrent syncs
    // (webhook and profile view, or two views) can list different
    // "newest" cards mid-replacement, and a blind write would let the
    // loser overwrite the winner — ending with a stored card that the
    // winner's detach below then removes at Stripe, leaving the member
    // with no usable card. The loser writes nothing and returns.
    if !persist_card(user_id, newest, stored.as_deref(), time_source, pool)
        .await?
    {
        tracing::info!(
            %user_id,
            payment_method_id = %newest.id,
            "concurrent card sync won; leaving stored card alone"
        );
        return Ok(());
    }
    tracing::info!(
        %user_id,
        payment_method_id = %newest.id,
        "saved card adopted from Stripe"
    );

    // One stored method per user: the replaced card has no further use.
    // Best-effort — a failed detach leaves a dangling but chargeable-by
    // -nobody method on the customer, and the next replace retries
    // nothing (it detaches its own predecessor), so just log.
    if let Some(old_id) = stored
        && let Err(e) = stripe_service.detach_payment_method(&old_id).await
    {
        tracing::warn!(
            %user_id,
            payment_method_id = %old_id,
            error = ?e,
            "failed to detach replaced payment method"
        );
    }

    Ok(())
}

/// Store the card's display metadata and consent stamp, but only while
/// the row still holds `expected` — the value the caller's adoption
/// decision was made from. Returns whether the write applied; `false`
/// means a concurrent writer moved the row on and this caller's view is
/// stale.
async fn persist_card(
    user_id: &UserId,
    card: &CardPaymentMethod,
    expected: Option<&str>,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<bool, StoreError> {
    // IS NOT DISTINCT FROM so the first-save case (expected NULL)
    // compares as equal rather than dropping the row from the match.
    let updated = sqlx::query(
        "UPDATE user_payment_profiles \
         SET payment_method_id = $1, card_brand = $2, card_last4 = $3, \
             card_exp_month = $4, card_exp_year = $5, consented_at = $6, \
             updated_at = $6 \
         WHERE user_id = $7 \
           AND payment_method_id IS NOT DISTINCT FROM $8",
    )
    .bind(&card.id)
    .bind(&card.brand)
    .bind(&card.last4)
    .bind(card.exp_month)
    .bind(card.exp_year)
    .bind(time_source.now().to_sqlx())
    .bind(user_id)
    .bind(expected)
    .execute(pool)
    .await
    .context("Failed to persist card")?;
    Ok(updated.rows_affected() > 0)
}
