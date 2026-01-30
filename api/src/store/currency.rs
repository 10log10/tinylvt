//! Currency and ledger operations
//!
//! Implements double-entry accounting with:
//! - Account management (member_main and treasury accounts)
//! - Journal entry creation with balance updates
//! - Credit limit enforcement
//! - Idempotency support
//!
//! # Decimal calculation notes
//!
//! Care must be taken to avoid imprecision in decimal calculations. Ordering
//! can determine whether values sum to zero or not:
//!
//! ```
//! use rust_decimal::Decimal;
//!
//! let total = Decimal::from(8);
//! let n = Decimal::from(3);
//! let base = total / n;
//!
//! let via_mult = base * n;
//! let via_add = base + base + base;
//!
//! println!(
//!     "mult: {}, add: {}, equal: {}",
//!     via_mult, via_add, via_mult == via_add
//! );
//!
//! let add_sums_to_zero1 =
//!     Decimal::ZERO == [-total, base, base, base].iter().sum();
//! let add_sums_to_zero2 =
//!     Decimal::ZERO == [base, base, base, -total].iter().sum();
//! println!(
//!     "add sums to zero 1: {}, add sums to zero 2: {}",
//!     add_sums_to_zero1,
//!     add_sums_to_zero2
//! );
//! ```
//!
//! ```text
//! running 1 test
//!
//! mult: 8.000000000000000000000000000,
//! add: 8.000000000000000000000000000,
//! equal: true
//!
//! add sums to zero 1: false, add sums to zero 2: true
//! test tmp::test_tmp ... ok
//! ```

use jiff::Timestamp;
use jiff_sqlx::{Timestamp as SqlxTs, ToSqlx};
use payloads::{
    Account, AccountId, AccountOwner, AccountOwnerType, CommunityId,
    CurrencyMode, EntryType, IdempotencyKey, JournalEntry, JournalEntryId,
    UserId,
};
use rust_decimal::Decimal;
use sqlx::{FromRow, PgPool};
use std::collections::HashMap;

use super::StoreError;
use crate::time::TimeSource;

/// Database-level Account struct that matches the accounts table schema
#[derive(Debug, Clone, FromRow)]
struct DbAccount {
    id: AccountId,
    community_id: CommunityId,
    owner_type: AccountOwnerType,
    owner_id: Option<UserId>,
    #[sqlx(try_from = "SqlxTs")]
    created_at: Timestamp,
    balance_cached: Decimal,
    credit_limit: Option<Decimal>,
}

impl TryFrom<DbAccount> for Account {
    type Error = StoreError;

    fn try_from(db: DbAccount) -> Result<Self, Self::Error> {
        let owner = AccountOwner::from_parts(db.owner_type, db.owner_id)
            .ok_or(StoreError::InvalidAccountOwnership)?;

        Ok(Account {
            id: db.id,
            community_id: db.community_id,
            owner,
            created_at: db.created_at,
            balance_cached: db.balance_cached,
            credit_limit: db.credit_limit,
        })
    }
}

