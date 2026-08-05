//! Transaction-scoped coordination-lock tracking.
//!
//! [`TrackedTx`] owns a database transaction together with the ordered record
//! of the coordination locks that transaction holds, so the lock-ordering
//! declaration and its enforcement are one artifact. The authoritative locking
//! model — holder criteria per lock, per-path acquisition sequences, and the
//! pairwise deadlock-freedom arguments — is `api/CONCURRENCY.md`; this module
//! is its runtime enforcement point. Every acquisition of a tracked lock flows
//! through a method here (which runs or observes the SQL and records the
//! acquisition), and functions with lock preconditions assert them against the
//! record (`expect_*`).
//!
//! # The tracked levels
//!
//! Three coordination locks are tracked, in nesting order:
//!
//! 1. **`auction_user` pair advisory lock** — "at most one funding/bidding
//!    actor per (auction, user)". Acquired first or not at all.
//! 2. **auction-processing advisory lock** — serializes settlement,
//!    cancellation, and intent activation per auction. A pair holder may
//!    blocking-acquire it (`activate_intent_tx`); processing holders never take
//!    pair locks, so the nesting is one-directional.
//! 3. **account rows** (`FOR UPDATE`, canonical id order) — one acquisition per
//!    transaction locks the full set up front; later acquisitions must be
//!    re-locks of already-held rows (never a new wait). The acquisition's row
//!    snapshot is carried in the record and stays authoritative until a
//!    balance-updating entry invalidates it.
//!
//! Canonical lock order for account rows is the id's `Ord` — raw UUID
//! byte order (`AccountId` derives it), matching the
//! database's `ORDER BY id`. Every site that takes row locks on multiple rows
//! within one transaction — explicit `SELECT FOR UPDATE` or the implicit locks
//! of balance updates — must acquire in this order, or two transactions sharing
//! rows can AB/BA deadlock. Sites must use this one order rather than an ad-hoc
//! ordering: lexical order on `id.to_string()`, for example, happens to
//! coincide with byte order for lowercase hyphenated UUIDs, but that's a fact
//! to avoid depending on site by site. The discipline spans the transaction's
//! entire lifetime, not each statement or loop — which is why a second
//! acquisition with new rows is an ordering violation even if it is internally
//! sorted; a flow that discovers it needs another row must restructure to lock
//! the full set in one acquisition up front.
//!
//! There is deliberately no single strict global order (see CONCURRENCY.md):
//! safety is layered, and the rules above are the composition constraints the
//! layered arguments rely on. Acquisitions that violate them log at error level
//! and `debug_assert` — the operation proceeds in release builds (a recorded
//! ordering violation is a deadlock *risk*, not a certain fault in the current
//! transaction), but any test exercising the path fails loudly.
//!
//! # Savepoints
//!
//! Postgres releases locks acquired inside a subtransaction when that
//! subtransaction aborts (row locks and advisory xact locks alike), and
//! transfers them to the parent on release. [`TrackedTx::savepoint`] mirrors
//! this exactly: the savepoint scope shares the parent's record, its `commit`
//! keeps the entries it added (the locks survive), and its `rollback` truncates
//! them. A savepoint scope you intend to outlive must therefore end in an
//! explicit `commit` or `rollback`, not a drop — a bare drop rolls the
//! savepoint back at the SQL level but leaves its entries recorded. Advisory
//! locks belong outside savepoints entirely (asserted here): a lock that can be
//! unwound out from under the rest of the transaction can't back a contract the
//! rest of the transaction relies on.

use std::ops::{Deref, DerefMut};

use sqlx::Acquire;

use payloads::{
    Account, AccountId, AccountOwner, ApiError, AuctionId, CommunityId, UserId,
};

use super::StoreError;
use super::currency::DbAccount;

/// SQL expression computing the advisory lock key that coordinates auction
/// processing between the scheduler and lifecycle mutations. `id_expr` is a SQL
/// expression yielding the auction id. Public within the crate because the
/// scheduler's selection query is the one site that acquires the lock without
/// [`TrackedTx`] — its try-lock must be embedded in the selection SQL
/// (`lock_next_auction_needing_update`), and the claim is recorded afterwards
/// via [`TrackedTx::assume_processing_locked`].
pub(crate) fn auction_processing_lock_key(id_expr: &str) -> String {
    format!("hashtextextended('auction_processing:' || {id_expr}::text, 0)")
}

