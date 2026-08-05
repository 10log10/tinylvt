//! Reconciliation: the hourly pass — the pure-DB invariant suite plus
//! the correcting sweeps ([`run_reconciliation_pass`]).
//!
//! Each check in the suite is an invariant written down with its legal
//! exceptions, so drift is detected mechanically rather than by reading
//! code. The suite itself corrects nothing; correction lives in the
//! sweeps, whose selection loops run here (orphaned holds, the live-PI
//! cross-check) or in the module owning the rows (`purchases`,
//! `funding_checkout`) while per-row work dispatches to its owner
//! (`funding_flow::sweep_orphaned_hold`, `convergence`,
//! `converge_purchase`, `converge_checkout`). The scheduler keeps only
//! the cadence gate (`run_reconciliation_if_due`).
//!
//! The pure-DB suite runs in a single REPEATABLE READ READ ONLY
//! transaction per community, taking no advisory or row locks. The
//! invariants hold at every commit boundary, and a repeatable-read
//! snapshot is exactly the state after some prefix of committed
//! transactions — so any violation visible in the snapshot is a real
//! committed-state violation, with zero contention against settlement
//! and bid flows. READ COMMITTED would not work: the commitment
//! computation is multi-query, and per-statement snapshots could tear
//! across a concurrent settlement into false findings. A read-only
//! repeatable-read transaction can never abort on serialization.
//!
//! Cadence: `communities.last_reconciliation_at` is a per-community
//! last-run watermark. When any community's watermark is older than the
//! interval, one pass runs for ALL communities and restamps every row —
//! a single hourly pass with one summary log line, deduplicated across
//! instances by an advisory try-lock rather than staggered
//! per-community staleness. Because the gate is global, community
//! creation stamps the watermark current (the column is NOT NULL): a
//! new community has nothing to reconcile, and treating it as due would
//! trip a full pass (every community's suite plus the Stripe
//! cross-check) on every creation.
//! (`refresh_all_community_storage` predates this shape and is intended
//! to migrate into it.)

use std::collections::HashMap;

use jiff::{SignedDuration, Timestamp};
use payloads::{
    AccountId, AuctionId, CommunityId, CreditPurchaseId, FundingIntentId,
    FundingIntentStatus, JournalEntryId, UserId,
};
use rust_decimal::Decimal;
use sqlx::PgPool;

use super::StoreError;
use crate::WorkerPool;
use crate::stripe_service::StripeService;
use crate::time::TimeSource;
use jiff_sqlx::ToSqlx;

/// How often the reconciliation pass runs.
pub(crate) const RECONCILIATION_INTERVAL_HOURS: i64 = 1;

/// How stale an intent row must be before its state counts as drift
/// rather than ordinary worker lag. The worker retry backoff caps at
/// ~2.3h, so a smaller grace would flag rows legitimately pacing
/// retries through a Stripe outage; past 3h a row is genuinely stuck.
pub(crate) const WORKER_LAG_GRACE_HOURS: i64 = 3;

/// `now` minus the worker-lag grace — the staleness cutoff shared by
/// the invariant checks and the stuck-row sweeps: rows last touched
/// before it are genuinely stuck, not pacing retries.
pub(crate) fn worker_lag_cutoff(
    time_source: &TimeSource,
) -> anyhow::Result<Timestamp> {
    time_source
        .now()
        .checked_sub(SignedDuration::from_hours(WORKER_LAG_GRACE_HOURS))
        .map_err(anyhow::Error::from)
}

