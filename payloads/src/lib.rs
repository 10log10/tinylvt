use jiff::{Span, Timestamp, civil::Time};
#[cfg(feature = "use-sqlx")]
use jiff_sqlx::{Span as SqlxSpan, Timestamp as SqlxTs};
use rust_decimal::Decimal;
#[cfg(feature = "use-sqlx")]
use sqlx::{FromRow, Type};

/// Maximum allowed size for site images (1MB)
pub const MAX_IMAGE_SIZE: usize = 1_048_576;

/// Maximum allowed length for site descriptions (10,000 characters)
pub const MAX_SITE_DESCRIPTION_LENGTH: usize = 10_000;

/// Maximum allowed length for community descriptions (10,000 characters)
pub const MAX_COMMUNITY_DESCRIPTION_LENGTH: usize = 10_000;

/// Maximum allowed length for space descriptions (500 characters)
pub const MAX_SPACE_DESCRIPTION_LENGTH: usize = 500;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type))]
#[cfg_attr(
    feature = "use-sqlx",
    sqlx(type_name = "role", rename_all = "lowercase")
)]
pub enum Role {
    Member,
    Moderator,
    Coleader,
    Leader,
}

impl std::str::FromStr for Role {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Member" => Ok(Role::Member),
            "Moderator" => Ok(Role::Moderator),
            "Coleader" => Ok(Role::Coleader),
            "Leader" => Ok(Role::Leader),
            _ => Err(()),
        }
    }
}

impl Role {
    pub fn is_ge_moderator(&self) -> bool {
        matches!(self, Self::Moderator | Self::Coleader | Self::Leader)
    }

    pub fn is_ge_coleader(&self) -> bool {
        matches!(self, Self::Coleader | Self::Leader)
    }

    pub fn is_leader(&self) -> bool {
        matches!(self, Self::Leader)
    }

    /// Check if actor can remove target based on role hierarchy.
    ///
    /// Permission rules:
    /// - Leader can remove anyone except themselves
    /// - Coleader can remove members or moderators
    /// - Moderator can only remove members
    /// - Member cannot remove anyone
    pub fn can_remove_role(&self, target_role: &Role) -> bool {
        match self {
            Role::Leader => !target_role.is_leader(),
            Role::Coleader => {
                matches!(target_role, Role::Member | Role::Moderator)
            }
            Role::Moderator => matches!(target_role, Role::Member),
            Role::Member => false,
        }
    }

    /// Check if actor can change target's role to new_role.
    ///
    /// Permission rules:
    /// - Leader can change anyone (except leader) to any role (except leader)
    /// - Coleader can promote members/moderators to moderator or coleader
    /// - Coleader can demote moderators to member
    /// - Coleader cannot demote other coleaders
    /// - Moderator and Member cannot change roles
    /// - No one can change leader's role or promote to leader
    pub fn can_change_role(&self, target_role: &Role, new_role: &Role) -> bool {
        // Cannot change leader's role, promote to leader, or change to same
        // role
        if target_role.is_leader()
            || new_role.is_leader()
            || target_role == new_role
        {
            return false;
        }

        match self {
            Role::Leader => true,
            Role::Coleader => {
                // Coleader cannot demote other coleaders, but can promote new
                // coleaders
                !target_role.is_ge_coleader()
            }
            Role::Moderator | Role::Member => false,
        }
    }

    /// Moderators can change per-member attributes like active status and
    /// credit limits.
    pub fn can_edit_credit_limit(&self) -> bool {
        self.is_ge_moderator()
    }