/// SQL expression computing the `auction_user` pair-lock key. Private:
/// [`TrackedTx::acquire_pair`] is the only acquisition path.
fn auction_user_lock_key(auction_expr: &str, user_expr: &str) -> String {
    format!(
        "hashtextextended('auction_user:' || {auction_expr}::text \
         || ':' || {user_expr}::text, 0)"
    )
}

/// How an advisory-lock acquisition waits: member-present flows block (the
/// member is waiting on the result); workers try and skip, relying on
/// state-driven re-selection to retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockWait {
    Block,
    Try,
}

/// One recorded coordination-lock acquisition. Entries are append-only within a
/// scope; savepoint rollback truncates the entries its scope added.
enum LockEntry {
    Pair {
        auction_id: AuctionId,
        user_id: UserId,
    },
    Processing {
        auction_id: AuctionId,
    },
    /// An account-row acquisition, carrying the rows as read under the lock —
    /// authoritative while `valid`, since no other transaction can change them.
    /// A balance-updating entry (`create_entry`) flips `valid` off; a later
    /// re-lock records a fresh entry. The latest entry is the live snapshot;
    /// earlier entries persist only as the record of which rows this
    /// transaction holds.
    Accounts {
        /// Sorted in canonical lock order (the acquisition's `ORDER BY id`).
        accounts: Vec<Account>,
        valid: bool,
    },
}

impl std::fmt::Debug for LockEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockEntry::Pair {
                auction_id,
                user_id,
            } => write!(f, "Pair({auction_id}, {user_id})"),
            LockEntry::Processing { auction_id } => {
                write!(f, "Processing({auction_id})")
            }
            LockEntry::Accounts { accounts, valid } => {
                write!(f, "Accounts(n={}, valid={valid})", accounts.len())
            }
        }
    }
}

/// The ordered acquisition record for one transaction. Owned by the root
/// [`TrackedTx`] and lent to savepoint scopes.
#[derive(Debug, Default)]
pub struct LockRecord {
    entries: Vec<LockEntry>,
}

impl LockRecord {
    fn union_of_held_account_ids(
        &self,
    ) -> std::collections::BTreeSet<AccountId> {
        self.entries
            .iter()
            .filter_map(|e| match e {
                LockEntry::Accounts { accounts, .. } => Some(accounts),
                _ => None,
            })
            .flatten()
            .map(|a| a.id)
            .collect()
    }

    fn latest_accounts_entry(&self) -> Option<(&Vec<Account>, bool)> {
        self.entries.iter().rev().find_map(|e| match e {
            LockEntry::Accounts { accounts, valid } => Some((accounts, *valid)),
            _ => None,
        })
    }
}

/// Where a [`TrackedTx`]'s record lives: owned by the root transaction,
/// borrowed from the parent by a savepoint scope.
enum RecordSlot<'r> {
    Owned(LockRecord),
    Shared(&'r mut LockRecord),
}

impl RecordSlot<'_> {
    fn get(&self) -> &LockRecord {
        match self {
            RecordSlot::Owned(r) => r,
            RecordSlot::Shared(r) => r,
        }
    }

    fn get_mut(&mut self) -> &mut LockRecord {
        match self {
            RecordSlot::Owned(r) => r,
            RecordSlot::Shared(r) => r,
        }
    }
}

/// A database transaction paired with the record of the coordination locks it
/// holds (see the module docs). Derefs to the underlying [`sqlx::Transaction`]
/// so plain query code is unaffected; [`TrackedTx::tx`] is the explicit
/// accessor for statement execution. Constructed at `BEGIN` by
/// [`TrackedTx::begin`] and threaded as `&mut TrackedTx` from there;
/// [`commit`](TrackedTx::commit)/[`rollback`](TrackedTx::rollback) consume it.
pub struct TrackedTx<'c, 'r> {
    tx: sqlx::Transaction<'c, sqlx::Postgres>,
    record: RecordSlot<'r>,
    /// Record length when this scope opened; rollback truncates to it
    /// (savepoint semantics — see the module docs).
    base: usize,
}

impl TrackedTx<'static, 'static> {
    /// Begin a transaction on `pool` with an empty lock record. Taking the
    /// pool rather than an already-begun transaction guarantees the record
    /// covers the transaction's whole lifetime — no untracked SQL (with its
    /// implicit row locks) can precede it.
    pub async fn begin(pool: &sqlx::PgPool) -> Result<Self, sqlx::Error> {
        Ok(TrackedTx {
            tx: pool.begin().await?,
            record: RecordSlot::Owned(LockRecord::default()),
            base: 0,
        })
    }
}