/// One finding from the invariant suite (or the live cross-check, whose
/// findings the scheduler constructs). Tolerated findings are legal
/// design states reported for volume monitoring; the rest are drift.
#[derive(Debug, Clone)]
pub enum InvariantViolation {
    /// Per-member shortfall: Σ balance commitments (`max(0, commitment − live
    /// auth)` over live auctions) exceeds what balance plus pending captures
    /// can back. Tolerated: the shortfall state is a deliberate design state
    /// the funding flows self-heal from. Its causes are a terminal capture
    /// failure (the lost credit-back leaves debt under standing commitments)
    /// and out-of-band loss of a live auth backing standing bids (dashboard
    /// cancel or Stripe expiry). A decline pause can prolong either until the
    /// member acts or settlement books the gap as debt.
    MemberUnderBacked {
        user_id: UserId,
        balance_commitments: Decimal,
        backing: Decimal,
    },
    /// A journal entry's lines don't sum to zero.
    EntryNotBalanced {
        entry_id: JournalEntryId,
        sum: Decimal,
    },
    /// `balance_cached` disagrees with the account's journal lines.
    BalanceDrift {
        account_id: AccountId,
        cached: Decimal,
        derived: Decimal,
    },
    /// An active `authorized` intent on an auction that ended past the
    /// worker-lag grace — the release/capture marking never ran.
    StaleAuthorizedIntent {
        intent_id: FundingIntentId,
        auction_id: AuctionId,
        ended_at: Timestamp,
    },
    /// A `capture_pending` intent whose capture window lapsed past the
    /// grace — the worker should have captured or failed it.
    OverdueCapture {
        intent_id: FundingIntentId,
        capture_before: Timestamp,
    },
    /// A `stripe_payment` entry without a matching source record
    /// (captured intent for auction entries, succeeded purchase for
    /// purchase entries) or with a mismatched amount.
    PaymentEntryMismatch {
        entry_id: JournalEntryId,
        payment_intent_id: String,
        detail: String,
    },
    /// A `captured` intent with no `stripe_payment` entry for its
    /// PaymentIntent. Safe with no grace: the capture worker writes the
    /// mark and the issuance entry in one transaction.
    CapturedIntentMissingEntry { intent_id: FundingIntentId },
    /// A `succeeded` purchase with no `stripe_payment` entry for its
    /// PaymentIntent (same one-transaction argument as captures).
    SucceededPurchaseMissingEntry { purchase_id: CreditPurchaseId },
    /// Live cross-check: a local intent row disagrees with the
    /// PaymentIntent's state at Stripe in a way convergence has no
    /// transition for (an anomalous pair), or an amount disagrees.
    /// Constructed by the scheduler's cross-check step.
    IntentDrift {
        intent_id: FundingIntentId,
        payment_intent_id: String,
        local_status: FundingIntentStatus,
        stripe_status: String,
        detail: String,
    },
    /// Live cross-check repair: convergence applied a transition
    /// bringing the row in line with Stripe truth. Tolerated (info)
    /// unless money moved outside the app's flow.
    IntentRepaired {
        intent_id: FundingIntentId,
        payment_intent_id: String,
        detail: String,
        out_of_band_money: bool,
    },
    /// Live cross-check: the PaymentIntent retrieve (or the repair
    /// pass) failed, so the row could not be checked this pass.
    IntentProbeFailed {
        intent_id: FundingIntentId,
        payment_intent_id: String,
        detail: String,
    },
    /// Stuck-purchase sweep: convergence failed (a session or intent
    /// retrieve, or the convergence write), so the row could not be
    /// checked this pass.
    PurchaseProbeFailed {
        purchase_id: CreditPurchaseId,
        detail: String,
    },
    /// Stuck-checkout sweep: converging a `checkout_created` funding row
    /// from live session state failed, so the row could not be checked
    /// this pass.
    CheckoutProbeFailed {
        intent_id: FundingIntentId,
        detail: String,
    },
}

impl InvariantViolation {
    /// Whether this finding is a legal design state (reported for
    /// volume, logged at info) rather than drift (logged at error).
    pub fn is_tolerated(&self) -> bool {
        matches!(
            self,
            Self::MemberUnderBacked { .. }
                | Self::IntentRepaired {
                    out_of_band_money: false,
                    ..
                }
        )
    }

    /// Short kebab-case label for the summary log's per-class counts.
    pub fn class(&self) -> &'static str {
        match self {
            Self::MemberUnderBacked { .. } => "member-under-backed",
            Self::EntryNotBalanced { .. } => "entry-not-balanced",
            Self::BalanceDrift { .. } => "balance-drift",
            Self::StaleAuthorizedIntent { .. } => "stale-authorized-intent",
            Self::OverdueCapture { .. } => "overdue-capture",
            Self::PaymentEntryMismatch { .. } => "payment-entry-mismatch",
            Self::CapturedIntentMissingEntry { .. } => {
                "captured-intent-missing-entry"
            }
            Self::SucceededPurchaseMissingEntry { .. } => {
                "succeeded-purchase-missing-entry"
            }
            Self::IntentDrift { .. } => "intent-drift",
            Self::IntentRepaired { .. } => "intent-repaired",
            Self::IntentProbeFailed { .. } => "intent-probe-failed",
            Self::PurchaseProbeFailed { .. } => "purchase-probe-failed",
            Self::CheckoutProbeFailed { .. } => "checkout-probe-failed",
        }
    }
}

/// Findings and counters from one full reconciliation pass, the suite's
/// output for the summary log and for tests using the suite as an
/// oracle (`TestApp::reconcile`).
#[derive(Debug)]
pub struct ReconciliationReport {
    pub communities_checked: usize,
    pub violations: Vec<(CommunityId, InvariantViolation)>,
    pub orphan_candidates: usize,
    pub orphans_resolved: usize,
    pub holds_canceled: usize,
    pub intents_cross_checked: usize,
    pub purchase_candidates: usize,
    pub checkout_candidates: usize,
}