/// Create an account for a member or treasury (transaction version)
pub async fn create_account_tx(
    community_id: &CommunityId,
    owner: AccountOwner,
    credit_limit: Option<Decimal>,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Account, StoreError> {
    let now = time_source.now();

    let db_account = sqlx::query_as::<_, DbAccount>(
        r#"
        INSERT INTO accounts (
            community_id,
            owner_type,
            owner_id,
            created_at,
            balance_cached,
            credit_limit
        )
        VALUES ($1, $2, $3, $4, 0, $5)
        RETURNING *
        "#,
    )
    .bind(community_id)
    .bind(owner.owner_type())
    .bind(owner.owner_id())
    .bind(now.to_sqlx())
    .bind(credit_limit)
    .fetch_one(&mut **tx)
    .await?;

    db_account.try_into()
}

/// Get account by owner
async fn get_account(
    community_id: &CommunityId,
    owner: AccountOwner,
    pool: &PgPool,
) -> Result<Account, StoreError> {
    let mut tx = pool.begin().await?;
    return get_account_tx(community_id, owner, &mut tx).await;
}

/// Get account by owner and lock for update
///
/// Locks the account row using SELECT FOR UPDATE, preventing concurrent
/// modifications until the transaction commits. Must be called inside a
/// transaction.
pub(crate) async fn get_account_for_update_tx(
    community_id: &CommunityId,
    owner: AccountOwner,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Account, StoreError> {
    let db_account = sqlx::query_as::<_, DbAccount>(
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
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(StoreError::AccountNotFound)?;

    db_account.try_into()
}

/// Get effective credit limit for an account, excluding any locked balance
/// pledged via auction bids.
///
/// Returns account-specific limit if set, otherwise community default
pub(crate) async fn get_effective_credit_limit_tx(
    account_id: &AccountId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Option<Decimal>, StoreError> {
    let row: (Option<Decimal>, Option<Decimal>) = sqlx::query_as(
        r#"
        SELECT a.credit_limit, c.default_credit_limit
        FROM accounts a
        JOIN communities c ON a.community_id = c.id
        WHERE a.id = $1
        "#,
    )
    .bind(account_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(StoreError::AccountNotFound)?;

    Ok(row.0.or(row.1))
}

/// Get locked balance for an account (Rust-based implementation).
///
/// The locked balance reduces the user's available credit.
///
/// Works within an active transaction and will see uncommitted changes made
/// by the same transaction (e.g., bids inserted but not yet committed).
///
/// Locked balance includes:
/// - Winning bids: value from latest processed round_space_results
/// - Outstanding bids: (prev round value + bid increment) for unprocessed
///   rounds
async fn get_locked_balance_tx(
    account_id: &AccountId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Decimal, StoreError> {
    // Step 1: Get the account to find its community
    let community_id: payloads::CommunityId =
        sqlx::query_scalar("SELECT community_id FROM accounts WHERE id = $1")
            .bind(account_id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(StoreError::AccountNotFound)?;

    // Step 2: Get all active auctions in this community
    #[derive(sqlx::FromRow)]
    struct ActiveAuction {
        auction_id: payloads::AuctionId,
        auction_params_id: uuid::Uuid,
    }

    let active_auctions: Vec<ActiveAuction> = sqlx::query_as(
        r#"
        SELECT auc.id as auction_id, auc.auction_params_id
        FROM auctions auc
        JOIN sites s ON auc.site_id = s.id
        WHERE s.community_id = $1 AND auc.end_at IS NULL
        "#,
    )
    .bind(community_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut total_locked = Decimal::ZERO;

    // Step 3: For each auction, calculate locked balance
    for auction in active_auctions {
        // Get the latest processed round number (highest round with results)
        let max_processed_round: Option<i32> = sqlx::query_scalar(
            r#"
            SELECT MAX(ar.round_num)
            FROM round_space_results rsr
            JOIN auction_rounds ar ON rsr.round_id = ar.id
            WHERE ar.auction_id = $1
            "#,
        )
        .bind(auction.auction_id)
        .fetch_optional(&mut **tx)
        .await?
        .flatten();

        // Get bid increment for calculating bid amounts
        let bid_increment: Decimal = sqlx::query_scalar(
            "SELECT bid_increment FROM auction_params WHERE id = $1",
        )
        .bind(auction.auction_params_id)
        .fetch_one(&mut **tx)
        .await?;

        // Step 3a: Add locked balance from winning bids in latest processed
        // round
        if let Some(processed_round_num) = max_processed_round {
            let winning_values: Vec<Decimal> = sqlx::query_scalar(
                r#"
                SELECT rsr.value
                FROM round_space_results rsr
                JOIN auction_rounds ar ON rsr.round_id = ar.id
                WHERE ar.auction_id = $1
                  AND ar.round_num = $2
                  AND rsr.winning_user_id = $3
                "#,
            )
            .bind(auction.auction_id)
            .bind(processed_round_num)
            .bind(account_id)
            .fetch_all(&mut **tx)
            .await?;

            for value in winning_values {
                total_locked += value;
            }
        }

        // Step 3b: Add locked balance from bids in unprocessed rounds
        #[derive(sqlx::FromRow)]
        struct UnprocessedBid {
            space_id: payloads::SpaceId,
            round_num: i32,
        }

        let unprocessed_bids: Vec<UnprocessedBid> = sqlx::query_as(
            r#"
            SELECT b.space_id, ar.round_num
            FROM bids b
            JOIN auction_rounds ar ON b.round_id = ar.id
            WHERE ar.auction_id = $1
              AND ar.round_num > $2
              AND b.user_id = $3
            "#,
        )
        .bind(auction.auction_id)
        .bind(max_processed_round.unwrap_or(-1))
        .bind(account_id)
        .fetch_all(&mut **tx)
        .await?;

        for bid in unprocessed_bids {
            // Get the previous round's value for this space (if any)
            let prev_round_value: Option<Decimal> = if bid.round_num > 0 {
                sqlx::query_scalar(
                    r#"
                    SELECT rsr.value
                    FROM round_space_results rsr
                    JOIN auction_rounds ar ON rsr.round_id = ar.id
                    WHERE ar.auction_id = $1
                      AND ar.round_num = $2
                      AND rsr.space_id = $3
                    "#,
                )
                .bind(auction.auction_id)
                .bind(bid.round_num - 1)
                .bind(bid.space_id)
                .fetch_optional(&mut **tx)
                .await?
                .flatten()
            } else {
                None
            };

            // Locked amount = (prev value + bid increment) OR zero
            let locked_for_bid = prev_round_value
                .map(|v| v + bid_increment)
                .unwrap_or(Decimal::ZERO);
            total_locked += locked_for_bid;
        }
    }

    Ok(total_locked)
}

/// Get available credit for an account
///
/// Returns the amount the account can still spend, accounting for:
/// - Current balance (positive = credit, negative = debt)
/// - Locked balance from outstanding auction bids
/// - Credit limit (the maximum negative balance allowed)
///
/// Formula: available = balance - locked_balance + credit_limit
///
/// Examples:
/// - balance=100, locked=20, limit=50 -> available=130
/// - balance=0, locked=0, limit=50 -> available=50
/// - balance=-30, locked=0, limit=50 -> available=20
/// - balance=100, locked=50, limit=None -> available=None (unlimited)
///
/// Returns None if there's no credit limit (unlimited credit)
async fn get_available_credit_tx(
    account_id: &AccountId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Option<Decimal>, StoreError> {
    // Get account info including balance and owner_type
    let (balance, owner_type, credit_limit, default_limit): (
        Decimal,
        AccountOwnerType,
        Option<Decimal>,
        Option<Decimal>,
    ) = sqlx::query_as(
        r#"
        SELECT a.balance_cached, a.owner_type, a.credit_limit,
               c.default_credit_limit
        FROM accounts a
        JOIN communities c ON a.community_id = c.id
        WHERE a.id = $1
        "#,
    )
    .bind(account_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(StoreError::AccountNotFound)?;

    // Treasury accounts have unlimited credit
    if owner_type == AccountOwnerType::CommunityTreasury {
        return Ok(None);
    }

    // Get locked balance
    let locked = get_locked_balance_tx(account_id, tx).await?;

    // Calculate effective credit limit
    let effective_limit = credit_limit.or(default_limit);

    // If no limit, return None (unlimited)
    let Some(limit) = effective_limit else {
        return Ok(None);
    };

    // Calculate available credit
    // available = balance - locked + limit
    // This is equivalent to: limit - (locked - balance)
    let available = balance - locked + limit;

    Ok(Some(available))
}

/// Check if an account has sufficient credit for a transaction
///
/// Returns Ok(()) if the account can spend the given amount, or
/// Err(StoreError::InsufficientBalance) if not.
pub(crate) async fn check_sufficient_credit_tx(
    account_id: &AccountId,
    amount: Decimal,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    let available = get_available_credit_tx(account_id, tx).await?;

    // If available is None, unlimited credit
    let Some(available_amount) = available else {
        return Ok(());
    };

    if available_amount < amount {
        return Err(StoreError::InsufficientBalance);
    }

    Ok(())
}

/// Create a journal entry with lines, updating balances atomically
///
/// This is the core ledger operation. It:
/// 1. Validates that lines sum to zero
/// 2. Validates one line per account (unless is_auction_settlement)
/// 3. Locks debited accounts and checks credit limits (unless
///    is_auction_settlement)
/// 4. Creates the journal entry and lines
/// 5. Updates balance_cached for all accounts
///
/// The `is_auction_settlement` parameter should be true only for auction
/// settlements. It enables:
/// - Multiple lines per account (for distributed_clearing transparency)
/// - Skipping credit checks (locked balance includes the debits being settled)
///
/// Must be called within a transaction. The caller is responsible for
/// committing or rolling back the transaction.
///
/// Uses idempotency_key for deduplication - if key exists, returns Ok
/// without error.
#[allow(clippy::too_many_arguments)]
async fn create_entry(
    community_id: &CommunityId,
    entry_type: EntryType,
    idempotency_key: IdempotencyKey,
    lines: Vec<(AccountId, Decimal)>,
    auction_id: Option<&payloads::AuctionId>,
    initiated_by_id: Option<&UserId>,
    note: Option<String>,
    is_auction_settlement: bool,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    // Check idempotency
    let existing: Option<JournalEntryId> = sqlx::query_scalar(
        "SELECT id FROM journal_entries WHERE idempotency_key = $1",
    )
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await?;

    if existing.is_some() {
        return Ok(()); // Idempotent - already processed
    }

    // Validate lines sum to zero
    let sum: Decimal = lines.iter().map(|(_, amount)| amount).sum();
    if sum != Decimal::ZERO {
        return Err(StoreError::JournalLinesDoNotSumToZero(sum));
    }

    let now = time_source.now();

    // For non-settlement entries, validate and check credit limits
    if !is_auction_settlement {
        // Validate one line per account
        let unique_accounts: std::collections::HashSet<AccountId> =
            lines.iter().map(|(account_id, _)| *account_id).collect();
        if unique_accounts.len() != lines.len() {
            return Err(StoreError::DuplicateAccountInJournalEntry);
        }

        // Collect debited accounts and sort by ID to prevent deadlocks
        let mut debited_accounts: Vec<_> = lines
            .iter()
            .filter(|(_, amount)| *amount < Decimal::ZERO)
            .map(|(account_id, _)| *account_id)
            .collect();
        debited_accounts.sort_by_key(|id| id.to_string());
        debited_accounts.dedup(); // Should be no-op given one line per account

        // Lock debited accounts using SELECT FOR UPDATE
        // Ensures there's no changes between when the available credit is
        // checked and when the debit is committed.
        for account_id in &debited_accounts {
            sqlx::query("SELECT 1 FROM accounts WHERE id = $1 FOR UPDATE")
                .bind(account_id)
                .execute(&mut **tx)
                .await?;
        }

        // Check credit limits BEFORE making changes
        for (account_id, amount) in &lines {
            if *amount >= Decimal::ZERO {
                continue; // Skip credits, only check debits
            }

            check_sufficient_credit_tx(account_id, amount.abs(), tx).await?;
        }
    }

    // Create journal entry
    let entry_id: JournalEntryId = sqlx::query_scalar(
        r#"
        INSERT INTO journal_entries (
            community_id,
            entry_type,
            idempotency_key,
            auction_id,
            initiated_by_id,
            note,
            created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id
        "#,
    )
    .bind(community_id)
    .bind(entry_type)
    .bind(idempotency_key)
    .bind(auction_id)
    .bind(initiated_by_id)
    .bind(&note)
    .bind(now.to_sqlx())
    .fetch_one(&mut **tx)
    .await?;

    // Create journal lines and update balances
    for (account_id, amount) in &lines {
        // Insert journal line
        sqlx::query(
            r#"
            INSERT INTO journal_lines (entry_id, account_id, amount)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(entry_id)
        .bind(account_id)
        .bind(amount)
        .execute(&mut **tx)
        .await?;

        // Update balance_cached
        // We don't need to lock credited accounts since the update is atomic.
        sqlx::query(
            r#"
            UPDATE accounts
            SET balance_cached = balance_cached + $1
            WHERE id = $2
            "#,
        )
        .bind(amount)
        .bind(account_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

/// Create auction settlement journal entry based on currency mode
///
/// The settlement behavior depends on the community's currency_mode:
/// - points_allocation: Winners pay treasury
/// - distributed_clearing: Winners pay, split equally among active members
/// - deferred_payment: Winners pay treasury
/// - prepaid_credits: Winners pay treasury
///
/// For distributed_clearing, distributions go only to members currently marked
/// as active (allows observer members, decouples bidding rights from
/// distribution rights).
///
/// Multiple lines per account are only used to separate debits and credits in
/// distributed_clearing mode (a winner who is also an active member will have
/// both a debit line for what they owe and a credit line for their share).
///
/// The caller must aggregate payments by user. Individual space wins are
/// tracked in round_space_results, not in the journal.
///
/// This function must call create_entry with skip_credit_check=true, since the
/// locked balance includes the very bids being settled. For this reason, it is
/// not nessecary to lock the account.
///
/// Generates a random idempotency key. Exactly-once settlement is guaranteed
/// by the scheduler's advisory lock, not the idempotency key. Using a random
/// key prevents malicious actors from blocking settlement by preemptively
/// creating a journal entry with a predictable idempotency key.
pub async fn create_auction_settlement_entry(
    community_id: &CommunityId,
    auction_id: &payloads::AuctionId,
    winner_payments: HashMap<UserId, Decimal>,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    // Generate random idempotency key to prevent predictable key attacks
    let idempotency_key = IdempotencyKey(uuid::Uuid::new_v4());

    // Get community currency mode
    let currency_mode: CurrencyMode = sqlx::query_scalar(
        "SELECT currency_mode FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_one(&mut **tx)
    .await?;

    // Filter out zero payments and calculate total
    let winner_payments: HashMap<UserId, Decimal> = winner_payments
        .into_iter()
        .filter(|(_, amount)| *amount > Decimal::ZERO)
        .collect();

    let total_paid: Decimal = winner_payments.values().sum();

    if total_paid == Decimal::ZERO {
        // No payments to process, return early
        return Ok(());
    }

    // Build journal lines based on currency mode
    let mut lines: Vec<(AccountId, Decimal)> = Vec::new();

    // Add debit lines for winners (negative amounts)
    // One debit line per user for their total payment
    for (winner_user_id, amount) in &winner_payments {
        let winner_account = get_account_tx(
            community_id,
            AccountOwner::Member(*winner_user_id),
            tx,
        )
        .await?;
        lines.push((winner_account.id, -amount));
    }

    match currency_mode {
        CurrencyMode::PointsAllocation
        | CurrencyMode::DeferredPayment
        | CurrencyMode::PrepaidCredits => {
            // All payments go to treasury
            let treasury_account =
                get_account_tx(community_id, AccountOwner::Treasury, tx)
                    .await?;
            lines.push((treasury_account.id, total_paid));
        }
        CurrencyMode::DistributedClearing => {
            // Distribute equally among active members
            let active_member_ids: Vec<UserId> = sqlx::query_scalar(
                r#"
                SELECT user_id
                FROM community_members
                WHERE community_id = $1 AND is_active = true
                "#,
            )
            .bind(community_id)
            .fetch_all(&mut **tx)
            .await?;

            if active_member_ids.is_empty() {
                // Fallback: if no active members, send to treasury
                // This can occur if membership schedules expire or all members
                // are manually set to inactive
                let treasury_account =
                    get_account_tx(community_id, AccountOwner::Treasury, tx)
                        .await?;
                lines.push((treasury_account.id, total_paid));
            } else {
                // Calculate per-member distribution
                let num_active = Decimal::from(active_member_ids.len());
                let base_amount = total_paid / num_active;

                // Add credit lines for each active member
                // Winners who are also active members will have both debit
                // (above) and credit (here) lines, making the journal
                // transparent
                //
                // To handle rounding: Give all members except the last one
                // base_amount, then give the last member exactly what's
                // left. This guarantees the sum equals total_paid with no
                // floating-point precision errors.
                let mut distributed_so_far = Decimal::ZERO;

                for (idx, member_user_id) in
                    active_member_ids.iter().enumerate()
                {
                    let member_account = get_account_tx(
                        community_id,
                        AccountOwner::Member(*member_user_id),
                        tx,
                    )
                    .await?;

                    let amount = if idx == active_member_ids.len() - 1 {
                        // Last member gets exactly what's left
                        total_paid - distributed_so_far
                    } else {
                        distributed_so_far += base_amount;
                        base_amount
                    };

                    lines.push((member_account.id, amount));
                }
            }
        }
    }

    // Create the journal entry as an auction settlement
    // This allows multiple lines per account and skips credit checks
    create_entry(
        community_id,
        EntryType::AuctionSettlement,
        idempotency_key,
        lines,
        Some(auction_id),
        None, // No initiated_by_id for automated settlements
        None, // No note
        true, // is_auction_settlement
        time_source,
        tx,
    )
    .await?;

    Ok(())
}

/// Get account by owner within a transaction
async fn get_account_tx(
    community_id: &CommunityId,
    owner: AccountOwner,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Account, StoreError> {
    let db_account = sqlx::query_as::<_, DbAccount>(
        r#"
        SELECT * FROM accounts
        WHERE community_id = $1
          AND owner_type = $2
          AND owner_id IS NOT DISTINCT FROM $3
        "#,
    )
    .bind(community_id)
    .bind(owner.owner_type())
    .bind(owner.owner_id())
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(StoreError::AccountNotFound)?;

    db_account.try_into()
}

/// Get currency information for a member
///
/// Returns account balance, effective credit limit (uses community default if
/// not set), locked balance (from auction bids), and available credit for the
/// member in their community.
///
/// Note: `target_member` represents the member whose info is being fetched,
/// not necessarily the user making the request.
async fn get_member_currency_info(
    target_member: &super::ValidatedMember,
    pool: &PgPool,
) -> Result<payloads::responses::MemberCurrencyInfo, StoreError> {
    let mut tx = pool.begin().await?;

    // Get the member's account
    let account = get_account_tx(
        &target_member.0.community_id,
        AccountOwner::Member(target_member.0.user_id),
        &mut tx,
    )
    .await?;

    // Get effective credit limit (account-specific or community default)
    let effective_credit_limit =
        get_effective_credit_limit_tx(&account.id, &mut tx).await?;

    // Get locked balance
    let locked_balance = get_locked_balance_tx(&account.id, &mut tx).await?;

    // Get available credit
    let available_credit =
        get_available_credit_tx(&account.id, &mut tx).await?;

    tx.commit().await?;

    Ok(payloads::responses::MemberCurrencyInfo {
        account_id: account.id,
        balance: account.balance_cached,
        credit_limit: effective_credit_limit,
        locked_balance,
        available_credit,
    })
}

/// Helper function to fetch and format transactions for an account
async fn fetch_account_transactions(
    account_id: &AccountId,
    community_id: &CommunityId,
    limit: i64,
    offset: i64,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::MemberTransaction>, StoreError> {
    // Fetch entries that involve this account
    let entries = sqlx::query_as::<_, JournalEntry>(
        r#"
        SELECT DISTINCT je.*
        FROM journal_entries je
        JOIN journal_lines jl ON je.id = jl.entry_id
        WHERE jl.account_id = $1
        ORDER BY je.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(account_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    // Collect all user IDs from all entries to batch fetch user identities
    let mut all_user_ids = std::collections::HashSet::new();
    for entry in &entries {
        #[derive(sqlx::FromRow)]
        struct LineAccountInfo {
            owner_type: AccountOwnerType,
            owner_id: Option<UserId>,
        }

        let lines: Vec<LineAccountInfo> = sqlx::query_as(
            r#"
            SELECT a.owner_type, a.owner_id
            FROM journal_lines jl
            JOIN accounts a ON jl.account_id = a.id
            WHERE jl.entry_id = $1
            "#,
        )
        .bind(entry.id)
        .fetch_all(pool)
        .await?;

        for line in lines {
            if line.owner_type == AccountOwnerType::MemberMain
                && let Some(user_id) = line.owner_id
            {
                all_user_ids.insert(user_id);
            }
        }
    }

    // Batch fetch all user identities at once
    let user_ids: Vec<UserId> = all_user_ids.into_iter().collect();
    let user_identities =
        super::get_user_identities(&user_ids, community_id, pool).await?;

    // For each entry, fetch lines and convert to user-friendly format
    let mut transactions = Vec::new();
    for entry in entries {
        #[derive(sqlx::FromRow)]
        struct LineWithAccount {
            amount: Decimal,
            owner_type: AccountOwnerType,
            owner_id: Option<UserId>,
        }

        let lines: Vec<LineWithAccount> = sqlx::query_as(
            r#"
            SELECT jl.amount, a.owner_type, a.owner_id
            FROM journal_lines jl
            JOIN accounts a ON jl.account_id = a.id
            WHERE jl.entry_id = $1
            "#,
        )
        .bind(entry.id)
        .fetch_all(pool)
        .await?;

        // Convert lines to TransactionLine with user-friendly info
        let mut transaction_lines = Vec::new();
        for line in lines {
            let party = match line.owner_type {
                AccountOwnerType::CommunityTreasury => {
                    payloads::responses::TransactionParty::Treasury
                }
                AccountOwnerType::MemberMain => {
                    let user_id = line
                        .owner_id
                        .ok_or(StoreError::InvalidAccountOwnership)?;

                    let user_identity = user_identities
                        .get(&user_id)
                        .cloned()
                        .ok_or(StoreError::UserNotFound)?;

                    payloads::responses::TransactionParty::Member(user_identity)
                }
            };

            transaction_lines.push(payloads::responses::TransactionLine {
                party,
                amount: line.amount,
            });
        }

        transactions.push(payloads::responses::MemberTransaction {
            entry_type: entry.entry_type,
            auction_id: entry.auction_id,
            note: entry.note,
            created_at: entry.created_at,
            lines: transaction_lines,
        });
    }

    Ok(transactions)
}

/// Get journal entries for a member's account
///
/// Returns transaction history for the member's account in their community.
///
/// The response includes user-friendly information (usernames instead of
/// account IDs) and filters lines to show only those relevant to the member.
///
/// Note: `target_member` represents the member whose transactions are being
/// fetched, not necessarily the user making the request.
async fn get_member_transactions(
    target_member: &super::ValidatedMember,
    limit: i64,
    offset: i64,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::MemberTransaction>, StoreError> {
    // Get the member's account
    let account = get_account(
        &target_member.0.community_id,
        AccountOwner::Member(target_member.0.user_id),
        pool,
    )
    .await?;

    fetch_account_transactions(
        &account.id,
        &target_member.0.community_id,
        limit,
        offset,
        pool,
    )
    .await
}

/// Convert database columns to CurrencyConfig
/// Returns None if the configuration is invalid
pub fn currency_config_from_db(
    mode: CurrencyMode,
    default_credit_limit: Option<Decimal>,
    debts_callable: bool,
    allowance_amount: Option<Decimal>,
    allowance_period: Option<jiff::Span>,
    allowance_start: Option<jiff::Timestamp>,
) -> Option<payloads::CurrencyConfig> {
    use payloads::{
        CurrencyConfig, IOUConfig, PointsAllocationConfig, PrepaidCreditsConfig,
    };

    match mode {
        CurrencyMode::PointsAllocation => {
            let amount = allowance_amount?;
            let period = allowance_period?;
            let start = allowance_start?;
            // Validate constraints
            if default_credit_limit != Some(Decimal::ZERO) || debts_callable {
                return None;
            }
            Some(CurrencyConfig::PointsAllocation(Box::new(
                PointsAllocationConfig {
                    allowance_amount: amount,
                    allowance_period: period,
                    allowance_start: start,
                },
            )))
        }
        CurrencyMode::DistributedClearing => {
            // Must not have allowance fields
            if allowance_amount.is_some()
                || allowance_period.is_some()
                || allowance_start.is_some()
            {
                return None;
            }
            // Without callable debts, must have finite credit limit
            if !debts_callable && default_credit_limit.is_none() {
                return None;
            }
            Some(CurrencyConfig::DistributedClearing(IOUConfig {
                default_credit_limit,
                debts_callable,
            }))
        }
        CurrencyMode::DeferredPayment => {
            // Must not have allowance fields
            if allowance_amount.is_some()
                || allowance_period.is_some()
                || allowance_start.is_some()
            {
                return None;
            }
            // Without callable debts, must have finite credit limit
            if !debts_callable && default_credit_limit.is_none() {
                return None;
            }
            Some(CurrencyConfig::DeferredPayment(IOUConfig {
                default_credit_limit,
                debts_callable,
            }))
        }
        CurrencyMode::PrepaidCredits => {
            // Must not have allowance fields
            if allowance_amount.is_some()
                || allowance_period.is_some()
                || allowance_start.is_some()
            {
                return None;
            }
            // Validate credit_limit is 0
            if default_credit_limit != Some(Decimal::ZERO) {
                return None;
            }
            Some(CurrencyConfig::PrepaidCredits(PrepaidCreditsConfig {
                debts_callable,
            }))
        }
    }
}

/// Convert CurrencyConfig to database columns for storage
pub fn currency_config_to_db(
    config: &payloads::CurrencyConfig,
) -> (
    CurrencyMode,
    Option<Decimal>,
    bool,
    Option<Decimal>,
    Option<jiff::Span>,
    Option<jiff::Timestamp>,
) {
    use payloads::CurrencyConfig;

    match config {
        CurrencyConfig::PointsAllocation(cfg) => (
            CurrencyMode::PointsAllocation,
            Some(cfg.credit_limit()),
            cfg.debts_callable(),
            Some(cfg.allowance_amount),
            Some(cfg.allowance_period),
            Some(cfg.allowance_start),
        ),
        CurrencyConfig::DistributedClearing(cfg) => (
            CurrencyMode::DistributedClearing,
            cfg.default_credit_limit,
            cfg.debts_callable,
            None,
            None,
            None,
        ),
        CurrencyConfig::DeferredPayment(cfg) => (
            CurrencyMode::DeferredPayment,
            cfg.default_credit_limit,
            cfg.debts_callable,
            None,
            None,
            None,
        ),
        CurrencyConfig::PrepaidCredits(cfg) => (
            CurrencyMode::PrepaidCredits,
            Some(cfg.credit_limit()),
            cfg.debts_callable,
            None,
            None,
            None,
        ),
    }
}

/// Update the credit limit for a member account with permission checking
///
/// Requires moderator or higher permissions.
pub async fn update_credit_limit(
    actor: &super::ValidatedMember,
    member_user_id: &UserId,
    credit_limit: Option<Decimal>,
    pool: &PgPool,
) -> Result<Account, StoreError> {
    // Check permissions
    if !actor.0.role.is_ge_moderator() {
        return Err(StoreError::RequiresModeratorPermissions);
    }

    let mut tx = pool.begin().await?;

    // Update the credit limit
    sqlx::query(
        r#"
        UPDATE accounts
        SET credit_limit = $1
        WHERE community_id = $2
          AND owner_type = 'member_main'
          AND owner_id = $3
        "#,
    )
    .bind(credit_limit)
    .bind(actor.0.community_id)
    .bind(member_user_id)
    .execute(&mut *tx)
    .await?;

    // Fetch and return the updated account
    let account = get_account_tx(
        &actor.0.community_id,
        AccountOwner::Member(*member_user_id),
        &mut tx,
    )
    .await?;

    tx.commit().await?;

    Ok(account)
}

/// Create a transfer from one member to another
///
/// Creates a journal entry with entry_type='transfer' that debits the
/// sender's account and credits the recipient's account.
///
/// The sender must be a validated member of the community. Any member can
/// send transfers to other members in the same community.
pub async fn create_transfer(
    sender: &super::ValidatedMember,
    to_user_id: &UserId,
    amount: Decimal,
    note: Option<String>,
    idempotency_key: IdempotencyKey,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    if amount <= Decimal::ZERO {
        return Err(StoreError::AmountMustBePositive);
    }

    let mut tx = pool.begin().await?;

    // Get both accounts
    let from_account = get_account_tx(
        &sender.0.community_id,
        AccountOwner::Member(sender.0.user_id),
        &mut tx,
    )
    .await?;
    let to_account = get_account_tx(
        &sender.0.community_id,
        AccountOwner::Member(*to_user_id),
        &mut tx,
    )
    .await?;

    // Create journal lines: debit sender, credit recipient
    let lines = vec![
        (from_account.id, -amount), // Debit
        (to_account.id, amount),    // Credit
    ];

    // Create the journal entry
    create_entry(
        &sender.0.community_id,
        EntryType::Transfer,
        idempotency_key,
        lines,
        None, // No auction_id
        None, // No initiated_by_id for member-to-member transfers
        note,
        false, // Not an auction settlement
        time_source,
        &mut tx,
    )
    .await?;

    tx.commit().await?;

    Ok(())
}

/// Unified treasury credit operation with permission checking
///
/// Credits one or more member accounts from the treasury. The entry type is
/// determined automatically based on the community's currency mode and the
/// recipient pattern.
///
/// Requires coleader or higher permissions.
///
/// Entry type selection:
/// - points_allocation + SingleMember → IssuanceGrantSingle
/// - points_allocation + AllActiveMembers → IssuanceGrantBulk
/// - distributed_clearing + AllActiveMembers → DistributionCorrection
/// - deferred_payment + SingleMember → DebtSettlement
/// - prepaid_credits + SingleMember → CreditPurchase
///
/// Returns the number of recipients and total amount debited from treasury.
pub async fn treasury_credit_operation(
    actor: &super::ValidatedMember,
    recipient: payloads::TreasuryRecipient,
    amount_per_recipient: Decimal,
    note: Option<String>,
    idempotency_key: IdempotencyKey,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<payloads::TreasuryOperationResult, StoreError> {
    // Check permissions
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    if amount_per_recipient <= Decimal::ZERO {
        return Err(StoreError::AmountMustBePositive);
    }

    let community_id = &actor.0.community_id;
    let initiated_by_id = &actor.0.user_id;

    let mut tx = pool.begin().await?;

    // Get community to determine currency mode
    let currency_mode: CurrencyMode = sqlx::query_scalar(
        "SELECT currency_mode FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_one(&mut *tx)
    .await?;

    // Get recipient list based on TreasuryRecipient
    let recipient_ids: Vec<UserId> = match &recipient {
        payloads::TreasuryRecipient::SingleMember(user_id) => {
            vec![*user_id]
        }
        payloads::TreasuryRecipient::AllActiveMembers => {
            sqlx::query_scalar(
                r#"
                SELECT user_id
                FROM community_members
                WHERE community_id = $1 AND is_active = true
                "#,
            )
            .bind(community_id)
            .fetch_all(&mut *tx)
            .await?
        }
    };

    let recipient_count = recipient_ids.len();

    if recipient_count == 0 {
        // No recipients - return early with zero result
        return Ok(payloads::TreasuryOperationResult {
            recipient_count: 0,
            total_amount: Decimal::ZERO,
        });
    }

    // Determine entry type based on mode and recipient pattern
    use CurrencyMode::*;
    use payloads::TreasuryRecipient::*;
    let entry_type = match (currency_mode, &recipient) {
        (PointsAllocation, SingleMember(_)) => EntryType::IssuanceGrantSingle,
        (PointsAllocation, AllActiveMembers) => EntryType::IssuanceGrantBulk,
        (DistributedClearing, AllActiveMembers) => {
            EntryType::DistributionCorrection
        }
        (DeferredPayment, SingleMember(_)) => EntryType::DebtSettlement,
        (PrepaidCredits, SingleMember(_)) => EntryType::CreditPurchase,
        _ => return Err(StoreError::InvalidTreasuryOperation),
    };

    // Get treasury account and lock it to prevent race conditions
    let treasury_account = get_account_for_update_tx(
        community_id,
        AccountOwner::Treasury,
        &mut tx,
    )
    .await?;

    // Build journal lines: one debit for treasury, one credit per recipient
    let mut lines: Vec<(AccountId, Decimal)> = Vec::new();

    // Calculate total amount to debit from treasury
    let total_amount =
        amount_per_recipient * Decimal::from(recipient_count as i64);

    // Check if this currency mode restricts treasury from going negative
    let prevent_negative_treasury = matches!(
        currency_mode,
        CurrencyMode::DistributedClearing | CurrencyMode::DeferredPayment
    );

    if prevent_negative_treasury {
        // Treasury balance going negative would mean treasury owes money
        if treasury_account.balance_cached - total_amount < Decimal::ZERO {
            return Err(StoreError::InsufficientBalance);
        }
    }

    // Single debit line for treasury
    lines.push((treasury_account.id, -total_amount));

    // Credit line for each recipient
    for recipient_user_id in &recipient_ids {
        let member_account = get_account_tx(
            community_id,
            AccountOwner::Member(*recipient_user_id),
            &mut tx,
        )
        .await?;
        lines.push((member_account.id, amount_per_recipient));
    }

    // Create journal entry
    create_entry(
        community_id,
        entry_type,
        idempotency_key,
        lines,
        None, // No auction_id
        Some(initiated_by_id),
        note,
        false, // Not an auction settlement
        time_source,
        &mut tx,
    )
    .await?;

    tx.commit().await?;

    Ok(payloads::TreasuryOperationResult {
        recipient_count,
        total_amount,
    })
}

/// Get treasury account for a community
///
/// Requires coleader+ permissions.
pub async fn get_treasury_account(
    actor: &super::ValidatedMember,
    pool: &PgPool,
) -> Result<Account, StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    get_account(&actor.0.community_id, AccountOwner::Treasury, pool).await
}

/// Get transaction history for treasury account
///
/// Requires coleader+ permissions.
pub async fn get_treasury_transactions(
    actor: &super::ValidatedMember,
    limit: i64,
    offset: i64,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::MemberTransaction>, StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    // Get treasury account
    let account =
        get_account(&actor.0.community_id, AccountOwner::Treasury, pool)
            .await?;

    fetch_account_transactions(
        &account.id,
        &actor.0.community_id,
        limit,
        offset,
        pool,
    )
    .await
}

/// Get member currency info with permission checking
///
/// If target_user_id is None, returns info for the actor.
/// If target_user_id is Some, requires coleader+ permissions.
pub async fn get_member_currency_info_with_permissions(
    actor: &super::ValidatedMember,
    target_user_id: Option<&UserId>,
    pool: &PgPool,
) -> Result<payloads::responses::MemberCurrencyInfo, StoreError> {
    let query_user_id = match target_user_id {
        None => actor.0.user_id,
        Some(uid) => {
            // Checking another user's info requires coleader+
            if !actor.0.role.is_ge_coleader() {
                return Err(StoreError::RequiresColeaderPermissions);
            }
            *uid
        }
    };

    // Verify the target user is a member of the community
    let target_member = super::get_validated_member(
        &query_user_id,
        &actor.0.community_id,
        pool,
    )
    .await?;

    // Call the existing function with the validated member
    get_member_currency_info(&target_member, pool).await
}

/// Get member transactions with permission checking
///
/// If target_user_id is None, returns transactions for the actor.
/// If target_user_id is Some, requires coleader+ permissions.
pub async fn get_member_transactions_with_permissions(
    actor: &super::ValidatedMember,
    target_user_id: Option<&UserId>,
    limit: i64,
    offset: i64,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::MemberTransaction>, StoreError> {
    let query_user_id = match target_user_id {
        None => actor.0.user_id,
        Some(uid) => {
            // Checking another user's transactions requires coleader+
            if !actor.0.role.is_ge_coleader() {
                return Err(StoreError::RequiresColeaderPermissions);
            }
            *uid
        }
    };

    // Verify the target user is a member of the community
    let target_member = super::get_validated_member(
        &query_user_id,
        &actor.0.community_id,
        pool,
    )
    .await?;

    // Call the existing function with the validated member
    get_member_transactions(&target_member, limit, offset, pool).await
}
