//! Stripe Connect onboarding for communities (stripe-backed backed_credits
//! mode).
//!
//! Each community gets its own connected account (Standard-equivalent
//! controller properties — the community is merchant of record, pays its own
//! fees, has a full dashboard) created lazily on the first connect request,
//! with the id persisted race-guarded like `stripe_customer_id`. Onboarding
//! happens on Stripe-hosted pages via Account Links; account standing converges
//! from `account.updated` events on the dedicated Connect webhook endpoint
//! (each triggering a live re-fetch — retried event payloads can be stale),
//! with API-error detection as the belt-and-suspenders for deauthorization (a
//! disconnected account's webhooks stop arriving). An account the platform has
//! lost access to (deleted or disconnected in Stripe) is unrecoverable, so the
//! next connect request replaces it with a freshly created account.

use anyhow::Context;
use jiff_sqlx::ToSqlx;
use payloads::{ApiError, CommunityId, CurrencyMode, StripeConnectStatus};
use sqlx::PgPool;

use super::{StoreError, ValidatedMember};
use crate::AppConfig;
use crate::stripe_service::StripeService;
use crate::time::TimeSource;

/// SQL boolean over `communities` columns qualified by `alias`: the
/// connected account can take charges — present, charges-enabled, and
/// not deauthorized. Every query site shares this one definition. Pass
/// the table's alias (`"c"`) or `""` for unqualified columns. The Rust
/// twin is [`connected_account_operational`].
pub(crate) fn account_operational_sql(alias: &str) -> String {
    let p = if alias.is_empty() {
        String::new()
    } else {
        format!("{alias}.")
    };
    format!(
        "({p}stripe_account_id IS NOT NULL AND {p}stripe_charges_enabled \
         AND {p}stripe_deauthorized_at IS NULL)"
    )
}

/// SQL boolean over `communities` columns qualified by `alias`: the
/// connected account is API-reachable — present and not deauthorized.
/// Weaker than [`account_operational_sql`]: a merely charges-disabled
/// account (Stripe pausing charges over requirements) can't take new
/// charges but its objects are still fully retrievable and cancelable,
/// so reconciliation probes it.
pub(crate) fn account_reachable_sql(alias: &str) -> String {
    let p = if alias.is_empty() {
        String::new()
    } else {
        format!("{alias}.")
    };
    format!(
        "({p}stripe_account_id IS NOT NULL \
         AND {p}stripe_deauthorized_at IS NULL)"
    )
}

/// Whether a community's connected account can take charges: it exists,
/// charges are enabled, and it isn't deauthorized. The Rust twin of
/// [`account_operational_sql`], for callers that already hold the row.
pub(crate) fn connected_account_operational(
    stripe_account_id: Option<&str>,
    stripe_charges_enabled: bool,
    stripe_deauthorized_at: Option<jiff::Timestamp>,
) -> bool {
    stripe_account_id.is_some()
        && stripe_charges_enabled
        && stripe_deauthorized_at.is_none()
}