    pub fn can_change_active_status(&self) -> bool {
        self.is_ge_moderator()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionLevel {
    /// Any member of the community
    Member,
    /// Moderator or higher (moderator, coleader, leader)
    Moderator,
    /// Coleader or higher (coleader, leader)
    Coleader,
    /// Only the leader
    Leader,
}

impl PermissionLevel {
    pub fn validate(&self, role: Role) -> bool {
        match self {
            Self::Member => true,
            Self::Moderator => role.is_ge_moderator(),
            Self::Coleader => role.is_ge_coleader(),
            Self::Leader => role.is_leader(),
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(FromRow))]
pub struct MembershipSchedule {
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
    pub start_at: Timestamp,
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
    pub end_at: Timestamp,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(FromRow))]
pub struct AuctionParams {
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxSpan"))]
    pub round_duration: Span,
    pub bid_increment: Decimal,
    pub activity_rule_params: ActivityRuleParams,
}

impl PartialEq for AuctionParams {
    fn eq(&self, other: &Self) -> bool {
        self.round_duration.fieldwise() == other.round_duration.fieldwise()
            && self.bid_increment == other.bid_increment
            && self.activity_rule_params == other.activity_rule_params
    }
}

/// Contents of the `activity_rule_params` JSONB column of `auction_params`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityRuleParams {
    /// Maps the round number to a 0-1 value indicating fraction of eligibility
    /// required.
    pub eligibility_progression: Vec<(i32, f64)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenHours {
    pub days_of_week: Vec<OpenHoursWeekday>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(FromRow))]
pub struct OpenHoursWeekday {
    pub day_of_week: i16,
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "jiff_sqlx::Time"))]
    pub open_time: Time,
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "jiff_sqlx::Time"))]
    pub close_time: Time,
}

/// An empty schedule can be used to delete the schedule entirely.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Site {
    pub community_id: CommunityId,
    pub name: String,
    pub description: Option<String>,
    pub default_auction_params: AuctionParams,
    pub possession_period: Span,
    pub auction_lead_time: Span,
    pub proxy_bidding_lead_time: Span,
    pub open_hours: Option<OpenHours>,
    pub auto_schedule: bool,
    pub timezone: Option<String>,
    pub site_image_id: Option<SiteImageId>,
}

impl PartialEq for Site {
    fn eq(&self, other: &Self) -> bool {
        self.community_id == other.community_id
            && self.name == other.name
            && self.description == other.description
            && self.default_auction_params == other.default_auction_params
            && self.possession_period.fieldwise()
                == other.possession_period.fieldwise()
            && self.auction_lead_time.fieldwise()
                == other.auction_lead_time.fieldwise()
            && self.proxy_bidding_lead_time.fieldwise()
                == other.proxy_bidding_lead_time.fieldwise()
            && self.open_hours == other.open_hours
            && self.auto_schedule == other.auto_schedule
            && self.timezone == other.timezone
            && self.site_image_id == other.site_image_id
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Space {
    pub site_id: SiteId,
    pub name: String,
    pub description: Option<String>,
    pub eligibility_points: f64,
    pub is_available: bool,
    pub site_image_id: Option<SiteImageId>,
}

/// An auction for a site's possession period
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Auction {
    pub site_id: SiteId,
    /// The possession period is the only time that is localized to the
    /// site's timezone (if the site has a timezone). This reflects how
    /// possession is for a physical space at the site's location, and that
    /// possession timing is coordinated between community members who need a
    /// common timezone to reference.
    pub possession_start_at: Timestamp,
    pub possession_end_at: Timestamp,
    /// Auction times are localized to the user's current timezone, since it
    /// should be clear to the user when auctions take place relative to now.
    /// They know when the auction will start, even if the site they're bidding
    /// for is located in a different timezone, with a possession period in the
    /// future.
    pub start_at: Timestamp,
    pub auction_params: AuctionParams,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct AuctionRoundId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuctionRound {
    pub auction_id: AuctionId,
    pub round_num: i32,
    pub start_at: Timestamp,
    pub end_at: Timestamp,
    pub eligibility_threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoundSpaceResult {
    pub space_id: SpaceId,
    pub round_id: AuctionRoundId,
    pub winner: responses::UserIdentity,
    pub value: rust_decimal::Decimal,
}

/// Visible only to the creator
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
pub struct Bid {
    pub space_id: SpaceId,
    pub round_id: AuctionRoundId,
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "jiff_sqlx::Timestamp"))]
    pub created_at: Timestamp,
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "jiff_sqlx::Timestamp"))]
    pub updated_at: Timestamp,
}

// Currency system types

