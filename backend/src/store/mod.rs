use anyhow::Context;
use derive_more::Display;
use jiff::Span;
use jiff::{Timestamp, civil::Time};
use jiff_sqlx::ToSqlx;
use jiff_sqlx::{Span as SqlxSpan, Timestamp as SqlxTs};
use rust_decimal::Decimal;
use sqlx::types::Json;
use sqlx::{Error, FromRow, PgPool, Postgres, Transaction};
use sqlx_postgres::types::PgInterval;
use tracing::Level;
use uuid::Uuid;

use payloads::{
    AuctionId, AuctionRoundId, Bid, CommunityId, InviteId, PermissionLevel,
    Role, SiteId, SpaceId, UserId, requests,
    responses::{self, Community},
};

use crate::time;

impl From<Space> for payloads::Space {
    fn from(space: Space) -> Self {
        Self {
            site_id: space.site_id,
            name: space.name,
            description: space.description,
            eligibility_points: space.eligibility_points,
            is_available: space.is_available,
        }
    }
}

impl From<Space> for payloads::responses::Space {
    fn from(space: Space) -> Self {
        Self {
            space_id: space.id,
            created_at: space.created_at,
            updated_at: space.updated_at,
            space_details: space.into(),
        }
    }
}

/// A complete user row that stays in the backend.
#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: UserId,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub email_verified: bool,
    pub balance: Decimal,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Display, sqlx::Type)]
#[sqlx(transparent)]
pub struct TokenId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct Token {
    pub id: TokenId,
    pub action: String,
    pub used: bool,
    #[sqlx(try_from = "SqlxTs")]
    pub expires_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, FromRow)]
pub struct CommunityMember {
    pub community_id: CommunityId,
    pub user_id: UserId,
    pub role: Role,
    pub is_active: bool,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

#[derive(sqlx::Type)]
#[sqlx(transparent)]
struct OptionalTimestamp(Option<SqlxTs>);

impl From<OptionalTimestamp> for Option<Timestamp> {
    fn from(x: OptionalTimestamp) -> Option<Timestamp> {
        x.0.map(|x| x.to_jiff())
    }
}

/// A type that can only exist if the interior CommunityMember has been
/// validated to exist.
pub struct ValidatedMember(CommunityMember);

#[derive(Debug, Clone, FromRow)]
pub struct CommunityInvite {
    pub id: InviteId,
    pub community_id: CommunityId,
    pub email: Option<String>,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, sqlx::FromRow)]
#[sqlx(transparent)]
pub struct CommunityMembershipScheduleId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct CommunityMembershipSchedule {
    pub id: CommunityMembershipScheduleId,
    pub community_id: CommunityId,
    #[sqlx(try_from = "SqlxTs")]
    pub start_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub end_at: Timestamp,
    pub email: String,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, sqlx::FromRow)]