impl ReconciliationReport {
    /// Non-tolerated findings — actual drift, as opposed to reported
    /// shortfall states.
    pub fn errors(
        &self,
    ) -> impl Iterator<Item = &(CommunityId, InvariantViolation)> {
        self.violations.iter().filter(|(_, v)| !v.is_tolerated())
    }
}

/// One full reconciliation pass over all communities, ungated (tests
/// call this directly via `TestApp::reconcile`): the pure-DB invariant
/// suite per community, the orphaned-hold sweep, and the live-PI
/// cross-check. Per-community failures log and continue; findings are
/// logged here (error for drift, info for tolerated shortfalls) plus
/// one summary line — the pass's single hourly log line when clean.
pub async fn run_reconciliation_pass(
    pool: &PgPool,
    worker_pool: &WorkerPool,
    time_source: &TimeSource,
    stripe_service: &StripeService,
) -> anyhow::Result<ReconciliationReport> {
    let mut report = ReconciliationReport {
        communities_checked: 0,
        violations: Vec::new(),
        orphan_candidates: 0,
        orphans_resolved: 0,
        holds_canceled: 0,
        intents_cross_checked: 0,
        purchase_candidates: 0,
        checkout_candidates: 0,
    };

    let communities: Vec<CommunityId> =
        sqlx::query_scalar("SELECT id FROM communities")
            .fetch_all(pool)
            .await?;
    for community_id in &communities {
        match check_community_invariants(community_id, time_source, pool).await
        {
            Ok(findings) => {
                report.communities_checked += 1;
                report
                    .violations
                    .extend(findings.into_iter().map(|f| (*community_id, f)));
            }
            Err(e) => {
                tracing::error!(?community_id, "invariant check failed: {e:#}")
            }
        }
    }

    sweep_orphaned_intents(
        pool,
        worker_pool,
        time_source,
        stripe_service,
        &mut report,
    )
    .await?;

    cross_check_intents(
        pool,
        worker_pool,
        time_source,
        stripe_service,
        &mut report,
    )
    .await?;

    super::purchases::sweep_stuck_purchases(
        pool,
        time_source,
        stripe_service,
        &mut report,
    )
    .await?;

    super::funding_checkout::sweep_stuck_checkouts(
        pool,
        time_source,
        stripe_service,
        &mut report,
    )
    .await?;

    let mut class_counts: HashMap<&'static str, usize> = HashMap::new();
    for (community_id, violation) in &report.violations {
        *class_counts.entry(violation.class()).or_default() += 1;
        if violation.is_tolerated() {
            tracing::info!(?community_id, ?violation, "tolerated shortfall");
        } else {
            tracing::error!(?community_id, ?violation, "invariant violated");
        }
    }
    let mut classes: Vec<String> = class_counts
        .iter()
        .map(|(class, count)| format!("{class}:{count}"))
        .collect();
    classes.sort();
    tracing::info!(
        communities = report.communities_checked,
        violations = classes.join(",").as_str(),
        orphan_candidates = report.orphan_candidates,
        orphans_resolved = report.orphans_resolved,
        holds_canceled = report.holds_canceled,
        intents_cross_checked = report.intents_cross_checked,
        purchase_candidates = report.purchase_candidates,
        checkout_candidates = report.checkout_candidates,
        "reconciliation pass complete"
    );
    Ok(report)
}

/// Run the pure-DB invariant suite for one community in a single
/// repeatable-read read-only snapshot (see the module docs for why that
/// isolation level and no locks), returning findings without logging.
/// Ledger integrity runs for every currency mode; the funding checks
/// only for backed_credits.
pub async fn check_community_invariants(
    community_id: &CommunityId,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<Vec<InvariantViolation>, StoreError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;

    let mut findings = Vec::new();
    check_ledger_integrity(community_id, &mut findings, &mut tx).await?;
    if super::funding::is_backed_mode(community_id, &mut *tx).await? {
        check_member_backing(community_id, time_source, &mut findings, &mut tx)
            .await?;
        check_intent_states(community_id, time_source, &mut findings, &mut tx)
            .await?;
        check_payment_entries(community_id, &mut findings, &mut tx).await?;
    }
    tx.commit().await?;
    Ok(findings)
}

