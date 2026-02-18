use super::*;
use anyhow::Context;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use payloads::UserId;
use sqlx::PgPool;

use crate::time::TimeSource;

/// Create a new user as would happen during signup.
pub async fn create_user(
    pool: &PgPool,
    username: &str,
    email: &str,
    password_hash: &str,
    time_source: &TimeSource,
) -> Result<User, StoreError> {
    // Validate username format
    let validation = payloads::requests::validate_username(username);
    if let Some(error_message) = validation.error_message() {
        return Err(StoreError::InvalidUsername(error_message.to_string()));
    }
    if email.len() > payloads::requests::EMAIL_MAX_LEN {
        return Err(StoreError::FieldTooLong);
    }
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (
                username,
                email,
                password_hash,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $4)
            RETURNING *;",
    )
    .bind(username)
    .bind(email)
    .bind(password_hash)
    .bind(time_source.now().to_sqlx())
    .fetch_one(pool)
    .await?;
    Ok(user)
}

/// Create a new user as would happen during signup.
pub async fn read_user(pool: &PgPool, id: &UserId) -> Result<User, StoreError> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1;")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StoreError::UserNotFound,
            e => StoreError::Database(e),
        })
}

pub async fn update_user_profile(
    user_id: &UserId,
    display_name: &Option<String>,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<User, StoreError> {
    let updated_user = sqlx::query_as::<_, User>(
        r#"
        UPDATE users SET display_name = $2, updated_at = $3
        WHERE id = $1 AND deleted_at IS NULL
        RETURNING *
        "#,
    )
    .bind(user_id.0)
    .bind(display_name.as_ref())
    .bind(time_source.now().to_sqlx())
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::UserNotFound,
        _ => StoreError::Database(e),
    })?;

    Ok(updated_user)
}