#[sqlx(transparent)]
pub struct AuctionParamsId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct AuctionParams {
    pub id: AuctionParamsId,
    #[sqlx(try_from = "SqlxSpan")]
    pub round_duration: Span,
    pub bid_increment: Decimal,
    pub activity_rule_params: Json<payloads::ActivityRuleParams>,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

impl From<AuctionParams> for payloads::AuctionParams {
    fn from(params: AuctionParams) -> Self {
        Self {
            round_duration: params.round_duration,
            bid_increment: params.bid_increment,
            activity_rule_params: params.activity_rule_params.0,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, sqlx::Type, sqlx::FromRow)]
#[sqlx(transparent)]
pub struct OpenHoursId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct OpenHours {
    pub id: OpenHoursId,
}

#[derive(Debug, Clone, FromRow)]
pub struct OpenHoursWeekday {
    pub open_hours_id: OpenHoursId,
    pub day_of_week: i16,
    #[sqlx(try_from = "jiff_sqlx::Time")]
    pub open_time: Time,
    #[sqlx(try_from = "jiff_sqlx::Time")]
    pub close_time: Time,
}

#[derive(Debug, Clone, FromRow)]
pub struct Site {
    pub id: SiteId,
    pub community_id: CommunityId,
    pub name: String,
    pub description: Option<String>,
    pub default_auction_params_id: AuctionParamsId,
    #[sqlx(try_from = "SqlxSpan")]
    pub possession_period: Span,
    #[sqlx(try_from = "SqlxSpan")]
    pub auction_lead_time: Span,
    #[sqlx(try_from = "SqlxSpan")]
    pub proxy_bidding_lead_time: Span,
    pub open_hours_id: Option<OpenHoursId>,
    pub auto_schedule: bool,
    pub site_image_id: Option<SiteImageId>,
    pub timezone: String,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, FromRow)]
pub struct Space {
    pub id: SpaceId,
    pub site_id: SiteId,
    pub name: String,
    pub description: Option<String>,
    pub eligibility_points: f64,
    pub is_available: bool,
    pub site_image_id: Option<SiteImageId>,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct SiteImageId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct SiteImage {
    pub id: SiteImageId,
    pub site_id: SiteId,
    pub name: String,
    pub image_data: Vec<u8>,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, FromRow)]
pub struct Auction {
    pub id: AuctionId,
    pub site_id: SiteId,
    #[sqlx(try_from = "SqlxTs")]
    pub possession_start_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub possession_end_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub start_at: Timestamp,
    #[sqlx(try_from = "OptionalTimestamp")]
    pub end_at: Option<Timestamp>,
    pub auction_params_id: AuctionParamsId,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

impl Auction {
    // Helper to convert to response type with params
    pub fn with_params(
        self,
        params: AuctionParams,
    ) -> payloads::responses::Auction {
        payloads::responses::Auction {
            auction_id: self.id,
            auction_details: payloads::Auction {
                site_id: self.site_id,
                possession_start_at: self.possession_start_at,
                possession_end_at: self.possession_end_at,
                start_at: self.start_at,
                auction_params: params.into(),
            },
            created_at: self.created_at,
            updated_at: self.updated_at,
            end_at: self.end_at,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct AuctionRound {
    pub id: AuctionRoundId,
    pub auction_id: AuctionId,
    pub round_num: i32,
    #[sqlx(try_from = "SqlxTs")]
    pub start_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub end_at: Timestamp,
    pub eligibility_threshold: f64, // fractional eligibility; 0-1
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

impl AuctionRound {
    pub fn into_response(self) -> payloads::responses::AuctionRound {
        payloads::responses::AuctionRound {
            round_id: self.id,
            round_details: payloads::AuctionRound {
                auction_id: self.auction_id,
                round_num: self.round_num,
                start_at: self.start_at,
                end_at: self.end_at,
                eligibility_threshold: self.eligibility_threshold,
            },
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

pub use payloads::SpaceRound;

#[derive(Debug, Clone, FromRow)]
pub struct UserEligibility {
    pub user_id: UserId,
    pub round_id: AuctionRoundId,
    pub eligibility: f64,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserValues {
    pub user_id: UserId,
    pub space_id: SpaceId,
    pub value: Decimal,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, FromRow)]
pub struct UseProxyBidding {
    pub user_id: UserId,
    pub auction_id: AuctionId,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct AuditLogId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct AuditLog {
    pub id: AuditLogId,
    pub actor_id: Option<UserId>,
    pub action: String,
    pub target_table: Option<String>,
    pub target_id: Option<Uuid>,
    pub details: Option<serde_json::Value>,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
}

/// Create a community and add the creating user as the leader.
pub async fn create_community(
    details: &requests::CreateCommunity,
    user_id: UserId, // initial leader of community
    pool: &PgPool,
) -> Result<Community, StoreError> {
    let user = read_user(pool, &user_id).await?;
    if !user.email_verified {
        return Err(StoreError::UnverifiedEmail);
    }
    if details.name.len() > payloads::requests::COMMUNITY_NAME_MAX_LEN {
        return Err(StoreError::FieldTooLong);
    }
    let mut tx = pool.begin().await?;

    let community = sqlx::query_as::<_, Community>(
        "INSERT INTO communities (
            name,
            new_members_default_active
        ) VALUES ($1, $2) RETURNING *;",
    )
    .bind(&details.name)
    .bind(details.new_members_default_active)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO community_members (community_id, user_id, role)
        VALUES ($1, $2, $3);",
    )
    .bind(community.id)
    .bind(user_id)
    .bind(Role::Leader)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(community)
}

/// Create a new user as would happen during signup.
pub async fn create_user(
    pool: &PgPool,
    username: &str,
    email: &str,
    password_hash: &str,
) -> Result<User, StoreError> {
    if username.len() > payloads::requests::USERNAME_MAX_LEN {
        return Err(StoreError::FieldTooLong);
    }
    if email.len() > payloads::requests::EMAIL_MAX_LEN {
        return Err(StoreError::FieldTooLong);
    }
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (
                username,
                email,
                password_hash
            )
            VALUES ($1, $2, $3)
            RETURNING *;",
    )
    .bind(username)
    .bind(email)
    .bind(password_hash)
    .fetch_one(pool)
    .await?;
    Ok(user)
}

/// Create a new user as would happen during signup.
pub async fn read_user(pool: &PgPool, id: &UserId) -> Result<User, Error> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1;")
        .bind(id)
        .fetch_one(pool)
        .await
}

/// Update fields that are not in the signup process.
pub async fn update_user(pool: &PgPool, user: &User) -> Result<User, Error> {
    sqlx::query_as::<_, User>(
        "UPDATE users
            SET display_name = $1,
                email_verified = $2,
                balance = $3
            WHERE id = $4
            RETURNING *;",
    )
    .bind(&user.display_name)
    .bind(user.email_verified)
    .bind(user.balance)
    .bind(user.id)
    .fetch_one(pool)
    .await
}

pub async fn delete_user(conn: &PgPool, id: &UserId) -> Result<User, Error> {
    sqlx::query_as::<_, User>("DELETE FROM users WHERE id = $1;")
        .bind(id)
        .fetch_one(conn)
        .await
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

pub async fn invite_community_member(
    actor: &ValidatedMember,
    new_member_email: &Option<String>,
    pool: &PgPool,
) -> Result<InviteId, StoreError> {
    if !actor.0.role.is_ge_moderator() {
        return Err(StoreError::RequiresModeratorPermissions);
    }
    let invite = sqlx::query_as::<_, CommunityInvite>(
        "INSERT INTO community_invites (community_id, email)
        VALUES ($1, $2) RETURNING *;",
    )
    .bind(actor.0.community_id)
    .bind(new_member_email)
    .fetch_one(pool)
    .await?;
    Ok(invite.id)
}

pub async fn accept_invite(
    user_id: &UserId,
    invite_id: &payloads::InviteId,
    pool: &PgPool,
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
        return Err(StoreError::InvalidInvite);
    };
    if let Some(invite_email) = invite.email {
        if invite_email != user.email {
            return Err(StoreError::MismatchedInviteEmail);
        }
    }

    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO community_members (community_id, user_id, role)
        VALUES ($1, $2, $3);",
    )
    .bind(invite.community_id)
    .bind(user_id)
    .bind(Role::Member)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM community_invites WHERE id = $1")
        .bind(invite_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(())
}

pub async fn get_communities(
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<Community>, StoreError> {
    Ok(sqlx::query_as::<_, Community>(
        "SELECT b.*
        FROM community_members a
        JOIN communities b ON a.community_id = b.id
        WHERE a.user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_invites(
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<responses::CommunityInvite>, StoreError> {
    let user = read_user(pool, user_id).await?;
    // Need to make sure this user actually owns this email before showing them
    // the invites they've received
    if !user.email_verified {
        return Err(StoreError::UnverifiedEmail);
    }
    Ok(sqlx::query_as::<_, responses::CommunityInvite>(
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

pub async fn get_members(
    actor: &ValidatedMember,
    pool: &PgPool,
) -> Result<Vec<responses::CommunityMember>, StoreError> {
    Ok(sqlx::query_as::<_, responses::CommunityMember>(
        "SELECT
            a.role,
            a.is_active,
            b.username,
            b.display_name
        FROM community_members a
        JOIN users b ON a.user_id = b.id
        WHERE a.community_id = $1",
    )
    .bind(actor.0.community_id)
    .fetch_all(pool)
    .await?)
}

pub async fn set_membership_schedule(
    actor: &ValidatedMember,
    schedule: &[payloads::MembershipSchedule],
    pool: &PgPool,
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
                email
            ) VALUES ($1, $2, $3, $4);",
        )
        .bind(actor.0.community_id)
        .bind(sched_elem.start_at.to_sqlx())
        .bind(sched_elem.end_at.to_sqlx())
        .bind(&sched_elem.email)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

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

#[tracing::instrument(skip(pool), err(level = Level::ERROR))]
/// Update members' is_active status in all communities based on the schedule,
/// if they are present in the schedule.
pub async fn update_is_active_from_schedule(
    pool: &PgPool,
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

    let now = time::now().to_sqlx();
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
            )
            WHERE
                m.user_id = $4
                AND m.community_id = $2",
        )
        .bind(&community_member.email)
        .bind(community_member.community_id)
        .bind(now)
        .bind(community_member.user_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}

pub async fn create_site(
    details: &payloads::Site,
    actor: &ValidatedMember,
    pool: &PgPool,
) -> Result<Site, StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        });
    }
    let mut tx = pool.begin().await?;

    let open_hours_id = match &details.open_hours {
        Some(hours) => Some(create_open_hours(hours, &mut tx).await?),
        None => None,
    };
    let auction_params_id =
        create_auction_params(&details.default_auction_params, &mut tx).await?;

    let site = sqlx::query_as::<_, Site>(
        "INSERT INTO sites (
            community_id,
            name,
            description,
            default_auction_params_id,
            possession_period,
            auction_lead_time,
            proxy_bidding_lead_time,
            open_hours_id,
            auto_schedule,
            timezone
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING *",
    )
    .bind(actor.0.community_id)
    .bind(&details.name)
    .bind(&details.description)
    .bind(auction_params_id)
    .bind(span_to_interval(&details.possession_period)?)
    .bind(span_to_interval(&details.auction_lead_time)?)
    .bind(span_to_interval(&details.proxy_bidding_lead_time)?)
    .bind(open_hours_id)
    .bind(details.auto_schedule)
    .bind(&details.timezone)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(site)
}

async fn create_open_hours(
    open_hours: &payloads::OpenHours,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<OpenHoursId, StoreError> {
    let open_hours_id = sqlx::query_as::<_, OpenHoursId>(
        "INSERT INTO open_hours DEFAULT VALUES RETURNING id",
    )
    .fetch_one(&mut **tx)
    .await?;

    insert_open_hours_weekdays(&open_hours_id, open_hours, tx).await?;
    Ok(open_hours_id)
}

async fn insert_open_hours_weekdays(
    open_hours_id: &OpenHoursId,
    open_hours: &payloads::OpenHours,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), StoreError> {
    for day_of_week in &open_hours.days_of_week {
        sqlx::query(
            "INSERT INTO open_hours_weekday (
                open_hours_id,
                day_of_week,
                open_time,
                close_time
            ) VALUES ($1, $2, $3, $4)",
        )
        .bind(open_hours_id)
        .bind(day_of_week.day_of_week)
        .bind(day_of_week.open_time.to_sqlx())
        .bind(day_of_week.close_time.to_sqlx())
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn create_auction_params(
    params: &payloads::AuctionParams,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<AuctionParamsId, StoreError> {
    Ok(sqlx::query_as::<_, AuctionParamsId>(
        "INSERT INTO auction_params (
                round_duration,
                bid_increment,
                activity_rule_params
            ) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(span_to_interval(&params.round_duration)?)
    .bind(params.bid_increment)
    .bind(Json(params.activity_rule_params.clone()))
    .fetch_one(&mut **tx)
    .await?)
}

pub async fn get_site_community_id(
    site_id: &SiteId,
    pool: &PgPool,
) -> Result<CommunityId, StoreError> {
    Ok(sqlx::query_as::<_, CommunityId>(
        "SELECT community_id FROM sites WHERE id = $1",
    )
    .bind(site_id)
    .fetch_one(pool)
    .await?)
}

pub async fn get_site(
    site_id: &SiteId,
    pool: &PgPool,
) -> Result<payloads::responses::Site, StoreError> {
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(site_id)
        .fetch_one(pool)
        .await?;
    let open_hours = match &site.open_hours_id {
        Some(open_hours_id) => {
            let days_of_week = sqlx::query_as::<_, payloads::OpenHoursWeekday>(
                "SELECT * FROM open_hours_weekday WHERE open_hours_id = $1",
            )
            .bind(open_hours_id)
            .fetch_all(pool)
            .await?;
            Some(payloads::OpenHours { days_of_week })
        }
        None => None,
    };
    let default_auction_params = sqlx::query_as::<_, AuctionParams>(
        "SELECT * FROM auction_params WHERE id = $1",
    )
    .bind(site.default_auction_params_id)
    .fetch_one(pool)
    .await?;
    let site_details = payloads::Site {
        community_id: site.community_id,
        name: site.name,
        description: site.description,
        default_auction_params: default_auction_params.into(),
        possession_period: site.possession_period,
        auction_lead_time: site.auction_lead_time,
        proxy_bidding_lead_time: site.proxy_bidding_lead_time,
        open_hours,
        auto_schedule: site.auto_schedule,
        timezone: site.timezone,
    };
    Ok(payloads::responses::Site {
        site_id: site.id,
        site_details,
        created_at: site.created_at,
        updated_at: site.updated_at,
    })
}

pub async fn update_site(
    update_site: &payloads::requests::UpdateSite,
    actor: &ValidatedMember,
    pool: &PgPool,
) -> Result<responses::Site, StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    let details = &update_site.site_details;

    let existing_site =
        sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
            .bind(update_site.site_id)
            .fetch_one(pool)
            .await?;

    let mut tx = pool.begin().await?;

    let new_open_hours_id = update_open_hours(
        &existing_site.open_hours_id,
        &details.open_hours,
        &mut tx,
    )
    .await?;

    let new_auction_params_id =
        create_auction_params(&details.default_auction_params, &mut tx).await?;

    sqlx::query(
        "UPDATE sites SET
            name = $1,
            description = $2,
            default_auction_params_id = $3,
            possession_period = $4,
            auction_lead_time = $5,
            proxy_bidding_lead_time = $6,
            open_hours_id = $7,
            auto_schedule = $8,
            timezone = $9
        WHERE id = $10",
    )
    .bind(&details.name)
    .bind(&details.description)
    .bind(new_auction_params_id)
    .bind(span_to_interval(&details.possession_period)?)
    .bind(span_to_interval(&details.auction_lead_time)?)
    .bind(span_to_interval(&details.proxy_bidding_lead_time)?)
    .bind(new_open_hours_id)
    .bind(details.auto_schedule)
    .bind(&details.timezone)
    .bind(existing_site.id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let site = get_site(&existing_site.id, pool).await?;

    cleanup_unused_auction_params(pool).await;
    Ok(site)
}

async fn cleanup_unused_auction_params(pool: &PgPool) {
    if let Err(e) = sqlx::query(
        "DELETE FROM auction_params p
        WHERE NOT EXISTS (
            SELECT FROM sites
            WHERE default_auction_params_id = p.id
        ) AND NOT EXISTS (
            SELECT FROM auctions
            WHERE auction_params_id = p.id
        );",
    )
    .execute(pool)
    .await
    .context("cleanup unused auction params")
    {
        tracing::error!("{e:#}");
    }
}

/// Update an existing open hours (if it exists), returning the id.
async fn update_open_hours(
    // existing open hours
    open_hours_id: &Option<OpenHoursId>,
    new_open_hours: &Option<payloads::OpenHours>,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Option<OpenHoursId>, StoreError> {
    // delete the existing open hours
    sqlx::query("DELETE FROM open_hours WHERE id = $1;")
        .bind(open_hours_id)
        .execute(&mut **tx)
        .await?;

    // add new open hours
    match new_open_hours {
        Some(new_open_hours) => {
            Ok(Some(create_open_hours(new_open_hours, tx).await?))
        }
        None => Ok(None),
    }
}

pub async fn delete_site(
    site_id: &payloads::SiteId,
    actor: &ValidatedMember,
    pool: &PgPool,
) -> Result<(), StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    let existing_site =
        sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
            .bind(site_id)
            .fetch_one(pool)
            .await?;

    let mut tx = pool.begin().await?;

    // remove any remaining open hours
    update_open_hours(&existing_site.open_hours_id, &None, &mut tx).await?;

    sqlx::query("DELETE FROM sites WHERE id = $1")
        .bind(site_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    cleanup_unused_auction_params(pool).await;

    Ok(())
}

/// Get a space and validate that the user has the required permission level in the site's community.
/// Returns both the space and the validated member if successful.
async fn get_validated_space(
    space_id: &SpaceId,
    user_id: &UserId,
    required_permission: PermissionLevel,
    pool: &PgPool,
) -> Result<(Space, ValidatedMember), StoreError> {
    let space =
        sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = $1")
            .bind(space_id)
            .fetch_one(pool)
            .await?;

    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(space.site_id)
        .fetch_one(pool)
        .await?;

    let actor = get_validated_member(user_id, &site.community_id, pool).await?;

    if !required_permission.validate(actor.0.role) {
        return Err(StoreError::InsufficientPermissions {
            required: required_permission,
        });
    }

    Ok((space, actor))
}

pub async fn create_space(
    details: &payloads::Space,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Space, StoreError> {
    // Get the site and validate user permissions
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(details.site_id)
        .fetch_one(pool)
        .await?;

    let actor = get_validated_member(user_id, &site.community_id, pool).await?;

    if !PermissionLevel::Coleader.validate(actor.0.role) {
        return Err(StoreError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        });
    }

    let space = sqlx::query_as::<_, Space>(
        "INSERT INTO spaces (
            site_id,
            name,
            description,
            eligibility_points,
            is_available
        ) VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(details.site_id)
    .bind(&details.name)
    .bind(&details.description)
    .bind(details.eligibility_points)
    .bind(details.is_available)
    .fetch_one(pool)
    .await?;

    Ok(space)
}

pub async fn get_space(
    space_id: &SpaceId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::Space, StoreError> {
    let (space, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    Ok(space.into())
}

pub async fn update_space(
    space_id: &SpaceId,
    details: &payloads::Space,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::Space, StoreError> {
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Coleader, pool)
            .await?;

    let updated_space = sqlx::query_as::<_, Space>(
        "UPDATE spaces SET
            name = $1,
            description = $2,
            eligibility_points = $3,
            is_available = $4
        WHERE id = $5
        RETURNING *",
    )
    .bind(&details.name)
    .bind(&details.description)
    .bind(details.eligibility_points)
    .bind(details.is_available)
    .bind(space_id)
    .fetch_one(pool)
    .await?;

    Ok(updated_space.into())
}

pub async fn delete_space(
    space_id: &SpaceId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Coleader, pool)
            .await?;

    sqlx::query("DELETE FROM spaces WHERE id = $1")
        .bind(space_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn list_spaces(
    site_id: &SiteId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::Space>, StoreError> {
    // Get the site and validate user permissions
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(site_id)
        .fetch_one(pool)
        .await?;

    let _ = get_validated_member(user_id, &site.community_id, pool).await?;

    let spaces = sqlx::query_as::<_, Space>(
        "SELECT * FROM spaces WHERE site_id = $1 ORDER BY name",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;

    Ok(spaces.into_iter().map(Into::into).collect())
}

/// Get an auction and validate that the user has the required permission level in the site's community.
/// Returns both the auction and the validated member if successful.
async fn get_validated_auction(
    auction_id: &AuctionId,
    user_id: &UserId,
    required_permission: PermissionLevel,
    pool: &PgPool,
) -> Result<(Auction, ValidatedMember), StoreError> {
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_id)
            .fetch_one(pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => StoreError::AuctionNotFound,
                e => StoreError::Database(e),
            })?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let actor = get_validated_member(user_id, &community_id, pool).await?;

    if !required_permission.validate(actor.0.role) {
        return Err(StoreError::InsufficientPermissions {
            required: required_permission,
        });
    }

    Ok((auction, actor))
}

pub async fn create_auction(
    details: &payloads::Auction,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::AuctionId, StoreError> {
    // Get the site and validate user permissions
    let community_id = get_site_community_id(&details.site_id, pool).await?;
    let actor = get_validated_member(user_id, &community_id, pool).await?;

    if !PermissionLevel::Coleader.validate(actor.0.role) {
        return Err(StoreError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        });
    }

    let mut tx = pool.begin().await?;

    // Create auction params first
    let auction_params_id =
        create_auction_params(&details.auction_params, &mut tx).await?;

    let auction_id = sqlx::query_as::<_, Auction>(
        "INSERT INTO auctions (
            site_id,
            possession_start_at,
            possession_end_at,
            start_at,
            auction_params_id
        ) VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(details.site_id)
    .bind(details.possession_start_at.to_sqlx())
    .bind(details.possession_end_at.to_sqlx())
    .bind(details.start_at.to_sqlx())
    .bind(auction_params_id)
    .fetch_one(&mut *tx)
    .await?
    .id;

    tx.commit().await?;

    Ok(auction_id)
}

pub async fn read_auction(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::Auction, StoreError> {
    let (auction, _) = get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Member,
        pool,
    )
    .await?;

    let auction_params = sqlx::query_as::<_, AuctionParams>(
        "SELECT * FROM auction_params WHERE id = $1",
    )
    .bind(&auction.auction_params_id)
    .fetch_one(pool)
    .await?;

    Ok(auction.with_params(auction_params))
}

pub async fn delete_auction(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let (_, _) = get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Coleader,
        pool,
    )
    .await?;

    sqlx::query("DELETE FROM auctions WHERE id = $1")
        .bind(auction_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn list_auctions(
    site_id: &SiteId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::Auction>, StoreError> {
    // Get the site and validate user permissions
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(site_id)
        .fetch_one(pool)
        .await?;

    let _ = get_validated_member(user_id, &site.community_id, pool).await?;

    let auctions = sqlx::query_as::<_, Auction>(
        "SELECT * FROM auctions WHERE site_id = $1 ORDER BY start_at DESC",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;

    // Convert each auction with its params
    let mut responses = Vec::new();
    for auction in auctions {
        let auction_params = sqlx::query_as::<_, AuctionParams>(
            "SELECT * FROM auction_params WHERE id = $1",
        )
        .bind(&auction.auction_params_id)
        .fetch_one(pool)
        .await?;

        responses.push(auction.with_params(auction_params));
    }

    Ok(responses)
}

pub async fn get_auction_round(
    round_id: &payloads::AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::AuctionRound, StoreError> {
    let round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await?;

    // Validate user has access to this auction's community
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(round.auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    Ok(round.into_response())
}

pub async fn list_auction_rounds(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::AuctionRound>, StoreError> {
    // First validate user has access to this auction's community
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    let rounds = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE auction_id = $1 ORDER BY round_num",
    )
    .bind(auction_id)
    .fetch_all(pool)
    .await?;

    Ok(rounds.into_iter().map(|r| r.into_response()).collect())
}

pub async fn get_space_round(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<SpaceRound, StoreError> {
    // Get the space to validate user permissions
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    let space_round = sqlx::query_as::<_, SpaceRound>(
        "SELECT space_id, round_id, winning_user_id, value FROM space_rounds WHERE space_id = $1 AND round_id = $2",
    )
    .bind(space_id)
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::SpaceRoundNotFound,
        e => StoreError::Database(e),
    })?;

    Ok(space_round)
}

pub async fn list_space_rounds(
    space_id: &SpaceId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<SpaceRound>, StoreError> {
    // Get the space to validate user permissions
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    let space_rounds = sqlx::query_as::<_, SpaceRound>(
        "SELECT space_id, round_id, winning_user_id, value FROM space_rounds WHERE space_id = $1 ORDER BY (
            SELECT round_num FROM auction_rounds WHERE id = round_id
        )",
    )
    .bind(space_id)
    .fetch_all(pool)
    .await?;

    Ok(space_rounds)
}

pub async fn create_bid(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    // Get the space to validate user permissions
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    // Verify the round exists and is ongoing
    let round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => StoreError::Database(e),
    })?;

    let now = time::now();
    if now < round.start_at {
        return Err(StoreError::RoundNotStarted);
    }
    if now >= round.end_at {
        return Err(StoreError::RoundEnded);
    }

    // Create the bid
    sqlx::query(
        "INSERT INTO bids (space_id, round_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(space_id)
    .bind(round_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_bid(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Bid, StoreError> {
    // Get the space to validate user permissions
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    let bid = sqlx::query_as::<_, Bid>(
        "SELECT * FROM bids WHERE space_id = $1 AND round_id = $2 AND user_id = $3",
    )
    .bind(space_id)
    .bind(round_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::BidNotFound,
        e => StoreError::Database(e),
    })?;

    Ok(bid)
}

pub async fn list_bids(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<Bid>, StoreError> {
    // Get the space to validate user permissions
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    let bids = sqlx::query_as::<_, Bid>(
        "SELECT * FROM bids WHERE space_id = $1 AND round_id = $2 AND user_id = $3",
    )
    .bind(space_id)
    .bind(round_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(bids)
}

pub async fn delete_bid(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    // Get the space to validate user permissions
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    // Verify the round exists and is ongoing
    let round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => StoreError::Database(e),
    })?;

    let now = dbg!(time::now());
    dbg!(jiff::Timestamp::now());
    if now < dbg!(round.start_at) {
        return Err(StoreError::RoundNotStarted);
    }
    if now >= dbg!(round.end_at) {
        return Err(StoreError::RoundEnded);
    }

    // Delete the bid
    sqlx::query(
        "DELETE FROM bids WHERE space_id = $1 AND round_id = $2 AND user_id = $3",
    )
    .bind(space_id)
    .bind(round_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Email not yet verified")]
    UnverifiedEmail,
    #[error("Moderator permissions required")]
    RequiresModeratorPermissions,
    #[error("Coleader permissions required")]
    RequiresColeaderPermissions,
    #[error("Mismatched invite email")]
    MismatchedInviteEmail,
    #[error("Field too long")]
    FieldTooLong,
    #[error("Invalid invite")]
    InvalidInvite,
    #[error("Member not found")]
    MemberNotFound,
    #[error("Span too large")]
    SpanTooLarge(Box<Span>),
    #[error("Unique constraint violation")]
    NotUnique(#[source] sqlx::Error),
    #[error("Row not found")]
    RowNotFound(#[source] sqlx::Error),
    #[error("Database error")]
    Database(#[source] sqlx::Error),
    #[error("Unexpected error")]
    UnexpectedError(#[from] anyhow::Error),
    #[error("Insufficient permissions. Required: {required:?}")]
    InsufficientPermissions { required: PermissionLevel },
    #[error("Auction not found")]
    AuctionNotFound,
    #[error("Space round not found")]
    SpaceRoundNotFound,
    #[error("Bid not found")]
    BidNotFound,
    #[error("Round has ended")]
    RoundEnded,
    #[error("Auction round not found")]
    AuctionRoundNotFound,
    #[error("Round has not started yet")]
    RoundNotStarted,
}

impl From<sqlx::Error> for StoreError {
    fn from(e: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db_err) = &e {
            if db_err.code().as_deref() == Some("23505") {
                return StoreError::NotUnique(e);
            }
        } else if matches!(e, sqlx::Error::RowNotFound) {
            return StoreError::RowNotFound(e);
        }
        StoreError::Database(e)
    }
}

/// Balance a jiff::Span according to the units that can be stored in a Postgres
/// interval.
pub fn span_to_interval(span: &Span) -> Result<PgInterval, StoreError> {
    span_to_interval_opt(span).ok_or(StoreError::SpanTooLarge(Box::new(*span)))
}

fn span_to_interval_opt(span: &Span) -> Option<PgInterval> {
    let microseconds = span
        .get_milliseconds()
        .checked_add(span.get_milliseconds().checked_mul(1_000)?)?
        .checked_add(span.get_seconds().checked_mul(1_000_000)?)?
        .checked_add(span.get_minutes().checked_mul(60 * 1_000_000)?)?
        .checked_add(
            (span.get_hours() as i64).checked_mul(60 * 60 * 1_000_000)?,
        )?;
    let days = span
        .get_days()
        .checked_add(span.get_weeks().checked_mul(7)?)?;
    let months = span
        .get_months()
        .checked_add((span.get_years() as i32).checked_mul(12)?)?;
    Some(PgInterval {
        microseconds,
        days,
        months,
    })
}