/// Connect a community's Stripe account: create the connected account
/// if absent, then return an onboarding Account Link URL to redirect
/// the coleader to.
///
/// Re-invocable at any point (incomplete onboarding, new requirements,
/// reconnect after a dashboard-side disconnect) — account links are
/// single-use and expire, so each call mints a fresh one. A stored
/// account the platform has lost access to (deleted or disconnected in
/// Stripe — unrecoverable, since re-attaching would need OAuth) is
/// replaced with a freshly created one.
pub async fn connect_community_stripe(
    actor: &ValidatedMember,
    stripe_service: &StripeService,
    app_config: &AppConfig,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<String, StoreError> {
    if !actor.0.role.can_manage_stripe_connection() {
        return Err(ApiError::RequiresColeaderPermissions.into());
    }
    let community_id = actor.0.community_id;

    let (name, currency_mode, existing_account_id, deauthorized): (
        String,
        CurrencyMode,
        Option<String>,
        bool,
    ) = sqlx::query_as(
        "SELECT name, currency_mode, stripe_account_id, \
         stripe_deauthorized_at IS NOT NULL \
         FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_one(pool)
    .await
    .context("Failed to get community")?;

    // Card charges denominate real money; the account only makes sense
    // for the stripe-backed mode.
    if currency_mode != CurrencyMode::BackedCredits {
        return Err(ApiError::StripeConnectRequiresBackedCredits.into());
    }

    let had_stored_account = existing_account_id.is_some();
    let account_id = match existing_account_id {
        Some(id) => id,
        None => {
            let id = stripe_service
                .create_connected_account(&name, &community_id)
                .await
                .map_err(StoreError::stripe)?;

            // Persist immediately. WHERE stripe_account_id IS NULL guards
            // against a concurrent connect race.
            let rows = sqlx::query(
                "UPDATE communities SET stripe_account_id = $1 \
                 WHERE id = $2 AND stripe_account_id IS NULL",
            )
            .bind(&id)
            .bind(community_id)
            .execute(pool)
            .await
            .context("Failed to persist stripe account ID")?;

            if rows.rows_affected() == 0 {
                // Another request won the race — use their account.
                let winner: Option<String> = sqlx::query_scalar(
                    "SELECT stripe_account_id FROM communities \
                     WHERE id = $1",
                )
                .bind(community_id)
                .fetch_one(pool)
                .await
                .context("Failed to read winning account ID")?;
                winner.ok_or_else(|| {
                    StoreError::StripeError(
                        "Race: stripe_account_id still NULL".into(),
                    )
                })?
            } else {
                tracing::info!(
                    %community_id,
                    account_id = %id,
                    "Stripe connected account persisted on community"
                );
                id
            }
        }
    };

    // Both URLs land back on the settings page; the query param lets the
    // UI distinguish an expired-link refresh (mint another link) from a
    // completed return (poll status).
    let refresh_url = format!(
        "{}/communities/{}/settings?stripe_connect=refresh",
        app_config.base_url, community_id,
    );
    let return_url = format!(
        "{}/communities/{}/settings?stripe_connect=return",
        app_config.base_url, community_id,
    );

    let url = match stripe_service
        .create_account_link(&account_id, &refresh_url, &return_url)
        .await
    {
        Ok(url) => url,
        Err(e) if had_stored_account && is_account_gone_error(&e) => {
            // The stored account was deleted or disconnected in
            // Stripe; account links can never target it again, so mint
            // a replacement and retry the link once. Only a stored
            // account qualifies — a gone error on an account created
            // moments ago is an anomaly to surface, not to paper over.
            let new_id = replace_gone_account(
                stripe_service,
                pool,
                &name,
                &community_id,
                &account_id,
                time_source,
            )
            .await?;
            return stripe_service
                .create_account_link(&new_id, &refresh_url, &return_url)
                .await
                .map_err(StoreError::stripe);
        }
        Err(e) => {
            return Err(StoreError::stripe(e));
        }
    };

    // A successful account-link creation proves the platform still has
    // API access to the account, so a stale disconnect marker is wrong.
    if deauthorized {
        sqlx::query(
            "UPDATE communities SET stripe_deauthorized_at = NULL \
             WHERE id = $1",
        )
        .bind(community_id)
        .execute(pool)
        .await
        .context("Failed to clear deauthorization marker")?;
        tracing::info!(
            %community_id,
            "cleared Stripe deauthorization marker on reconnect"
        );
    }

    Ok(url)
}

/// The community's Stripe Connect standing for the settings UI. Fetches
/// live account state when connected — doubling as drift reconciliation
/// for `stripe_charges_enabled` — and compares the account's settlement
/// currency against the community's denomination.
pub async fn get_community_stripe_status(
    actor: &ValidatedMember,
    stripe_service: &StripeService,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<payloads::responses::CommunityStripeStatus, StoreError> {
    if !actor.0.role.can_manage_stripe_connection() {
        return Err(ApiError::RequiresColeaderPermissions.into());
    }
    let community_id = actor.0.community_id;

    let (account_id, stored_charges_enabled, deauthorized, currency_name): (
        Option<String>,
        bool,
        bool,
        String,
    ) = sqlx::query_as(
        "SELECT stripe_account_id, stripe_charges_enabled, \
         stripe_deauthorized_at IS NOT NULL, currency_name \
         FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_one(pool)
    .await
    .context("Failed to get community")?;

    let Some(account_id) = account_id else {
        return Ok(payloads::responses::CommunityStripeStatus {
            status: StripeConnectStatus::NotConnected,
            settlement_currency_mismatch: None,
        });
    };
    if deauthorized {
        return Ok(payloads::responses::CommunityStripeStatus {
            status: StripeConnectStatus::Disconnected,
            settlement_currency_mismatch: None,
        });
    }

    let live = match stripe_service.get_account_status(&account_id).await {
        Ok(live) => live,
        Err(e) => {
            if is_account_gone_error(&e) {
                // Belt-and-suspenders deauthorization detection: a
                // gone account's webhooks stop arriving, so the
                // failing API call is the only signal. Recovery (a
                // replacement account) happens when the coleader
                // reconnects via `connect_community_stripe`.
                set_deauthorized(pool, &community_id, &account_id, time_source)
                    .await?;
                return Ok(payloads::responses::CommunityStripeStatus {
                    status: StripeConnectStatus::Disconnected,
                    settlement_currency_mismatch: None,
                });
            }
            return Err(StoreError::stripe(e));
        }
    };

    if live.charges_enabled != stored_charges_enabled {
        sqlx::query(
            "UPDATE communities SET stripe_charges_enabled = $1 \
             WHERE id = $2",
        )
        .bind(live.charges_enabled)
        .bind(community_id)
        .execute(pool)
        .await
        .context("Failed to reconcile stripe_charges_enabled")?;
        tracing::info!(
            %community_id,
            charges_enabled = live.charges_enabled,
            "reconciled stripe_charges_enabled from live account status"
        );
    }

    // The community's currency name is the denomination's ISO code
    // (validated at creation); Stripe reports lowercase.
    let settlement_currency_mismatch = live
        .default_currency
        .filter(|c| !c.eq_ignore_ascii_case(&currency_name));

    let status = if live.charges_enabled {
        StripeConnectStatus::ChargesEnabled
    } else {
        // Covers both unfinished onboarding and outstanding requirements
        // (details submitted, charges still disabled) — both resolve
        // through a fresh onboarding link.
        StripeConnectStatus::OnboardingIncomplete
    };

    Ok(payloads::responses::CommunityStripeStatus {
        status,
        settlement_currency_mismatch,
    })
}

/// Handle an event from the Connect webhook endpoint. Events carry the
/// connected account id in the envelope's `account` field.
pub async fn handle_connect_webhook_event(
    pool: &PgPool,
    time_source: &TimeSource,
    stripe_service: &StripeService,
    event: &serde_json::Value,
) -> Result<(), StoreError> {
    let event_type = event["type"].as_str().unwrap_or("unknown");
    // The connected account the event originated on. Stripe sets this on
    // the envelope; the purchase and funding handlers verify it against
    // the community that owns the referenced row, so a forged
    // credit_purchase_id or funding_intent_id in a foreign account's
    // PaymentIntent can't touch another community's records.
    let event_account = event["account"].as_str();

    match event_type {
        "account.updated" => {
            let obj = &event["data"]["object"];
            let account_id = obj["id"].as_str().ok_or_else(|| {
                StoreError::StripeError(
                    "account.updated without object id".into(),
                )
            })?;

            // Stripe redelivers failed events for days with no ordering
            // guarantee, so the payload's `charges_enabled` may be
            // stale — applying it could regress the flag or clear a
            // real deauthorization marker. Retrieve the account and
            // apply live state instead. This is the one webhook handler
            // that calls Stripe: safe because it holds no locks and no
            // transaction across the retrieve.
            let live = match stripe_service.get_account_status(account_id).await
            {
                Ok(live) => live,
                Err(e) if is_account_gone_error(&e) => {
                    // The account is gone; any deauthorization marker (set by
                    // the deauthorized event or the status probe) must survive
                    // this stale event.
                    tracing::warn!(
                        account_id,
                        "account.updated for an account the platform \
                         can no longer access; leaving stored state \
                         unchanged"
                    );
                    return Ok(());
                }
                // Transient failure: error out so Stripe redelivers.
                Err(e) => {
                    return Err(StoreError::stripe(e));
                }
            };

            // A successful retrieve is proof the platform still has API
            // access to the account, so clear any deauthorization
            // marker in the same write.
            let updated: Option<(CommunityId,)> = sqlx::query_as(
                "UPDATE communities \
                 SET stripe_charges_enabled = $1, \
                     stripe_deauthorized_at = NULL \
                 WHERE stripe_account_id = $2 \
                 RETURNING id",
            )
            .bind(live.charges_enabled)
            .bind(account_id)
            .fetch_optional(pool)
            .await
            .context("Failed to apply account.updated")?;

            match updated {
                Some((community_id,)) => {
                    tracing::info!(
                        %community_id,
                        account_id,
                        charges_enabled = live.charges_enabled,
                        "applied live account state on account.updated"
                    );
                }
                None => {
                    // Every connected account is platform-created for a
                    // community, so an unmatched id is unexpected.
                    tracing::warn!(
                        account_id,
                        "account.updated for unknown connected account"
                    );
                }
            }
        }
        "account.application.deauthorized" => {
            // The connected account id lives in the event envelope; the
            // data object is the application.
            let account_id = event["account"].as_str().ok_or_else(|| {
                StoreError::StripeError(
                    "deauthorized event without account field".into(),
                )
            })?;
            let mut tx = pool.begin().await?;
            let updated: Option<(CommunityId,)> = sqlx::query_as(
                "UPDATE communities SET stripe_deauthorized_at = $1 \
                 WHERE stripe_account_id = $2 \
                   AND stripe_deauthorized_at IS NULL \
                 RETURNING id",
            )
            .bind(time_source.now().to_sqlx())
            .bind(account_id)
            .fetch_optional(&mut *tx)
            .await
            .context("Failed to apply deauthorization")?;
            if let Some((community_id,)) = updated {
                retire_orphan_candidates(&mut *tx, &community_id, time_source)
                    .await?;
                tracing::warn!(
                    %community_id,
                    account_id,
                    "community disconnected the platform in Stripe"
                );
            }
            tx.commit().await?;
        }
        // PaymentIntent events split by metadata: funding-auth intents
        // carry `funding_intent_id`, purchase intents carry
        // `credit_purchase_id`. Each handler ignores objects without
        // its own metadata, so the split is on what the object claims
        // to be.
        "payment_intent.amount_capturable_updated"
        | "payment_intent.canceled"
        | "payment_intent.payment_failed"
        | "payment_intent.processing"
        | "payment_intent.succeeded" => {
            let obj = &event["data"]["object"];
            // Route to the purchase handler when the object claims a
            // purchase via metadata, or when its intent id is already
            // linked to a purchase row (so a purchase whose metadata was
            // cleared at the dashboard still converges through its
            // stored payment_intent_id rather than being dropped by the
            // funding handler). Authorization against the account is the
            // handler's job — metadata only routes.
            let is_purchase = obj["metadata"]["credit_purchase_id"].is_string()
                || match obj["id"].as_str() {
                    Some(pi) => {
                        super::purchases::intent_belongs_to_purchase(pi, pool)
                            .await?
                    }
                    None => false,
                };
            if is_purchase {
                super::purchases::handle_purchase_intent_event(
                    event_type,
                    event_account,
                    obj,
                    stripe_service,
                    time_source,
                    pool,
                )
                .await?;
            } else {
                super::funding::handle_intent_event(
                    event_type,
                    event_account,
                    obj,
                    stripe_service,
                    time_source,
                    pool,
                )
                .await?;
            }
        }
        "checkout.session.completed" | "checkout.session.expired" => {
            let obj = &event["data"]["object"];
            // Session events split by metadata like the PaymentIntent
            // events above: funding sessions carry `funding_intent_id`,
            // purchase sessions `credit_purchase_id`, with the stored
            // session id as the metadata-cleared fallback.
            let is_funding = obj["metadata"]["funding_intent_id"].is_string()
                || match obj["id"].as_str() {
                    Some(sid) => {
                        super::funding_checkout::session_belongs_to_funding(
                            sid, pool,
                        )
                        .await?
                    }
                    None => false,
                };
            if is_funding {
                super::funding_checkout::handle_funding_session_event(
                    event_type,
                    event_account,
                    obj,
                    stripe_service,
                    time_source,
                    pool,
                )
                .await?;
            } else {
                super::purchases::handle_session_event(
                    event_type,
                    event_account,
                    obj,
                    stripe_service,
                    time_source,
                    pool,
                )
                .await?;
            }
        }
        _ => {
            // Deliberately unhandled: `charge.refunded` and dispute
            // events. A community can refund a captured charge from its
            // own Stripe dashboard, but clawing back the corresponding
            // credits is a manual leadership operation, not something
            // the ledger automates: for unspent credits, leadership
            // asks the member for a member→treasury transfer as part of
            // the refund process; for credits already spent on a win,
            // the community revokes the member's access to the won
            // resource out-of-band (auction results are a record of
            // what happened, not a registry of current possession).
            // Automating the reversal would mean negative balances and
            // an unintuitive win-reversal operation for a flow that is
            // community-initiated and self-affecting — the refund comes
            // out of the community's own Stripe balance.
            tracing::trace!(
                event_type,
                "Ignoring unhandled Connect webhook event type"
            );
        }
    }

    Ok(())
}

/// Resolve a webhook `data.object` to the local row it references — the
/// two-step lookup shared by the purchase and funding-checkout event
/// routing. `stored_id_sql` matches the object's Stripe id against the
/// id our own flow stored (`... WHERE <column> = $1`, returning the row
/// id); `account_scoped_sql` matches the row id named by the object's
/// `metadata_key` metadata, scoped to the community owning the event's
/// envelope account (`... WHERE <id> = $1 AND c.stripe_account_id =
/// $2`).
///
/// The stored-id match is deliberately not account-scoped: the stored
/// id was written by us and `data.object.id` is Stripe-serialized, so a
/// match proves the event refers to our object — the only events
/// unscoping newly admits are the same community's own since-replaced
/// account, letting late old-account deliveries converge rows instead
/// of orphaning them, never a foreign community.
///
/// The metadata match exists for the pre-link window (an event arriving
/// before our flow stores the Stripe id) and trusts
/// account-owner-writable metadata, so it requires the row's community
/// to own `event_account` — Stripe sets the envelope account, making it
/// the authorization the forgeable metadata is not. A metadata match
/// against a foreign account (an attempted cross-account forgery), or
/// one with no envelope account to authorize it, is rejected loudly.
pub(crate) async fn resolve_webhook_row<T>(
    event_account: Option<&str>,
    metadata: &serde_json::Value,
    metadata_key: &str,
    stripe_id: &str,
    stored_id_sql: &str,
    account_scoped_sql: &str,
    pool: &PgPool,
) -> Result<Option<T>, StoreError>
where
    T: for<'r> sqlx::Decode<'r, sqlx::Postgres>
        + sqlx::Type<sqlx::Postgres>
        + Send
        + Unpin,
{
    let row: Option<T> = sqlx::query_scalar(stored_id_sql)
        .bind(stripe_id)
        .fetch_optional(pool)
        .await?;
    if let Some(id) = row {
        return Ok(Some(id));
    }

    let Some(meta_id) = metadata[metadata_key]
        .as_str()
        .and_then(|s| s.parse::<uuid::Uuid>().ok())
    else {
        return Ok(None);
    };
    let Some(event_account) = event_account else {
        tracing::warn!(
            stripe_id,
            metadata_key,
            "webhook event without an envelope account; cannot \
             authorize a metadata match"
        );
        return Ok(None);
    };
    let row: Option<T> = sqlx::query_scalar(account_scoped_sql)
        .bind(meta_id)
        .bind(event_account)
        .fetch_optional(pool)
        .await?;
    if row.is_none() {
        // The metadata names a row, but not one on this account: either
        // a stale/unknown id, or a foreign account referencing another
        // community's row.
        tracing::warn!(
            %meta_id,
            metadata_key,
            event_account,
            "webhook metadata does not match a row on the event's \
             account; ignoring"
        );
    }
    Ok(row)
}

/// Whether a Stripe error means the connected account is gone from the
/// platform's perspective — deleted, or disconnected from the
/// community's dashboard. The prose matcher itself lives at the
/// service boundary (`stripe_service::is_account_gone_message`, shared
/// with the intent-call error taxonomy's `AccountGone` class) so the
/// phrase list has one definition.
fn is_account_gone_error(e: &anyhow::Error) -> bool {
    crate::stripe_service::is_account_gone_message(&format!("{e:#}"))
}

/// Replace a community's gone connected account with a freshly created
/// one, resetting onboarding state (charges disabled, deauthorization
/// marker cleared) in the same swap. Returns the account id to use:
/// the new one, or a concurrent replacement's winner. Late webhook
/// events for the old account stop matching the community after the
/// swap — acceptable, since a gone account emits no platform-visible
/// events; the warn/info pair below keeps the transition
/// reconstructible from logs.
///
/// In-flight `created` purchase rows are retired in the swap
/// transaction: their Checkout sessions live on the gone account, so
/// they can never complete through our flow, and a leftover `created`
/// settlement row would make `expire_stale_settlement_sessions` fail
/// against the new account forever, blocking the member's future debt
/// settlements. Orphan-sweep candidates are retired for the same
/// reason — the sweep would otherwise probe the new account for
/// old-account PaymentIntents (this also runs at
/// deauthorization-marker time, but the direct connect-flow
/// replacement path never sets the marker).
async fn replace_gone_account(
    stripe_service: &StripeService,
    pool: &PgPool,
    community_name: &str,
    community_id: &CommunityId,
    gone_account_id: &str,
    time_source: &TimeSource,
) -> Result<String, StoreError> {
    tracing::warn!(
        %community_id,
        gone_account_id,
        "stored connected account is gone from Stripe; replacing"
    );
    let new_id = stripe_service
        .create_connected_account(community_name, community_id)
        .await
        .map_err(StoreError::stripe)?;

    // Compare-and-swap against the gone id so concurrent replacements
    // can't clobber each other.
    let mut tx = pool.begin().await?;
    let rows = sqlx::query(
        "UPDATE communities SET stripe_account_id = $1, \
             stripe_charges_enabled = false, \
             stripe_deauthorized_at = NULL \
         WHERE id = $2 AND stripe_account_id = $3",
    )
    .bind(&new_id)
    .bind(community_id)
    .bind(gone_account_id)
    .execute(&mut *tx)
    .await
    .context("Failed to swap in replacement account")?;

    if rows.rows_affected() == 0 {
        // A concurrent request already replaced it — use theirs. Ours
        // stays orphaned in Stripe: harmless, it was never onboarded.
        let winner: Option<String> = sqlx::query_scalar(
            "SELECT stripe_account_id FROM communities WHERE id = $1",
        )
        .bind(community_id)
        .fetch_one(pool)
        .await
        .context("Failed to read winning replacement account ID")?;
        return winner.ok_or_else(|| {
            StoreError::StripeError(
                "Race: stripe_account_id NULL after replacement".into(),
            )
        });
    }

    let retired = sqlx::query(
        "UPDATE credit_purchases \
         SET status = 'expired', updated_at = $1 \
         WHERE community_id = $2 AND status = 'created'",
    )
    .bind(time_source.now().to_sqlx())
    .bind(community_id)
    .execute(&mut *tx)
    .await
    .context("Failed to retire purchases on the gone account")?;
    retire_orphan_candidates(&mut *tx, community_id, time_source).await?;
    tx.commit().await?;
    if retired.rows_affected() > 0 {
        tracing::warn!(
            %community_id,
            gone_account_id,
            count = retired.rows_affected(),
            "retired in-flight purchases whose Checkout sessions died \
             with the gone account"
        );
    }

    tracing::info!(
        %community_id,
        gone_account_id,
        new_account_id = %new_id,
        "replaced gone Stripe connected account"
    );
    Ok(new_id)
}

/// Mark a community deauthorized after a status probe found `account_id` gone.
/// Conditioned on the probed id still being the stored one: a stale probe
/// racing a concurrent gone-account replacement must not stamp the fresh
/// account (whose orphan candidates the sweep can legitimately probe), so on a
/// miss both the marker and candidate retirement are skipped.
pub async fn set_deauthorized(
    pool: &PgPool,
    community_id: &CommunityId,
    account_id: &str,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE communities SET stripe_deauthorized_at = $1 \
         WHERE id = $2 AND stripe_account_id = $3 \
           AND stripe_deauthorized_at IS NULL",
    )
    .bind(time_source.now().to_sqlx())
    .bind(community_id)
    .bind(account_id)
    .execute(&mut *tx)
    .await
    .context("Failed to set deauthorization marker")?;
    if updated.rows_affected() > 0 {
        retire_orphan_candidates(&mut *tx, community_id, time_source).await?;
        tracing::warn!(
            %community_id,
            account_id,
            "marked community Stripe account deauthorized (account \
             inaccessible via API)"
        );
    } else {
        tracing::info!(
            %community_id,
            account_id,
            "skipped stale deauthorization: probed account no longer stored"
        );
    }
    tx.commit().await?;
    Ok(())
}

/// Stamp `reconciled_at` on a community's outstanding orphan-sweep
/// candidates (canceled, no recorded PaymentIntent, unreconciled). Run
/// whenever the connected account becomes unreachable — deauthorization
/// marker set, or a gone account replaced: any PaymentIntent such a
/// row's create minted lives on the old account, so it can never be
/// probed (and after a replacement the sweep would search the new
/// account and wrongly confirm absence). Without this the rows also sit
/// in the sweep's partial index forever.
async fn retire_orphan_candidates(
    executor: impl sqlx::PgExecutor<'_>,
    community_id: &CommunityId,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let retired = sqlx::query(
        "UPDATE funding_intents fi \
         SET reconciled_at = $1, updated_at = $1 \
         FROM auctions a \
         JOIN sites s ON a.site_id = s.id \
         WHERE fi.auction_id = a.id \
           AND s.community_id = $2 \
           AND fi.status = 'canceled' \
           AND fi.payment_intent_id IS NULL \
           AND fi.reconciled_at IS NULL",
    )
    .bind(time_source.now().to_sqlx())
    .bind(community_id)
    .execute(executor)
    .await
    .context("Failed to retire orphan candidates")?;
    if retired.rows_affected() > 0 {
        tracing::info!(
            %community_id,
            count = retired.rows_affected(),
            "retired orphan-sweep candidates on unreachable account"
        );
    }
    Ok(())
}