impl<'c, 'r> TrackedTx<'c, 'r> {
    /// Access the transaction for statement execution.
    pub fn tx(&mut self) -> &mut sqlx::Transaction<'c, sqlx::Postgres> {
        &mut self.tx
    }

    /// Open a savepoint scope sharing this transaction's lock record. The
    /// returned scope must end in an explicit [`commit`](TrackedTx::commit)
    /// (locks acquired inside transfer to the parent; their entries stay) or
    /// [`rollback`](TrackedTx::rollback) (Postgres releases them; their entries
    /// are truncated). A bare drop rolls back the savepoint but cannot truncate
    /// the record — acceptable only when the whole transaction is unwinding.
    pub async fn savepoint(
        &mut self,
    ) -> Result<TrackedTx<'_, '_>, sqlx::Error> {
        let record = self.record.get_mut();
        let base = record.entries.len();
        let tx = self.tx.begin().await?;
        Ok(TrackedTx {
            tx,
            record: RecordSlot::Shared(record),
            base,
        })
    }

    /// Commit the transaction (root) or release the savepoint (scope).
    /// Savepoint entries stay recorded: Postgres hands the scope's locks to the
    /// parent, which holds them to top-level commit.
    pub async fn commit(self) -> Result<(), sqlx::Error> {
        self.tx.commit().await
    }

    /// Roll back the transaction (root) or to the savepoint (scope), truncating
    /// the entries this scope added to match Postgres releasing the locks it
    /// acquired.
    pub async fn rollback(mut self) -> Result<(), sqlx::Error> {
        let base = self.base;
        self.record.get_mut().entries.truncate(base);
        self.tx.rollback().await
    }

    fn in_savepoint(&self) -> bool {
        matches!(self.record, RecordSlot::Shared(_))
    }

    /// Log an ordering violation and fail loudly in debug builds. Release
    /// builds proceed: a recorded violation is a deadlock risk to fix, not a
    /// certain fault in this transaction.
    fn violation(&self, what: &str) {
        tracing::error!(
            record = ?self.record.get().entries,
            what,
            "coordination-lock ordering violation"
        );
        debug_assert!(
            false,
            "coordination-lock ordering violation: {what}; record: {:?}",
            self.record.get().entries
        );
    }

    // --- Advisory locks (levels 1–2) -------------------------------------

    /// Acquire the `auction_user` pair advisory lock — "at most one
    /// funding/bidding actor per (auction, user)" — per `wait`. Returns
    /// whether the lock is held; false only for a failed try. The lock serves
    /// two duties: the Stripe-lineage mutex (order transactions, execute
    /// claims, intent-worker arms) and the proxy bidding claim's work-claim
    /// token; holder criteria for both are in `api/CONCURRENCY.md`. It is the
    /// first lock or no lock: acquiring it while anything else is held (or
    /// inside a savepoint) is an ordering violation.
    pub(crate) async fn acquire_pair(
        &mut self,
        auction_id: &AuctionId,
        user_id: &UserId,
        wait: LockWait,
    ) -> Result<bool, StoreError> {
        if self.in_savepoint() {
            self.violation("pair advisory lock inside a savepoint");
        }
        if !self.record.get().entries.is_empty() {
            self.violation("pair advisory lock acquired while holding locks");
        }
        let acquired = match wait {
            LockWait::Block => {
                sqlx::query(&format!(
                    "SELECT pg_advisory_xact_lock({})",
                    auction_user_lock_key("$1", "$2")
                ))
                .bind(auction_id)
                .bind(user_id)
                .execute(&mut *self.tx)
                .await?;
                true
            }
            LockWait::Try => {
                sqlx::query_scalar(&format!(
                    "SELECT pg_try_advisory_xact_lock({})",
                    auction_user_lock_key("$1", "$2")
                ))
                .bind(auction_id)
                .bind(user_id)
                .fetch_one(&mut *self.tx)
                .await?
            }
        };
        if acquired {
            self.record.get_mut().entries.push(LockEntry::Pair {
                auction_id: *auction_id,
                user_id: *user_id,
            });
        }
        Ok(acquired)
    }

    /// Blocking-acquire the auction-processing advisory lock. Legal while
    /// holding nothing or only the pair lock (the one-directional nesting);
    /// anything else — including a savepoint scope — is an ordering violation.
    pub(crate) async fn acquire_processing(
        &mut self,
        auction_id: &AuctionId,
    ) -> Result<(), StoreError> {
        self.check_processing_acquisition();
        sqlx::query(&format!(
            "SELECT pg_advisory_xact_lock({})",
            auction_processing_lock_key("$1")
        ))
        .bind(auction_id)
        .execute(&mut *self.tx)
        .await?;
        self.record.get_mut().entries.push(LockEntry::Processing {
            auction_id: *auction_id,
        });
        Ok(())
    }

    /// Record an auction-processing lock acquired outside this module — the
    /// scheduler's selection query, whose try-lock is embedded in the selection
    /// SQL. Record-only; the caller vouches the lock is held.
    pub(crate) fn assume_processing_locked(&mut self, auction_id: &AuctionId) {
        self.check_processing_acquisition();
        self.record.get_mut().entries.push(LockEntry::Processing {
            auction_id: *auction_id,
        });
    }

    fn check_processing_acquisition(&self) {
        if self.in_savepoint() {
            self.violation("processing advisory lock inside a savepoint");
        }
        let only_pair = self
            .record
            .get()
            .entries
            .iter()
            .all(|e| matches!(e, LockEntry::Pair { .. }));
        if !only_pair {
            self.violation(
                "processing advisory lock acquired after row locks or a \
                 second processing lock",
            );
        }
    }

    /// Assert the auction-processing lock for `auction_id` is recorded.
    pub(crate) fn expect_processing(
        &self,
        auction_id: &AuctionId,
    ) -> Result<(), StoreError> {
        let held = self.record.get().entries.iter().any(|e| {
            matches!(e, LockEntry::Processing { auction_id: a }
                if a == auction_id)
        });
        if held {
            Ok(())
        } else {
            Err(StoreError::CoordLockNotHeld("auction-processing"))
        }
    }

    /// Assert the `auction_user` pair lock for (`auction_id`, `user_id`) is
    /// recorded.
    pub(crate) fn expect_pair(
        &self,
        auction_id: &AuctionId,
        user_id: &UserId,
    ) -> Result<(), StoreError> {
        let held = self.record.get().entries.iter().any(|e| {
            matches!(e, LockEntry::Pair { auction_id: a, user_id: u }
                if a == auction_id && u == user_id)
        });
        if held {
            Ok(())
        } else {
            Err(StoreError::CoordLockNotHeld("auction_user pair"))
        }
    }

    // --- Account rows (level 3) ------------------------------------------

    /// Lock account rows in canonical order (`ORDER BY id` before the locking
    /// node acquires in exactly that order). Callers that need account state
    /// before `create_entry` — e.g. a balance read that determines the journal
    /// lines — must lock the entry's full account set here first; locking only
    /// a subset and acquiring the rest later re-creates the out-of-order
    /// interleaving the canonical order exists to prevent (checked: a later
    /// acquisition must be a re-lock of already-held rows).
    pub(crate) async fn lock_accounts(
        &mut self,
        account_ids: &[AccountId],
    ) -> Result<(), StoreError> {
        let mut ids: Vec<AccountId> = account_ids.to_vec();
        ids.sort();
        ids.dedup();
        let db_accounts: Vec<DbAccount> = sqlx::query_as(
            "SELECT * FROM accounts WHERE id = ANY($1) ORDER BY id FOR UPDATE",
        )
        .bind(&ids)
        .fetch_all(&mut *self.tx)
        .await?;
        if db_accounts.len() != ids.len() {
            return Err(ApiError::AccountNotFound.into());
        }
        self.record_account_acquisition(db_accounts)
    }

    /// Lock a single account row by owner. Single-row acquisition is trivially
    /// in canonical order; the resulting snapshot is `locked_accounts()[0]`.
    pub(crate) async fn lock_account(
        &mut self,
        community_id: &CommunityId,
        owner: AccountOwner,
    ) -> Result<(), StoreError> {
        let db_account: DbAccount = sqlx::query_as(
            r#"
            SELECT * FROM accounts
            WHERE community_id = $1
              AND owner_type = $2
              AND owner_id IS NOT DISTINCT FROM $3
            FOR UPDATE
            "#,
        )
        .bind(community_id)
        .bind(owner.owner_type())
        .bind(owner.owner_id())
        .fetch_optional(&mut *self.tx)
        .await?
        .ok_or(ApiError::AccountNotFound)?;
        self.record_account_acquisition(vec![db_account])
    }

    /// Lock every member and treasury account in a community, in canonical
    /// order. For operations whose account set is defined by a predicate rather
    /// than known ids (balance reset, re-quantization).
    pub(crate) async fn lock_community_accounts(
        &mut self,
        community_id: &CommunityId,
    ) -> Result<(), StoreError> {
        let db_accounts: Vec<DbAccount> = sqlx::query_as(
            r#"
            SELECT * FROM accounts
            WHERE community_id = $1
              AND owner_type IN ('member_main', 'community_treasury')
            ORDER BY id
            FOR UPDATE
            "#,
        )
        .bind(community_id)
        .fetch_all(&mut *self.tx)
        .await?;
        self.record_account_acquisition(db_accounts)
    }

    /// Record an account-row acquisition, checking it against the
    /// single-acquisition rule: new rows are legal only in the transaction's
    /// first account acquisition; afterwards, only re-locks of already-held
    /// rows.
    fn record_account_acquisition(
        &mut self,
        db_accounts: Vec<DbAccount>,
    ) -> Result<(), StoreError> {
        let accounts: Vec<Account> = db_accounts
            .into_iter()
            .map(Account::try_from)
            .collect::<Result<_, _>>()?;
        let record = self.record.get();
        let held = record.union_of_held_account_ids();
        let all_held = accounts.iter().all(|a| held.contains(&a.id));
        if !all_held && !held.is_empty() {
            self.violation(
                "account lock set extended after the first acquisition",
            );
        }
        self.record.get_mut().entries.push(LockEntry::Accounts {
            accounts,
            valid: true,
        });
        Ok(())
    }

    /// The latest account-row snapshot, in canonical lock order — authoritative
    /// while it lives, since no other transaction can change the rows. Errors
    /// if no acquisition exists or a balance-updating entry has consumed the
    /// snapshot (re-lock to refresh).
    pub(crate) fn locked_accounts(&self) -> Result<&[Account], StoreError> {
        match self.record.get().latest_accounts_entry() {
            None => Err(StoreError::AccountNotLocked),
            Some((_, false)) => Err(StoreError::AccountSnapshotInvalid),
            Some((accounts, true)) => Ok(accounts),
        }
    }

    /// The locked account with the given id, erring if the snapshot doesn't
    /// cover it. The error is an internal invariant violation (a 500, not a
    /// user error): it means a caller locked a set that doesn't cover the
    /// accounts it went on to touch.
    pub(crate) fn locked_account(
        &self,
        id: &AccountId,
    ) -> Result<&Account, StoreError> {
        let accounts = self.locked_accounts()?;
        accounts
            .binary_search_by_key(id, |a| a.id)
            .ok()
            .map(|i| &accounts[i])
            .ok_or(StoreError::AccountNotLocked)
    }

    /// The locked account owned by a community member, erring if the snapshot
    /// doesn't cover it.
    pub(crate) fn locked_member_account(
        &self,
        community_id: &CommunityId,
        user_id: &UserId,
    ) -> Result<&Account, StoreError> {
        self.locked_accounts()?
            .iter()
            .find(|a| {
                a.community_id == *community_id
                    && a.owner == AccountOwner::Member(*user_id)
            })
            .ok_or(StoreError::AccountNotLocked)
    }

    /// Re-lock every account row this transaction holds and record a fresh
    /// snapshot — the recovery move after `create_entry` consumes the
    /// snapshot. A re-lock of held rows never waits, so this is one
    /// non-blocking statement. Errors if no account acquisition exists.
    pub(crate) async fn refresh_accounts(&mut self) -> Result<(), StoreError> {
        let ids: Vec<AccountId> = self
            .record
            .get()
            .union_of_held_account_ids()
            .into_iter()
            .collect();
        if ids.is_empty() {
            return Err(StoreError::AccountNotLocked);
        }
        self.lock_accounts(&ids).await
    }

    /// Mark the current account snapshot consumed — its balances no longer
    /// match the rows. Called by `create_entry` after its balance updates; a
    /// caller that needs account state afterwards re-locks
    /// ([`refresh_accounts`](TrackedTx::refresh_accounts); a re-lock of held
    /// rows never waits) or re-reads with a plain `SELECT`.
    pub(crate) fn invalidate_account_snapshot(&mut self) {
        if let Some(LockEntry::Accounts { valid, .. }) = self
            .record
            .get_mut()
            .entries
            .iter_mut()
            .rev()
            .find(|e| matches!(e, LockEntry::Accounts { .. }))
        {
            *valid = false;
        }
    }
}

impl<'c> Deref for TrackedTx<'c, '_> {
    type Target = sqlx::Transaction<'c, sqlx::Postgres>;

    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

impl DerefMut for TrackedTx<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tx
    }
}