/// Ledger integrity: every entry's lines sum to zero, and each
/// account's `balance_cached` re-derives exactly from its lines. Both
/// are application-maintained (no DB constraint), both exact — the
/// per-line balance update commits atomically with the line insert in
/// `create_entry`.
async fn check_ledger_integrity(
    community_id: &CommunityId,
    findings: &mut Vec<InvariantViolation>,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    let unbalanced: Vec<(JournalEntryId, Decimal)> = sqlx::query_as(
        "SELECT je.id, SUM(jl.amount) FROM journal_entries je \
         JOIN journal_lines jl ON jl.entry_id = je.id \
         WHERE je.community_id = $1 \
         GROUP BY je.id HAVING SUM(jl.amount) <> 0",
    )
    .bind(community_id)
    .fetch_all(&mut **tx)
    .await?;
    for (entry_id, sum) in unbalanced {
        findings.push(InvariantViolation::EntryNotBalanced { entry_id, sum });
    }

    let drifted: Vec<(AccountId, Decimal, Decimal)> = sqlx::query_as(
        "SELECT a.id, a.balance_cached, COALESCE(SUM(jl.amount), 0) \
         FROM accounts a \
         LEFT JOIN journal_lines jl ON jl.account_id = a.id \
         WHERE a.community_id = $1 \
         GROUP BY a.id \
         HAVING a.balance_cached <> COALESCE(SUM(jl.amount), 0)",
    )
    .bind(community_id)
    .fetch_all(&mut **tx)
    .await?;
    for (account_id, cached, derived) in drifted {
        findings.push(InvariantViolation::BalanceDrift {
            account_id,
            cached,
            derived,
        });
    }
    Ok(())
}

/// Per-member backing: Σ balance commitments over live auctions must stay
/// within max(0, balance + Σ capture_pending). Candidates come from
/// live-auction bidders UNION intent holders, so bids with no card history
/// still surface. The balance commitments reuse `member_balance_commitments_tx`
/// — the enforcement-side computation — so the checker can't disagree with the
/// gates on bid valuation or auth liveness; the N+1 query shape is fine at
/// hourly cadence. This is the only funding invariant left: unallocated balance
/// is derived, so it cannot drift — only fall short, which is the tolerated
/// self-healing state.
async fn check_member_backing(
    community_id: &CommunityId,
    time_source: &TimeSource,
    findings: &mut Vec<InvariantViolation>,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    let now = time_source.now();
    let members: Vec<(UserId,)> = sqlx::query_as(
        "SELECT DISTINCT p.user_id FROM auctions a \
         JOIN sites s ON a.site_id = s.id \
         JOIN LATERAL ( \
             SELECT fi.user_id FROM funding_intents fi \
             WHERE fi.auction_id = a.id \
             UNION \
             SELECT b.user_id FROM bids b \
             JOIN auction_rounds ar ON b.round_id = ar.id \
             WHERE ar.auction_id = a.id \
         ) p ON TRUE \
         WHERE s.community_id = $1 AND a.end_at IS NULL",
    )
    .bind(community_id)
    .fetch_all(&mut **tx)
    .await?;

    for (user_id,) in members {
        let balance_commitments =
            super::funding::member_balance_commitments_tx(
                community_id,
                &user_id,
                None,
                now,
                tx,
            )
            .await?;
        if balance_commitments <= Decimal::ZERO {
            continue;
        }
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
        let pending =
            super::funding::pending_captures(community_id, &user_id, &mut **tx)
                .await?;
        let backing = (balance + pending).max(Decimal::ZERO);
        if balance_commitments > backing {
            findings.push(InvariantViolation::MemberUnderBacked {
                user_id,
                balance_commitments,
                backing,
            });
        }
    }
    Ok(())
}

/// Funding-intent state: no active `authorized` hold on an auction
/// ended past the worker-lag grace (the settlement marking or worker
/// catchall should have moved it), and no `capture_pending` past its
/// capture window plus grace (the worker captures or terminally fails
/// it well before the window lapses).
async fn check_intent_states(
    community_id: &CommunityId,
    time_source: &TimeSource,
    findings: &mut Vec<InvariantViolation>,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    let cutoff = worker_lag_cutoff(time_source)?;

    let stale: Vec<(FundingIntentId, AuctionId, jiff_sqlx::Timestamp)> =
        sqlx::query_as(
            "SELECT fi.id, fi.auction_id, a.end_at \
             FROM funding_intents fi \
             JOIN auctions a ON fi.auction_id = a.id \
             JOIN sites s ON a.site_id = s.id \
             WHERE s.community_id = $1 AND fi.is_active \
               AND fi.status = 'authorized' \
               AND a.end_at IS NOT NULL AND a.end_at < $2",
        )
        .bind(community_id)
        .bind(cutoff.to_sqlx())
        .fetch_all(&mut **tx)
        .await?;
    for (intent_id, auction_id, ended_at) in stale {
        findings.push(InvariantViolation::StaleAuthorizedIntent {
            intent_id,
            auction_id,
            ended_at: ended_at.to_jiff(),
        });
    }

    let overdue: Vec<(FundingIntentId, jiff_sqlx::Timestamp)> = sqlx::query_as(
        "SELECT fi.id, fi.capture_before \
             FROM funding_intents fi \
             JOIN auctions a ON fi.auction_id = a.id \
             JOIN sites s ON a.site_id = s.id \
             WHERE s.community_id = $1 \
               AND fi.status = 'capture_pending' \
               AND fi.capture_before < $2",
    )
    .bind(community_id)
    .bind(cutoff.to_sqlx())
    .fetch_all(&mut **tx)
    .await?;
    for (intent_id, capture_before) in overdue {
        findings.push(InvariantViolation::OverdueCapture {
            intent_id,
            capture_before: capture_before.to_jiff(),
        });
    }
    Ok(())
}