/// Currency mode enum for UI selection and mode identification
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "use-sqlx",
    sqlx(type_name = "currency_mode", rename_all = "snake_case")
)]
pub enum CurrencyMode {
    PointsAllocation,
    DistributedClearing,
    DeferredPayment,
    PrepaidCredits,
}

/// Points allocation configuration
/// Members are issued points by the treasury on a regular schedule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointsAllocationConfig {
    /// Amount issued per allowance period
    pub allowance_amount: rust_decimal::Decimal,
    /// Period between allowances
    pub allowance_period: jiff::Span,
    /// Starting point for automated issuance
    pub allowance_start: jiff::Timestamp,
}

impl PartialEq for PointsAllocationConfig {
    fn eq(&self, other: &Self) -> bool {
        self.allowance_amount == other.allowance_amount
            && self.allowance_period.fieldwise()
                == other.allowance_period.fieldwise()
            && self.allowance_start == other.allowance_start
    }
}

impl Eq for PointsAllocationConfig {}

impl PointsAllocationConfig {
    /// Credit limit is always 0 for points allocation
    pub fn credit_limit(&self) -> rust_decimal::Decimal {
        rust_decimal::Decimal::ZERO
    }

    /// Debts are never callable for points allocation
    pub fn debts_callable(&self) -> bool {
        false
    }
}

/// IOU-based currency configuration
/// Used by both DistributedClearing and DeferredPayment modes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IOUConfig {
    /// Optional default credit limit for members
    pub default_credit_limit: Option<rust_decimal::Decimal>,
    /// Whether debts carry promise of settlement
    pub debts_callable: bool,
}

/// Prepaid credits configuration
/// Members purchase credits from treasury up front
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrepaidCreditsConfig {
    /// Whether debts carry promise of settlement
    pub debts_callable: bool,
}

impl PrepaidCreditsConfig {
    /// Credit limit is always 0 for prepaid credits
    pub fn credit_limit(&self) -> rust_decimal::Decimal {
        rust_decimal::Decimal::ZERO
    }
}

/// Mode-specific currency configuration enum
/// Makes invalid currency configurations unrepresentable
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurrencyModeConfig {
    PointsAllocation(Box<PointsAllocationConfig>),
    DistributedClearing(IOUConfig),
    DeferredPayment(IOUConfig),
    PrepaidCredits(PrepaidCreditsConfig),
}

impl CurrencyModeConfig {
    /// Get the currency mode for this configuration
    pub fn mode(&self) -> CurrencyMode {
        match self {
            CurrencyModeConfig::PointsAllocation(_) => {
                CurrencyMode::PointsAllocation
            }
            CurrencyModeConfig::DistributedClearing(_) => {
                CurrencyMode::DistributedClearing
            }
            CurrencyModeConfig::DeferredPayment(_) => {
                CurrencyMode::DeferredPayment
            }
            CurrencyModeConfig::PrepaidCredits(_) => {
                CurrencyMode::PrepaidCredits
            }
        }
    }

    /// Get the default credit limit for this configuration
    pub fn default_credit_limit(&self) -> Option<rust_decimal::Decimal> {
        match self {
            CurrencyModeConfig::PointsAllocation(cfg) => {
                Some(cfg.credit_limit())
            }
            CurrencyModeConfig::DistributedClearing(cfg) => {
                cfg.default_credit_limit
            }
            CurrencyModeConfig::DeferredPayment(cfg) => {
                cfg.default_credit_limit
            }
            CurrencyModeConfig::PrepaidCredits(cfg) => Some(cfg.credit_limit()),
        }
    }

    /// Get whether debts are callable for this configuration
    pub fn debts_callable(&self) -> bool {
        match self {
            CurrencyModeConfig::PointsAllocation(cfg) => cfg.debts_callable(),
            CurrencyModeConfig::DistributedClearing(cfg) => cfg.debts_callable,
            CurrencyModeConfig::DeferredPayment(cfg) => cfg.debts_callable,
            CurrencyModeConfig::PrepaidCredits(cfg) => cfg.debts_callable,
        }
    }