/// Delete a user account.
///
/// Attempts a hard delete first. If that fails due to foreign key constraints
/// (user has bids, round_space_results, or user_eligibilities), falls back to
/// anonymizing PII and setting `deleted_at` to preserve referential integrity
/// for auction history.
///
/// On anonymization, also removes: user_values, use_proxy_bidding, tokens, and
/// community_members entries.
///
/// Returns `UserIsLeader` error if the user is a leader of any community.
/// Leaders must transfer leadership before deleting their account.
pub async fn delete_user(
    pool: &PgPool,
    id: &UserId,
    time_source: &TimeSource,
) -> Result<User, StoreError> {
    // Subquery to check if user is a leader. Used in WHERE clauses to
    // atomically prevent deletion of leaders (avoiding race with promotion).
    let is_leader_subquery = "EXISTS (SELECT 1 FROM community_members WHERE user_id = $1 AND role = 'leader')";

    // Try hard delete first
    let delete_result = sqlx::query_as::<_, User>(&format!(
        "DELETE FROM users WHERE id = $1 AND NOT {is_leader_subquery} RETURNING *"
    ))
    .bind(id)
    .fetch_one(pool)
    .await;

    match delete_result {
        Ok(user) => Ok(user),
        Err(sqlx::Error::RowNotFound) => {
            // Either user doesn't exist, or they're a leader. Check which.
            let is_leader = sqlx::query_scalar::<_, bool>(&format!(
                "SELECT {is_leader_subquery}"
            ))
            .bind(id)
            .fetch_one(pool)
            .await?;

            if is_leader {
                Err(StoreError::UserIsLeader)
            } else {
                Err(StoreError::UserNotFound)
            }
        }
        Err(sqlx::Error::Database(db_err))
            if db_err.is_foreign_key_violation() =>
        {
            // FK violation means user has historical data that must be
            // preserved. This can happen via:
            // - bids.user_id → user placed auction bids
            // - auction_results.winning_user_id → user won auction rounds
            // - entry_lines.account_id (via accounts cascade) → user has
            //   transaction history
            //
            // In these cases, anonymize the user instead of deleting.
            let now = time_source.now().to_sqlx();
            let mut tx = pool.begin().await?;

            // Delete community_members first, with leader check in WHERE clause
            // to atomically verify they're not a leader before removing their
            // membership (which would orphan the community).
            let rows_deleted = sqlx::query(&format!(
                "DELETE FROM community_members WHERE user_id = $1 AND NOT {is_leader_subquery}"
            ))
            .bind(id)
            .execute(&mut *tx)
            .await?
            .rows_affected();

            // Check if deletion was blocked due to being a leader
            if rows_deleted == 0 {
                let is_leader = sqlx::query_scalar::<_, bool>(&format!(
                    "SELECT {is_leader_subquery}"
                ))
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;

                if is_leader {
                    return Err(StoreError::UserIsLeader);
                }
                // rows_deleted == 0 but not a leader means they had no
                // community memberships, which is fine
            }

            // Remove other non-historical user data
            sqlx::query("DELETE FROM user_values WHERE user_id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM use_proxy_bidding WHERE user_id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM tokens WHERE user_id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;

            // Anonymize PII and mark as unverified to block community actions
            let user = sqlx::query_as::<_, User>(
                r#"
                UPDATE users SET
                    email = 'deleted-' || id || '@deleted.local',
                    username = 'deleted-' || id::text,
                    password_hash = '',
                    display_name = NULL,
                    email_verified = false,
                    deleted_at = $2,
                    updated_at = $2
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(id)
            .bind(now)
            .fetch_one(&mut *tx)
            .await?;

            tx.commit().await?;
            Ok(user)
        }
        Err(e) => Err(e.into()),
    }
}

/// Create a token for email verification or password reset
#[tracing::instrument(skip(pool, time_source))]
pub async fn create_token(
    user_id: &UserId,
    action: TokenAction,
    expires_at: Timestamp,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<TokenId, StoreError> {
    let token_id = sqlx::query_as::<_, TokenId>(
        r#"
        INSERT INTO tokens (user_id, action, expires_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $4)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(action)
    .bind(expires_at.to_sqlx())
    .bind(time_source.now().to_sqlx())
    .fetch_one(pool)
    .await
    .context("Failed to create token")?;

    tracing::info!("Created {:?} token for user {}", action, user_id.0);
    Ok(token_id)
}

/// Find and validate a token for use
#[tracing::instrument(skip(pool, time_source))]
pub async fn consume_token(
    token_id: &TokenId,
    expected_action: TokenAction,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<UserId, StoreError> {
    let mut tx = pool.begin().await.context("Failed to begin transaction")?;

    // Get the token and validate it
    let token = sqlx::query_as::<_, Token>(
        r#"
        SELECT *
        FROM tokens
        WHERE id = $1
        "#,
    )
    .bind(token_id)
    .fetch_optional(&mut *tx)
    .await
    .context("Failed to fetch token")?
    .ok_or(StoreError::TokenNotFound)?;

    // Validate token
    if token.action != expected_action {
        return Err(StoreError::InvalidTokenAction);
    }

    if token.used {
        return Err(StoreError::TokenAlreadyUsed);
    }

    // Check expiration using the provided time source
    let now = time_source.now();
    if now > token.expires_at {
        return Err(StoreError::TokenExpired);
    }

    // Mark token as used
    sqlx::query(
        r#"
        UPDATE tokens
        SET used = true, updated_at = $2
        WHERE id = $1
        "#,
    )
    .bind(token_id)
    .bind(time_source.now().to_sqlx())
    .execute(&mut *tx)
    .await
    .context("Failed to mark token as used")?;

    tx.commit().await.context("Failed to commit transaction")?;

    tracing::info!(
        "Consumed {:?} token for user {}",
        expected_action,
        token.user_id.0
    );
    Ok(token.user_id)
}

/// Mark user's email as verified
///
/// Fails for anonymized users since verification tokens are deleted.
#[tracing::instrument(skip(pool, time_source))]
pub async fn verify_user_email(
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let rows_affected = sqlx::query(
        r#"
        UPDATE users
        SET email_verified = true, updated_at = $2
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .bind(time_source.now().to_sqlx())
    .execute(pool)
    .await
    .context("Failed to verify user email")?
    .rows_affected();

    if rows_affected == 0 {
        return Err(StoreError::UserNotFound);
    }

    tracing::info!("Verified email for user {}", user_id.0);
    Ok(())
}

/// Get user by email for password reset
#[tracing::instrument(skip(pool))]
pub async fn get_user_by_email(
    email: &str,
    pool: &PgPool,
) -> Result<User, StoreError> {
    sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = $1 AND deleted_at IS NULL",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
    .context("Failed to fetch user by email")?
    .ok_or(StoreError::UserNotFound)
}

/// Clean up expired tokens
#[tracing::instrument(skip(pool, time_source))]
pub async fn cleanup_expired_tokens(
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<u64, StoreError> {
    let now = time_source.now();

    let result = sqlx::query(
        r#"
        DELETE FROM tokens
        WHERE expires_at < $1
        "#,
    )
    .bind(now.to_sqlx())
    .execute(pool)
    .await
    .context("Failed to cleanup expired tokens")?;

    tracing::info!("Cleaned up {} expired tokens", result.rows_affected());
    Ok(result.rows_affected())
}