/// `stripe_payment` ↔ source matching, both populations and both
/// directions. Auction captures (`auction_id` set) must match a
/// `captured` intent by `payment_intent_id` with the member line equal
/// to `capture_amount`; purchases (`auction_id` NULL) must match a
/// `succeeded` purchase row and amount. `payment_intent_id` is
/// deliberately non-unique on entries (future refund recording shares
/// the PI), so matching keys on entry type, not PI uniqueness.
async fn check_payment_entries(
    community_id: &CommunityId,
    findings: &mut Vec<InvariantViolation>,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    /// (entry id, payment intent id, source status, source amount,
    /// entry member-line amount) for a mismatched `stripe_payment`
    /// entry; the source columns are NULL when no source row matched,
    /// and the member-line amount is NULL when the entry has no
    /// member_main line.
    type EntryMismatch = (
        JournalEntryId,
        String,
        Option<String>,
        Option<Decimal>,
        Option<Decimal>,
    );

    // Member line per entry: stripe_payment entries are always the
    // treasury→member issuance pair, so the member_main line is the
    // issued amount. LEFT JOIN so an entry whose lines hit the wrong
    // accounts (no member line at all — balanced, so EntryNotBalanced
    // stays quiet) is flagged rather than dropped from the check.
    let capture_mismatches: Vec<EntryMismatch> = sqlx::query_as(
        "SELECT je.id, je.payment_intent_id, fi.status::TEXT, \
                fi.capture_amount, ml.amount \
         FROM journal_entries je \
         LEFT JOIN LATERAL ( \
             SELECT jl.amount FROM journal_lines jl \
             JOIN accounts ac ON jl.account_id = ac.id \
             WHERE jl.entry_id = je.id \
               AND ac.owner_type = 'member_main' \
         ) ml ON TRUE \
         LEFT JOIN funding_intents fi \
             ON fi.payment_intent_id = je.payment_intent_id \
         WHERE je.community_id = $1 AND je.entry_type = 'stripe_payment' \
           AND je.auction_id IS NOT NULL \
           AND (fi.id IS NULL OR fi.status <> 'captured' \
                OR ml.amount IS NULL \
                OR fi.capture_amount IS DISTINCT FROM ml.amount)",
    )
    .bind(community_id)
    .fetch_all(&mut **tx)
    .await?;
    for (entry_id, payment_intent_id, status, capture_amount, line) in
        capture_mismatches
    {
        let detail = match (status, line) {
            (None, _) => "no funding intent for this PaymentIntent".to_string(),
            (Some(status), None) => format!(
                "intent status {status}, capture_amount {capture_amount:?}, \
                 entry has no member_main line"
            ),
            (Some(status), Some(line)) => format!(
                "intent status {status}, capture_amount {capture_amount:?}, \
                 entry member line {line}"
            ),
        };
        findings.push(InvariantViolation::PaymentEntryMismatch {
            entry_id,
            payment_intent_id,
            detail,
        });
    }

    let purchase_mismatches: Vec<EntryMismatch> = sqlx::query_as(
        "SELECT je.id, je.payment_intent_id, cp.status::TEXT, cp.amount, \
                ml.amount \
         FROM journal_entries je \
         LEFT JOIN LATERAL ( \
             SELECT jl.amount FROM journal_lines jl \
             JOIN accounts ac ON jl.account_id = ac.id \
             WHERE jl.entry_id = je.id \
               AND ac.owner_type = 'member_main' \
         ) ml ON TRUE \
         LEFT JOIN credit_purchases cp \
             ON cp.payment_intent_id = je.payment_intent_id \
         WHERE je.community_id = $1 AND je.entry_type = 'stripe_payment' \
           AND je.auction_id IS NULL \
           AND (cp.id IS NULL OR cp.status <> 'succeeded' \
                OR ml.amount IS NULL \
                OR cp.amount IS DISTINCT FROM ml.amount)",
    )
    .bind(community_id)
    .fetch_all(&mut **tx)
    .await?;
    for (entry_id, payment_intent_id, status, amount, line) in
        purchase_mismatches
    {
        let detail = match (status, line) {
            (None, _) => {
                "no credit purchase for this PaymentIntent".to_string()
            }
            (Some(status), None) => format!(
                "purchase status {status}, amount {amount:?}, entry has no \
                 member_main line"
            ),
            (Some(status), Some(line)) => format!(
                "purchase status {status}, amount {amount:?}, entry member \
                 line {line}"
            ),
        };
        findings.push(InvariantViolation::PaymentEntryMismatch {
            entry_id,
            payment_intent_id,
            detail,
        });
    }

    let unissued_captures: Vec<(FundingIntentId,)> = sqlx::query_as(
        "SELECT fi.id FROM funding_intents fi \
         JOIN auctions a ON fi.auction_id = a.id \
         JOIN sites s ON a.site_id = s.id \
         WHERE s.community_id = $1 AND fi.status = 'captured' \
           AND NOT EXISTS ( \
               SELECT 1 FROM journal_entries je \
               WHERE je.entry_type = 'stripe_payment' \
                 AND je.payment_intent_id = fi.payment_intent_id \
           )",
    )
    .bind(community_id)
    .fetch_all(&mut **tx)
    .await?;
    for (intent_id,) in unissued_captures {
        findings
            .push(InvariantViolation::CapturedIntentMissingEntry { intent_id });
    }

    let unissued_purchases: Vec<(CreditPurchaseId,)> = sqlx::query_as(
        "SELECT cp.id FROM credit_purchases cp \
         WHERE cp.community_id = $1 AND cp.status = 'succeeded' \
           AND NOT EXISTS ( \
               SELECT 1 FROM journal_entries je \
               WHERE je.entry_type = 'stripe_payment' \
                 AND je.payment_intent_id = cp.payment_intent_id \
           )",
    )
    .bind(community_id)
    .fetch_all(&mut **tx)
    .await?;
    for (purchase_id,) in unissued_purchases {
        findings.push(InvariantViolation::SucceededPurchaseMissingEntry {
            purchase_id,
        });
    }
    Ok(())
}

