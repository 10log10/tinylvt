use super::*;
use jiff_sqlx::ToSqlx;
use payloads::{CommunityId, InviteId, Role, UserId, requests};
use sqlx::{PgPool, Row};
use tracing::Level;

use crate::time::TimeSource;

/// Create a community and add the creating user as the leader.
pub async fn create_community(
    details: &requests::CreateCommunity,
    user_id: UserId, // initial leader of community
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<Community, StoreError> {
    let user = read_user(pool, &user_id).await?;
    if !user.email_verified {
        return Err(StoreError::UnverifiedEmail);
    }
    if details.name.len() > payloads::requests::COMMUNITY_NAME_MAX_LEN {
        return Err(StoreError::FieldTooLong);
    }
    let mut tx = pool.begin().await?;

    // Validate and convert currency config enum to database columns
    // For IOU modes: if debts aren't callable, must have finite credit limit
    match &details.currency.mode_config {
        payloads::CurrencyModeConfig::DistributedClearing(cfg)
        | payloads::CurrencyModeConfig::DeferredPayment(cfg) => {
            if !cfg.debts_callable && cfg.default_credit_limit.is_none() {
                return Err(StoreError::InvalidCurrencyConfiguration);
            }
        }
        _ => {}
    }

    let currency_db = currency::currency_settings_to_db(&details.currency);

    let db_community = sqlx::query_as::<_, DbCommunity>(
        "INSERT INTO communities (
            name,
            new_members_default_active,
            currency_mode,
            default_credit_limit,
            debts_callable,
            currency_name,
            currency_symbol,
            currency_minor_units,
            balances_visible_to_members,
            allowance_amount,
            allowance_period,
            allowance_start,
            created_at,
            updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $13) RETURNING *;",
    )
    .bind(&details.name)
    .bind(currency_db.new_members_default_active)
    .bind(currency_db.mode)
    .bind(currency_db.default_credit_limit)
    .bind(currency_db.debts_callable)
    .bind(&currency_db.currency_name)
    .bind(&currency_db.currency_symbol)
    .bind(currency_db.currency_minor_units)
    .bind(currency_db.balances_visible_to_members)
    .bind(currency_db.allowance_amount)
    .bind(currency_db.allowance_period.as_ref().map(span_to_interval).transpose()?)
    .bind(currency_db.allowance_start.as_ref().map(|t| t.to_sqlx()))
    .bind(time_source.now().to_sqlx())
    .fetch_one(&mut *tx)
    .await?;

    let community: Community = db_community.try_into()?;

    sqlx::query(
        "INSERT INTO community_members (community_id, user_id, role, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $4);",
    )
    .bind(community.id)
    .bind(user_id)
    .bind(Role::Leader)
    .bind(time_source.now().to_sqlx())
    .execute(&mut *tx)
    .await?;

    // Create treasury account
    currency::create_account_tx(
        &community.id,
        payloads::AccountOwner::Treasury,
        None,
        time_source,
        &mut tx,
    )
    .await?;

    // Create leader's member_main account
    currency::create_account_tx(
        &community.id,
        payloads::AccountOwner::Member(user_id),
        None,
        time_source,
        &mut tx,
    )
    .await?;

    tx.commit().await?;

    Ok(community)
}

