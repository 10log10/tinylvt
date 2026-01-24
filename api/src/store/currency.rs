//! Currency and ledger operations
//!
//! Implements double-entry accounting with:
//! - Account management (member_main and treasury accounts)
//! - Journal entry creation with balance updates
//! - Credit limit enforcement
//! - Idempotency support

use jiff::Timestamp;
use jiff_sqlx::{Timestamp as SqlxTs, ToSqlx};
use payloads::{
    Account, AccountId, AccountOwner, AccountOwnerType, CommunityId,
    CurrencyMode, EntryType, IdempotencyKey, JournalEntry, JournalEntryId,
    UserId,
};
use rust_decimal::Decimal;
use sqlx::{FromRow, PgPool};

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

/// Create an account for a member or treasury (pool version)
pub async fn create_account(
    community_id: &CommunityId,
    owner: AccountOwner,
    credit_limit: Option<Decimal>,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<Account, StoreError> {
    let mut tx = pool.begin().await?;
    let account = create_account_tx(
        community_id,
        owner,
        credit_limit,
        time_source,
        &mut tx,
    )
    .await?;
    tx.commit().await?;
    Ok(account)
}

/// Get account by owner
pub async fn get_account(
    community_id: &CommunityId,
    owner: AccountOwner,
    pool: &PgPool,
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
    .fetch_optional(pool)
    .await?
    .ok_or(StoreError::AccountNotFound)?;

    db_account.try_into()
}

/// Get account by owner and lock for update
///
/// Locks the account row using SELECT FOR UPDATE, preventing concurrent
/// modifications until the transaction commits. Must be called inside a
/// transaction.
pub async fn get_account_for_update_tx(
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

/// Get current cached balance for an account (transaction version)
pub async fn get_balance_tx(
    account_id: &AccountId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Decimal, StoreError> {
    let row: (Decimal,) = sqlx::query_as(
        r#"
        SELECT balance_cached FROM accounts WHERE id = $1
        "#,
    )
    .bind(account_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(StoreError::AccountNotFound)?;

    Ok(row.0)
}

/// Get effective credit limit for an account, excluding any locked balance
/// pledged via auction bids.
///
/// Returns account-specific limit if set, otherwise community default
pub async fn get_effective_credit_limit_tx(
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
pub async fn get_locked_balance_tx(
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
pub async fn get_available_credit_tx(
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
pub async fn check_sufficient_credit_tx(
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
/// 2. Checks credit limits for all affected accounts
/// 3. Creates the journal entry and lines
/// 4. Updates balance_cached for all accounts
///
/// Uses idempotency_key for deduplication - if key exists, returns Ok
/// without error.
#[allow(clippy::too_many_arguments)]
pub async fn create_entry(
    community_id: &CommunityId,
    entry_type: EntryType,
    idempotency_key: IdempotencyKey,
    lines: Vec<(AccountId, Decimal)>,
    auction_id: Option<&payloads::AuctionId>,
    initiated_by_id: Option<&UserId>,
    note: Option<String>,
    time_source: &TimeSource,
    pool: &PgPool,
) -> Result<(), StoreError> {
    // Check idempotency
    let existing: Option<JournalEntryId> = sqlx::query_scalar(
        "SELECT id FROM journal_entries WHERE idempotency_key = $1",
    )
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await?;

    if existing.is_some() {
        return Ok(()); // Idempotent - already processed
    }

    // Validate only one line per account
    let accounts: std::collections::HashSet<AccountId> =
        lines.iter().map(|(account_id, _)| *account_id).collect();
    if accounts.len() != lines.len() {
        return Err(StoreError::DuplicateAccountInJournalEntry);
    }

    // Validate lines sum to zero
    let sum: Decimal = lines.iter().map(|(_, amount)| amount).sum();
    if sum != Decimal::ZERO {
        return Err(StoreError::JournalLinesDoNotSumToZero(sum));
    }

    let now = time_source.now();

    // Begin transaction
    let mut tx = pool.begin().await?;

    // Collect debited accounts and sort by ID to prevent deadlocks
    let mut debited_accounts: Vec<_> = lines
        .iter()
        .filter(|(_, amount)| *amount < Decimal::ZERO)
        .map(|(account_id, _)| *account_id)
        .collect();
    debited_accounts.sort_by_key(|id| id.to_string());
    debited_accounts.dedup();

    // Lock debited accounts using SELECT FOR UPDATE
    // Ensures there's no changes between when the available credit is checked
    // and when the debit is committed.
    for account_id in &debited_accounts {
        sqlx::query("SELECT 1 FROM accounts WHERE id = $1 FOR UPDATE")
            .bind(account_id)
            .execute(&mut *tx)
            .await?;
    }

    // Check credit limits BEFORE making changes
    // We only need to check each line since we've validated there's only one
    // line per account.
    for (account_id, amount) in &lines {
        if *amount >= Decimal::ZERO {
            continue; // Skip credits, only check debits
        }

        // Check if account has sufficient credit for this debit
        // Pass absolute value since check_sufficient_credit_tx expects
        // positive amount
        check_sufficient_credit_tx(account_id, amount.abs(), &mut tx).await?;
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
    .fetch_one(&mut *tx)
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
        .execute(&mut *tx)
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
        .execute(&mut *tx)
        .await?;
    }

    // Commit transaction
    tx.commit().await?;

    Ok(())
}

/// Get journal entries for an account
pub async fn get_transactions(
    account_id: &AccountId,
    limit: i64,
    offset: i64,
    pool: &PgPool,
) -> Result<Vec<JournalEntry>, StoreError> {
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

    Ok(entries)
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
        CurrencyConfig, DeferredPaymentConfig, DistributedClearingConfig,
        PointsAllocationConfig, PrepaidCreditsConfig,
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
            Some(CurrencyConfig::DistributedClearing(
                DistributedClearingConfig {
                    default_credit_limit,
                    debts_callable,
                },
            ))
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
            Some(CurrencyConfig::DeferredPayment(DeferredPaymentConfig {
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