/// Select and resolve orphan candidates: canceled intent rows with no
/// recorded PaymentIntent, unreconciled, younger than the maximum hold
/// lifetime (beyond it any hold expired on its own), and stale past the
/// worker-lag grace (a late webhook gets first claim at filling the
/// PI). Communities without a reachable account (none, or
/// deauthorized) are excluded in the selection — there is no account
/// to probe; a merely charges-disabled account is still fully readable
/// and its rows are swept. Per-candidate work is the pair-locked claim
/// in `funding_flow::sweep_orphaned_hold`.
async fn sweep_orphaned_intents(
    pool: &PgPool,
    worker_pool: &WorkerPool,
    time_source: &TimeSource,
    stripe_service: &StripeService,
    report: &mut ReconciliationReport,
) -> anyhow::Result<()> {
    let now = time_source.now();
    let floor = now
        .checked_sub(jiff::Span::new().hours(7 * 24))
        .map_err(anyhow::Error::from)?;
    let grace = worker_lag_cutoff(time_source)?;
    let candidates: Vec<super::funding_flow::OrphanCandidate> =
        sqlx::query_as(&format!(
            "SELECT fi.id, fi.auction_id, fi.user_id, \
                    c.stripe_account_id, fi.created_at \
             FROM funding_intents fi \
             JOIN auctions a ON fi.auction_id = a.id \
             JOIN sites s ON a.site_id = s.id \
             JOIN communities c ON s.community_id = c.id \
             WHERE fi.status = 'canceled' \
               AND fi.payment_intent_id IS NULL \
               AND fi.reconciled_at IS NULL \
               AND fi.created_at > $1 \
               AND fi.updated_at < $2 \
               AND {account_reachable} \
               AND (fi.worker_failure_count = 0 \
                    OR fi.worker_last_failed_at IS NULL \
                    OR $3 > fi.worker_last_failed_at + {backoff})",
            account_reachable = super::connect::account_reachable_sql("c"),
            backoff = super::backoff_interval_sql("fi.worker_failure_count"),
        ))
        .bind(floor.to_sqlx())
        .bind(grace.to_sqlx())
        .bind(now.to_sqlx())
        .fetch_all(pool)
        .await?;

    report.orphan_candidates = candidates.len();
    let deps = super::funding_flow::FlowDeps {
        worker_pool,
        time_source,
        stripe_service,
    };
    for candidate in &candidates {
        match super::funding_flow::sweep_orphaned_hold(candidate, deps).await {
            Ok(super::funding_flow::OrphanOutcome::Resolved {
                hold_canceled,
            }) => {
                report.orphans_resolved += 1;
                if hold_canceled {
                    report.holds_canceled += 1;
                }
            }
            Ok(super::funding_flow::OrphanOutcome::Skipped) => {}
            Err(e) => tracing::error!(
                intent_id = ?candidate.id,
                "orphan sweep failed: {e:#}"
            ),
        }
    }
    Ok(())
}