    /// Set the default credit limit (only for IOU-based modes)
    /// Returns None if this is not an IOU-based mode
    pub fn set_default_credit_limit(
        &self,
        limit: Option<rust_decimal::Decimal>,
    ) -> Option<Self> {
        match self {
            CurrencyModeConfig::DistributedClearing(config) => {
                Some(CurrencyModeConfig::DistributedClearing(IOUConfig {
                    default_credit_limit: limit,
                    debts_callable: config.debts_callable,
                }))
            }
            CurrencyModeConfig::DeferredPayment(config) => {
                Some(CurrencyModeConfig::DeferredPayment(IOUConfig {
                    default_credit_limit: limit,
                    debts_callable: config.debts_callable,
                }))
            }
            _ => None,
        }
    }

    /// Set debts callable flag
    /// Returns None if the mode doesn't support this setting
    pub fn set_debts_callable(&self, callable: bool) -> Option<Self> {
        match self {
            CurrencyModeConfig::DistributedClearing(config) => {
                Some(CurrencyModeConfig::DistributedClearing(IOUConfig {
                    default_credit_limit: config.default_credit_limit,
                    debts_callable: callable,
                }))
            }
            CurrencyModeConfig::DeferredPayment(config) => {
                Some(CurrencyModeConfig::DeferredPayment(IOUConfig {
                    default_credit_limit: config.default_credit_limit,
                    debts_callable: callable,
                }))
            }
            CurrencyModeConfig::PrepaidCredits(_) => {
                Some(CurrencyModeConfig::PrepaidCredits(PrepaidCreditsConfig {
                    debts_callable: callable,
                }))
            }
            _ => None,
        }
    }
}

/// Complete currency settings for a community
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrencySettings {
    pub mode_config: CurrencyModeConfig,
    pub name: String,
    pub symbol: String,
    pub minor_units: i16,
    pub balances_visible_to_members: bool,
    pub new_members_default_active: bool,
}

impl CurrencySettings {
    /// Get the currency mode
    pub fn mode(&self) -> CurrencyMode {
        self.mode_config.mode()
    }

    /// Get the default credit limit
    pub fn default_credit_limit(&self) -> Option<rust_decimal::Decimal> {
        self.mode_config.default_credit_limit()
    }

    /// Get whether debts are callable
    pub fn debts_callable(&self) -> bool {
        self.mode_config.debts_callable()
    }

    /// Format an amount using the currency symbol and minor units
    pub fn format_amount(&self, amount: rust_decimal::Decimal) -> String {
        // Round to the appropriate number of decimal places
        let rounded = amount.round_dp_with_strategy(
            self.minor_units as u32,
            rust_decimal::RoundingStrategy::MidpointNearestEven,
        );

        // Format with fixed decimal places (e.g., "$10.00" not "$10")
        let amount_str =
            format!("{:.prec$}", rounded, prec = self.minor_units as usize);
        format!("{}{}", self.symbol, amount_str)
    }
}

/// Database-level account owner type enum (used only for DB serialization)
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type))]
#[cfg_attr(
    feature = "use-sqlx",
    sqlx(type_name = "account_owner_type", rename_all = "snake_case")
)]
pub enum AccountOwnerType {
    MemberMain,
    CommunityTreasury,
}

/// Proper sum type for account ownership that makes invalid states
/// unrepresentable
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountOwner {
    Member(UserId),
    Treasury,
}

impl AccountOwner {
    pub fn owner_type(&self) -> AccountOwnerType {
        match self {
            AccountOwner::Member(_) => AccountOwnerType::MemberMain,
            AccountOwner::Treasury => AccountOwnerType::CommunityTreasury,
        }
    }

    pub fn owner_id(&self) -> Option<UserId> {
        match self {
            AccountOwner::Member(user_id) => Some(*user_id),
            AccountOwner::Treasury => None,
        }
    }

    pub fn from_parts(
        owner_type: AccountOwnerType,
        owner_id: Option<UserId>,
    ) -> Option<Self> {
        match (owner_type, owner_id) {
            (AccountOwnerType::MemberMain, Some(user_id)) => {
                Some(AccountOwner::Member(user_id))
            }
            (AccountOwnerType::CommunityTreasury, None) => {
                Some(AccountOwner::Treasury)
            }
            _ => None,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type))]