pub async fn get_validated_member(
    user_id: &UserId,
    community_id: &CommunityId,
    pool: &PgPool,
) -> Result<ValidatedMember, StoreError> {
    let Some(member) = sqlx::query_as::<_, CommunityMember>(
        "SELECT * FROM community_members WHERE
            community_id = $1 AND user_id = $2;",
    )
    .bind(community_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    else {
        return Err(StoreError::MemberNotFound);
    };
    Ok(ValidatedMember(member))
}

/// Batch fetch user identities for a list of user IDs
///
/// Returns a HashMap of user_id -> UserIdentity. This is useful for
/// efficiently fetching display information for multiple users at once.
pub(crate) async fn get_user_identities(
    user_ids: &[UserId],
    // TODO: migrate display_name to community_member table and use this param
    _community_id: &CommunityId,
    pool: &PgPool,
) -> Result<
    std::collections::HashMap<UserId, payloads::responses::UserIdentity>,
    StoreError,
> {
    if user_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let identities: Vec<payloads::responses::UserIdentity> = sqlx::query_as(
        r#"
        SELECT
            u.id as user_id,
            u.username,
            u.display_name
        FROM users u
        WHERE u.id = ANY($1)
        "#,
    )
    .bind(user_ids)
    .fetch_all(pool)
    .await?;

    Ok(identities
        .into_iter()
        .map(|identity| (identity.user_id, identity))
        .collect())
}

/// Helper to enrich a collection of items with user identities
///
/// Given a collection of items, extracts user IDs, batch loads their identities,
/// and maps each item to a result using the provided mapper function.
pub(crate) async fn with_user_identities<T, R, F>(
    items: Vec<T>,
    get_user_id: impl Fn(&T) -> UserId,
    mapper: F,
    community_id: &CommunityId,
    pool: &PgPool,
) -> Result<Vec<R>, StoreError>
where
    F: Fn(T, payloads::responses::UserIdentity) -> Result<R, StoreError>,
{
    if items.is_empty() {
        return Ok(Vec::new());
    }

    // Extract user IDs
    let user_ids: Vec<UserId> = items.iter().map(&get_user_id).collect();

    // Batch fetch identities
    let user_identities =
        get_user_identities(&user_ids, community_id, pool).await?;

    // Map items with their identities
    items
        .into_iter()
        .map(|item| {
            let user_id = get_user_id(&item);
            let identity = user_identities
                .get(&user_id)
                .cloned()
                .ok_or(StoreError::UserNotFound)?;
            mapper(item, identity)
        })
        .collect()
}

pub async fn invite_community_member(
    actor: &ValidatedMember,
    new_member_email: &Option<String>,
    single_use: bool,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<InviteId, StoreError> {
    if !actor.0.role.is_ge_moderator() {
        return Err(StoreError::RequiresModeratorPermissions);
    }
    let invite = sqlx::query_as::<_, CommunityInvite>(
        "INSERT INTO community_invites (community_id, email, single_use, created_at)
        VALUES ($1, $2, $3, $4) RETURNING *;",
    )
    .bind(actor.0.community_id)
    .bind(new_member_email)
    .bind(single_use)
    .bind(time_source.now().to_sqlx())
    .fetch_one(pool)
    .await?;
    Ok(invite.id)
}

pub async fn get_invite_community_name(
    invite_id: &payloads::InviteId,
    pool: &PgPool,
) -> Result<String, StoreError> {
    let community_name = sqlx::query_scalar::<_, String>(
        "SELECT c.name
         FROM community_invites ci
         JOIN communities c ON ci.community_id = c.id
         WHERE ci.id = $1;",
    )
    .bind(invite_id)
    .fetch_optional(pool)
    .await?;

    let Some(community_name) = community_name else {
        return Err(StoreError::CommunityInviteNotFound);
    };

    Ok(community_name)
}

pub async fn accept_invite(
    user_id: &UserId,
    invite_id: &payloads::InviteId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let user = read_user(pool, user_id).await?;
    if !user.email_verified {
        return Err(StoreError::UnverifiedEmail);
    }
    let invite = sqlx::query_as::<_, CommunityInvite>(
        "SELECT * FROM community_invites WHERE id = $1;",
    )
    .bind(invite_id)
    .fetch_optional(pool)
    .await?;
    let Some(invite) = invite else {
        return Err(StoreError::CommunityInviteNotFound);
    };
    if let Some(ref invite_email) = invite.email
        && *invite_email != user.email
    {
        return Err(StoreError::MismatchedInviteEmail);
    }

    // Fetch community to get new_members_default_active setting
    let community = get_community_by_id(&invite.community_id, pool).await?;
    let is_active = community.currency.new_members_default_active;

    let mut tx = pool.begin().await?;

    // Check if an orphaned account exists (user previously left)
    let orphaned_account_exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM accounts
            WHERE community_id = $1
              AND owner_id = $2
              AND owner_type = 'member_main'
        )
        "#,
    )
    .bind(invite.community_id)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    // Insert community_members row (new member or returning member)
    let result = sqlx::query(
        "INSERT INTO community_members (community_id, user_id, role, is_active, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $5);",
    )
    .bind(invite.community_id)
    .bind(user_id)
    .bind(Role::Member)
    .bind(is_active)
    .bind(time_source.now().to_sqlx())
    .execute(&mut *tx)
    .await;

    if let Err(StoreError::NotUnique(_)) = result.map_err(StoreError::from) {
        return Err(StoreError::AlreadyMember);
    }

    // Only create account if this is a new member (no orphaned account)
    if !orphaned_account_exists {
        currency::create_account_tx(
            &invite.community_id,
            payloads::AccountOwner::Member(*user_id),
            None,
            time_source,
            &mut tx,
        )
        .await?;
    }
    // else: Orphaned account exists, member is reconnecting to it

    if invite.email.is_some() || invite.single_use {
        sqlx::query("DELETE FROM community_invites WHERE id = $1")
            .bind(invite_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    Ok(())
}

pub async fn delete_invite(
    actor: &ValidatedMember,
    invite_id: &payloads::InviteId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    if !actor.0.role.is_ge_moderator() {
        return Err(StoreError::RequiresModeratorPermissions);
    }

    // Verify the invite exists and belongs to this community
    let invite_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM community_invites WHERE id = $1 AND community_id = $2)",
    )
    .bind(invite_id)
    .bind(actor.0.community_id)
    .fetch_one(pool)
    .await?;

    if !invite_exists {
        return Err(StoreError::CommunityInviteNotFound);
    }

    // Delete the invite
    sqlx::query("DELETE FROM community_invites WHERE id = $1")
        .bind(invite_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn get_communities(
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::CommunityWithRole>, StoreError> {
    let rows = sqlx::query(
        "SELECT
            b.*,
            a.role as user_role,
            a.is_active as user_is_active
        FROM community_members a
        JOIN communities b ON a.community_id = b.id
        WHERE a.user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut communities = Vec::new();
    for row in rows {
        let db_community = DbCommunity::from_row(&row)?;
        let community: Community = db_community.try_into()?;
        let user_role: Role = row.try_get("user_role")?;
        let user_is_active: bool = row.try_get("user_is_active")?;
        communities.push(payloads::responses::CommunityWithRole {
            community,
            user_role,
            user_is_active,
        });
    }

    Ok(communities)
}

pub async fn get_community_by_id(
    community_id: &CommunityId,
    pool: &PgPool,
) -> Result<Community, StoreError> {
    let db_community = sqlx::query_as::<_, DbCommunity>(
        "SELECT * FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_optional(pool)
    .await?
    .ok_or(StoreError::CommunityNotFound)?;

    db_community.try_into()
}

pub async fn get_received_invites(
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<responses::CommunityInviteReceived>, StoreError> {
    let user = read_user(pool, user_id).await?;
    // Need to make sure this user actually owns this email before showing them
    // the invites they've received
    if !user.email_verified {
        return Err(StoreError::UnverifiedEmail);
    }
    Ok(sqlx::query_as::<_, responses::CommunityInviteReceived>(
        "SELECT
            a.*,
            b.name as community_name
        FROM community_invites a
        JOIN communities b ON a.community_id = b.id
        WHERE a.email = $1",
    )
    .bind(user.email)
    .fetch_all(pool)
    .await?)
}

pub async fn get_issued_invites(
    actor: &ValidatedMember,
    pool: &PgPool,
) -> Result<Vec<responses::IssuedCommunityInvite>, StoreError> {
    if !actor.0.role.is_ge_moderator() {
        return Err(StoreError::RequiresModeratorPermissions);
    }

    Ok(sqlx::query_as::<_, responses::IssuedCommunityInvite>(
        "SELECT
            id,
            email as new_member_email,
            single_use,
            created_at
        FROM community_invites
        WHERE community_id = $1
        ORDER BY created_at DESC",
    )
    .bind(actor.0.community_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_members(
    actor: &ValidatedMember,
    pool: &PgPool,
) -> Result<Vec<responses::CommunityMember>, StoreError> {
    let should_include_balances = actor.0.role.is_ge_coleader()
        || sqlx::query_scalar::<_, bool>(
            "SELECT balances_visible_to_members
            FROM communities
            WHERE id = $1",
        )
        .bind(actor.0.community_id)
        .fetch_one(pool)
        .await?;

    #[derive(sqlx::FromRow)]
    struct DbMember {
        user_id: UserId,
        role: Role,
        is_active: bool,
        balance: Option<rust_decimal::Decimal>,
    }

    let db_members: Vec<DbMember> = if should_include_balances {
        sqlx::query_as(
            "SELECT cm.user_id, cm.role, cm.is_active,
                    a.balance_cached AS balance
            FROM community_members cm
            LEFT JOIN accounts a
                ON a.community_id = cm.community_id
                AND a.owner_id = cm.user_id
                AND a.owner_type = 'member_main'
            WHERE cm.community_id = $1
            ORDER BY cm.created_at ASC",
        )
        .bind(actor.0.community_id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT user_id, role, is_active,
                    NULL::numeric AS balance
            FROM community_members
            WHERE community_id = $1
            ORDER BY created_at ASC",
        )
        .bind(actor.0.community_id)
        .fetch_all(pool)
        .await?
    };

    with_user_identities(
        db_members,
        |m| m.user_id,
        |m, user| {
            Ok(responses::CommunityMember {
                user,
                role: m.role,
                is_active: m.is_active,
                balance: m.balance,
            })
        },
        &actor.0.community_id,
        pool,
    )
    .await
}

pub async fn remove_member(
    actor: &ValidatedMember,
    member_user_id: &UserId,
    pool: &PgPool,
    _time_source: &TimeSource,
) -> Result<(), StoreError> {
    // Permission check: Moderator+
    if !actor.0.role.is_ge_moderator() {
        return Err(StoreError::RequiresModeratorPermissions);
    }

    // Cannot remove yourself (use leave_community instead)
    if member_user_id == &actor.0.user_id {
        return Err(StoreError::CannotRemoveSelf);
    }

    // Get target member to validate they exist and check their role
    let target_member =
        get_validated_member(member_user_id, &actor.0.community_id, pool)
            .await?;

    let mut tx = pool.begin().await?;

    // Cannot remove higher role
    if !actor.0.role.can_remove_role(&target_member.0.role) {
        return Err(StoreError::CannotRemoveHigherRole);
    }

    // Delete the community_members row (account persists)
    // Include role != 'leader' check to prevent race condition where member
    // gets promoted to leader between our check and deletion
    let rows_deleted = sqlx::query(
        "DELETE FROM community_members
         WHERE community_id = $1 AND user_id = $2 AND role != 'leader'",
    )
    .bind(actor.0.community_id)
    .bind(member_user_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    // If no rows deleted, member was promoted to leader (race condition).
    // Or member was already removed/left (also race).
    // The error isn't accurate for the latter case but a retry will then yield
    // MemberNotFound.
    if rows_deleted == 0 {
        return Err(StoreError::CannotRemoveHigherRole);
    }

    tx.commit().await?;
    Ok(())
}

pub async fn change_member_role(
    actor: &ValidatedMember,
    member_user_id: &UserId,
    new_role: Role,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    // Cannot change own role
    if member_user_id == &actor.0.user_id {
        return Err(StoreError::CannotChangeSelfRole);
    }

    // Cannot promote to leader
    if new_role.is_leader() {
        return Err(StoreError::CannotPromoteToLeader);
    }

    // Minimum permission: must be coleader+
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    // Get target member to validate they exist and check their role
    let target_member =
        get_validated_member(member_user_id, &actor.0.community_id, pool)
            .await?;

    // Check role change is allowed
    if !actor
        .0
        .role
        .can_change_role(&target_member.0.role, &new_role)
    {
        return Err(StoreError::CannotChangeRole);
    }

    // Atomic update with race condition protection
    let rows_updated = sqlx::query(
        "UPDATE community_members
         SET role = $1, updated_at = $2
         WHERE community_id = $3 AND user_id = $4 AND role != 'leader'",
    )
    .bind(new_role)
    .bind(time_source.now().to_sqlx())
    .bind(actor.0.community_id)
    .bind(member_user_id)
    .execute(pool)
    .await?
    .rows_affected();

    if rows_updated == 0 {
        // Target was promoted to leader (race condition) or doesn't exist
        return Err(StoreError::CannotChangeRole);
    }

    Ok(())
}

pub async fn leave_community(
    member: &ValidatedMember,
    pool: &PgPool,
) -> Result<(), StoreError> {
    // Early check for leader (avoids unnecessary delete attempt)
    if member.0.role.is_leader() {
        return Err(StoreError::LeaderMustTransferFirst);
    }

    // Delete the community_members row (but not if leader).
    // Atomic check during deletion prevents races.
    let rows_deleted = sqlx::query(
        "DELETE FROM community_members
         WHERE community_id = $1 AND user_id = $2 AND role != 'leader'",
    )
    .bind(member.0.community_id)
    .bind(member.0.user_id)
    .execute(pool)
    .await?
    .rows_affected();

    // If no rows deleted, user was promoted to leader (race condition)
    // or the user was already removed.
    if rows_deleted == 0 {
        return Err(StoreError::LeaderMustTransferFirst);
    }

    Ok(())
}

pub async fn set_membership_schedule(
    actor: &ValidatedMember,
    schedule: &[payloads::MembershipSchedule],
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    if !actor.0.role.is_ge_moderator() {
        return Err(StoreError::RequiresModeratorPermissions);
    }

    let mut tx = pool.begin().await?;

    sqlx::query(
        "DELETE FROM community_membership_schedule
        WHERE community_id = $1",
    )
    .bind(actor.0.community_id)
    .execute(&mut *tx)
    .await?;

    for sched_elem in schedule {
        sqlx::query(
            "INSERT INTO community_membership_schedule (
                community_id,
                start_at,
                end_at,
                email,
                created_at,
                updated_at
            ) VALUES ($1, $2, $3, $4, $5, $5);",
        )
        .bind(actor.0.community_id)
        .bind(sched_elem.start_at.to_sqlx())
        .bind(sched_elem.end_at.to_sqlx())
        .bind(&sched_elem.email)
        .bind(time_source.now().to_sqlx())
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}

/// Update the active status of a community member (moderator+ only).
/// This is a manual override independent of the membership schedule.
pub async fn update_member_active_status(
    actor: &ValidatedMember,
    member_user_id: &UserId,
    is_active: bool,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    // Check permissions
    if !actor.0.role.can_change_active_status() {
        return Err(StoreError::RequiresModeratorPermissions);
    }

    // Verify target member exists
    let _target_member =
        get_validated_member(member_user_id, &actor.0.community_id, pool)
            .await?;

    // Update active status
    sqlx::query(
        "UPDATE community_members
        SET is_active = $1, updated_at = $2
        WHERE community_id = $3 AND user_id = $4",
    )
    .bind(is_active)
    .bind(time_source.now().to_sqlx())
    .bind(actor.0.community_id)
    .bind(member_user_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_membership_schedule(
    actor: &ValidatedMember,
    pool: &PgPool,
) -> Result<Vec<payloads::MembershipSchedule>, StoreError> {
    if !actor.0.role.is_ge_moderator() {
        return Err(StoreError::RequiresModeratorPermissions);
    }

    Ok(sqlx::query_as::<_, payloads::MembershipSchedule>(
        "SELECT * FROM community_membership_schedule
        WHERE community_id = $1",
    )
    .bind(actor.0.community_id)
    .fetch_all(pool)
    .await?)
}

#[derive(Debug, Clone, FromRow)]
struct MemberInSchedule {
    community_id: CommunityId,
    user_id: UserId,
    email: String,
}

#[tracing::instrument(skip(pool, time_source), err(level = Level::ERROR))]
/// Update members' is_active status in all communities based on the schedule,
/// if they are present in the schedule.
pub async fn update_is_active_from_schedule(
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    // Get all (community, user) pairs in the schedule table. Only these
    // community members are to have their is_active status updated.
    let community_members_in_schedule = sqlx::query_as::<_, MemberInSchedule>(
        "SELECT DISTINCT
            a.community_id,
            a.email,
            u.id as user_id
        FROM community_membership_schedule a
        JOIN users u ON a.email = u.email;",
    )
    .fetch_all(pool)
    .await?;

    let now = time_source.now().to_sqlx();
    // Might as well make sure we update everything or nothing to avoid
    // partially completed state.
    let mut tx = pool.begin().await?;
    for community_member in community_members_in_schedule {
        // Set is_active if we can find a matching row in the schedule where the
        // user is meant to be a member.
        sqlx::query(
            "UPDATE community_members m
            SET is_active = EXISTS (
                SELECT 1
                FROM community_membership_schedule a
                WHERE
                    a.email = $1
                    AND a.community_id = $2
                    AND a.start_at <= $3
                    AND a.end_at > $3
            ),
            updated_at = $5
            WHERE
                m.user_id = $4
                AND m.community_id = $2",
        )
        .bind(&community_member.email)
        .bind(community_member.community_id)
        .bind(now)
        .bind(community_member.user_id)
        .bind(time_source.now().to_sqlx())
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}

pub async fn delete_community(
    community_id: &payloads::CommunityId,
    actor: &ValidatedMember,
    pool: &PgPool,
) -> Result<(), StoreError> {
    // Only leader can delete a community
    if !actor.0.role.is_leader() {
        return Err(StoreError::RequiresLeaderPermissions);
    }

    let mut tx = pool.begin().await?;

    // Delete journal_entries first to unblock cascade deletions.
    // The ledger uses RESTRICT on auction_id and account_id FKs to preserve
    // financial history, but community deletion is the one case where we
    // intentionally destroy everything.
    sqlx::query("DELETE FROM journal_entries WHERE community_id = $1")
        .bind(community_id)
        .execute(&mut *tx)
        .await?;

    // Delete community - cascades to:
    // - community_members
    // - community_invites
    // - community_membership_schedule
    // - site_images
    // - sites (which cascades to spaces, auctions, etc.)
    // - accounts
    let result = sqlx::query("DELETE FROM communities WHERE id = $1")
        .bind(community_id)
        .execute(&mut *tx)
        .await?;

    if result.rows_affected() == 0 {
        return Err(StoreError::CommunityNotFound);
    }

    tx.commit().await?;

    // Clean up orphaned auction params
    cleanup_unused_auction_params(pool).await;

    Ok(())
}