/// Cross-check local intent rows against live PaymentIntent state at
/// Stripe for backed communities with a reachable account (a
/// charges-disabled one is still probeable) — the safety net for
/// missed webhooks and workers crashed between the Stripe call and the
/// DB write — and repair what it finds: each row's (local, live) pair
/// goes through `store::convergence`, whose transition table either
/// confirms the pair, applies the owned transition (reported as an
/// `IntentRepaired` finding, info-level unless money moved outside the
/// app's flow), or reports an `Anomaly` pair as `IntentDrift`. No new
/// Stripe fetches beyond the one retrieve per candidate the check
/// already made; retrieve and repair failures become
/// `IntentProbeFailed` findings instead of bare logs. Amount drift on
/// in-sync rows (hold or capture size disagreeing with Stripe) stays a
/// detection-only `IntentDrift`, guarded by a freshness re-read so a
/// row that moved mid-retrieve isn't reported.
async fn cross_check_intents(
    pool: &PgPool,
    worker_pool: &WorkerPool,
    time_source: &TimeSource,
    stripe_service: &StripeService,
    report: &mut ReconciliationReport,
) -> anyhow::Result<()> {
    use payloads::FundingIntentStatus as S;

    use super::convergence::ConvergeReport;

    #[derive(sqlx::FromRow)]
    struct CrossCheckRow {
        id: FundingIntentId,
        payment_intent_id: String,
        status: S,
        authorized_amount: Option<Decimal>,
        capture_amount: Option<Decimal>,
        updated_at: jiff_sqlx::Timestamp,
        auction_id: AuctionId,
        user_id: UserId,
    }

    let communities: Vec<(CommunityId, String, i16)> =
        sqlx::query_as(&format!(
            "SELECT id, stripe_account_id, currency_minor_units \
             FROM communities \
             WHERE currency_mode = 'backed_credits' AND {reachable}",
            reachable = super::connect::account_reachable_sql(""),
        ))
        .fetch_all(pool)
        .await?;

    let recent = time_source
        .now()
        .checked_sub(jiff::Span::new().hours(2 * RECONCILIATION_INTERVAL_HOURS))
        .map_err(anyhow::Error::from)?;

    for (community_id, account_id, minor_units) in communities {
        let rows: Vec<CrossCheckRow> = sqlx::query_as(
            "SELECT fi.id, fi.payment_intent_id, fi.status, \
                    fi.authorized_amount, fi.capture_amount, \
                    fi.updated_at, fi.auction_id, fi.user_id \
             FROM funding_intents fi \
             JOIN auctions a ON fi.auction_id = a.id \
             JOIN sites s ON a.site_id = s.id \
             WHERE s.community_id = $1 \
               AND fi.payment_intent_id IS NOT NULL \
               AND (fi.status IN ('pending', 'authorized', \
                                  'capture_pending', 'release_pending', \
                                  'superseded') \
                    OR fi.updated_at > $2)",
        )
        .bind(community_id)
        .bind(recent.to_sqlx())
        .fetch_all(pool)
        .await?;

        for row in rows {
            report.intents_cross_checked += 1;
            let retrieved = match stripe_service
                .retrieve_payment_intent(&account_id, &row.payment_intent_id)
                .await
            {
                Ok(retrieved) => retrieved,
                Err(e) => {
                    report.violations.push((
                        community_id,
                        InvariantViolation::IntentProbeFailed {
                            intent_id: row.id,
                            payment_intent_id: row.payment_intent_id.clone(),
                            detail: format!("retrieve failed: {e:#}"),
                        },
                    ));
                    continue;
                }
            };

            match super::convergence::converge_intent(
                &row.id,
                &row.auction_id,
                &row.user_id,
                &account_id,
                retrieved.as_ref(),
                super::convergence::ConvergePool::worker(worker_pool),
                time_source,
                stripe_service,
            )
            .await
            {
                Ok(ConvergeReport::Busy | ConvergeReport::Waiting) => continue,
                Ok(ConvergeReport::Converged {
                    note,
                    out_of_band_money,
                }) => {
                    report.violations.push((
                        community_id,
                        InvariantViolation::IntentRepaired {
                            intent_id: row.id,
                            payment_intent_id: row.payment_intent_id.clone(),
                            detail: note.to_string(),
                            out_of_band_money,
                        },
                    ));
                    continue;
                }
                Ok(ConvergeReport::Anomaly { detail }) => {
                    report.violations.push((
                        community_id,
                        InvariantViolation::IntentDrift {
                            intent_id: row.id,
                            payment_intent_id: row.payment_intent_id.clone(),
                            local_status: row.status,
                            stripe_status: retrieved
                                .as_ref()
                                .map(|r| r.status.as_str())
                                .unwrap_or("missing")
                                .to_string(),
                            detail,
                        },
                    ));
                    continue;
                }
                Ok(ConvergeReport::InSync) => {}
                Err(e) => {
                    report.violations.push((
                        community_id,
                        InvariantViolation::IntentProbeFailed {
                            intent_id: row.id,
                            payment_intent_id: row.payment_intent_id.clone(),
                            detail: format!("repair failed: {e:#}"),
                        },
                    ));
                    continue;
                }
            }

            // In-sync status: check the amounts (detection only).
            let Some(retrieved) = retrieved else {
                continue;
            };
            let mut detail = None;
            if row.status == S::Captured {
                let expected_minor = row
                    .capture_amount
                    .and_then(|a| payloads::to_minor_units(a, minor_units));
                if expected_minor
                    .is_some_and(|m| m != retrieved.amount_received_minor)
                {
                    detail = Some(format!(
                        "captured {} at Stripe, capture_amount says {:?}",
                        retrieved.amount_received_minor, expected_minor
                    ));
                }
            }
            if detail.is_none() && row.status == S::Authorized {
                let expected_minor = row
                    .authorized_amount
                    .and_then(|a| payloads::to_minor_units(a, minor_units));
                if expected_minor.is_some_and(|m| m != retrieved.amount_minor) {
                    detail = Some(format!(
                        "hold is {} at Stripe, authorized_amount says {:?}",
                        retrieved.amount_minor, expected_minor
                    ));
                }
            }
            let Some(detail) = detail else {
                continue;
            };

            // Freshness re-read: a webhook or worker may have moved the
            // row while the retrieve was in flight — the drift was
            // already converging, not a finding.
            let fresh: Option<(S, jiff_sqlx::Timestamp)> = sqlx::query_as(
                "SELECT status, updated_at FROM funding_intents \
                 WHERE id = $1",
            )
            .bind(row.id)
            .fetch_optional(pool)
            .await?;
            let unchanged = fresh.is_some_and(|(status, updated_at)| {
                status == row.status
                    && updated_at.to_jiff() == row.updated_at.to_jiff()
            });
            if !unchanged {
                continue;
            }
            report.violations.push((
                community_id,
                InvariantViolation::IntentDrift {
                    intent_id: row.id,
                    payment_intent_id: row.payment_intent_id.clone(),
                    local_status: row.status,
                    stripe_status: retrieved.status.as_str().to_string(),
                    detail,
                },
            ));
        }
    }
    Ok(())
}