#[cfg_attr(
    feature = "use-sqlx",
    sqlx(type_name = "entry_type", rename_all = "snake_case")
)]
pub enum EntryType {
    // Treasury credit operations
    IssuanceGrantSingle,
    IssuanceGrantBulk,
    CreditPurchase,
    DistributionCorrection,
    DebtSettlement,
    // Reset all balances in the community
    BalanceReset,
    // Auction settlement to treasury or active members depending on mode
    AuctionSettlement,
    // Member-member transfer
    Transfer,
    // Transfer from orphaned account (member who left) to treasury or
    // active members
    OrphanedAccountTransfer,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct AccountId(pub Uuid);

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct JournalEntryId(pub Uuid);

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct JournalLineId(pub Uuid);

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct IdempotencyKey(pub Uuid);

/// Treasury operation recipient specification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreasuryRecipient {
    /// Single member receives credit
    SingleMember(UserId),
    /// All active members receive equal credit
    AllActiveMembers,
}

/// Result of a treasury credit operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreasuryOperationResult {
    /// Number of members who received credits
    pub recipient_count: usize,
    /// Total amount debited from treasury
    pub total_amount: rust_decimal::Decimal,
}

/// Domain-level Account with type-safe ownership
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub community_id: CommunityId,
    pub owner: AccountOwner,
    pub created_at: Timestamp,
    pub balance_cached: Decimal,
    pub credit_limit_override: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(FromRow))]
pub struct JournalEntry {
    pub id: JournalEntryId,
    pub community_id: CommunityId,
    pub entry_type: EntryType,
    pub idempotency_key: IdempotencyKey,
    pub auction_id: Option<AuctionId>,
    pub initiated_by_id: Option<UserId>,
    pub note: Option<String>,
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(FromRow))]
pub struct JournalLine {
    pub id: JournalLineId,
    pub entry_id: JournalEntryId,
    pub account_id: AccountId,
    pub amount: Decimal,
}

pub mod requests;
pub mod responses;

use derive_more::Display;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Helper types for optional jiff types with sqlx
#[cfg(feature = "use-sqlx")]
#[derive(sqlx::Type)]
#[sqlx(transparent)]
pub struct OptionalSpan(pub Option<SqlxSpan>);

#[cfg(feature = "use-sqlx")]
impl From<OptionalSpan> for Option<jiff::Span> {
    fn from(x: OptionalSpan) -> Option<jiff::Span> {
        x.0.map(|x| x.to_jiff())
    }
}

#[cfg(feature = "use-sqlx")]
#[derive(sqlx::Type)]
#[sqlx(transparent)]
pub struct OptionalTimestamp(pub Option<SqlxTs>);

#[cfg(feature = "use-sqlx")]
impl From<OptionalTimestamp> for Option<jiff::Timestamp> {
    fn from(x: OptionalTimestamp) -> Option<jiff::Timestamp> {
        x.0.map(|x| x.to_jiff())
    }
}

/// Id type wrappers help ensure we don't mix up ids for different tables.
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct UserId(pub Uuid);

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct CommunityId(pub Uuid);

impl std::str::FromStr for CommunityId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        uuid::Uuid::parse_str(s).map(CommunityId)
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct InviteId(pub Uuid);

impl std::str::FromStr for InviteId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        uuid::Uuid::parse_str(s).map(InviteId)
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct TokenId(pub Uuid);

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct SiteId(pub Uuid);

impl std::str::FromStr for SiteId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        uuid::Uuid::parse_str(s).map(SiteId)
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct SpaceId(pub Uuid);
impl std::str::FromStr for SpaceId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        uuid::Uuid::parse_str(s).map(SpaceId)
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct AuctionId(pub Uuid);

impl std::str::FromStr for AuctionId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        uuid::Uuid::parse_str(s).map(AuctionId)
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct SiteImageId(pub Uuid);

pub mod api_client;

pub use api_client::{APIClient, ClientError, ok_body, ok_empty};