/// Claim this instance's right to run a reconciliation pass: the pass
/// advisory try-lock plus the due check (any community whose watermark
/// is older than the interval). Returns false when
/// another instance holds the lock or nothing is due; holding `tx` open
/// across the pass keeps the lock until `record_reconciliation_pass`
/// commits alongside it. `tx` must come from the worker pool (the
/// pinned connection sits idle for the pass's whole Stripe-heavy
/// duration), so the claim raises the pool's 60s leak-heal
/// `idle_in_transaction_session_timeout` to the pass's own scale —
/// `SET LOCAL`, so the pooled connection reverts at commit. A pass
/// somehow exceeding an hour still gets killed and the next tick
/// retries.
pub(crate) async fn try_claim_reconciliation_pass(
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<bool, StoreError> {
    let locked: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_xact_lock(hashtext('reconciliation_pass'))",
    )
    .fetch_one(&mut **tx)
    .await?;
    if !locked {
        return Ok(false);
    }
    sqlx::query("SET LOCAL idle_in_transaction_session_timeout = '1h'")
        .execute(&mut **tx)
        .await?;
    let cutoff = time_source
        .now()
        .checked_sub(SignedDuration::from_hours(RECONCILIATION_INTERVAL_HOURS))
        .map_err(anyhow::Error::from)?;
    Ok(sqlx::query_scalar(
        "SELECT EXISTS ( \
             SELECT 1 FROM communities WHERE last_reconciliation_at <= $1 \
         )",
    )
    .bind(cutoff.to_sqlx())
    .fetch_one(&mut **tx)
    .await?)
}

/// Restamp every community's watermark after a completed pass (the
/// run-all rule: one pass covers all communities, so all advance
/// together).
pub(crate) async fn record_reconciliation_pass(
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    sqlx::query("UPDATE communities SET last_reconciliation_at = $1")
        .bind(time_source.now().to_sqlx())
        .execute(&mut **tx)
        .await?;
    Ok(())
}
