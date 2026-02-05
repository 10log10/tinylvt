use jiff::{Span, Timestamp, civil::Time};
#[cfg(feature = "use-sqlx")]
use jiff_sqlx::{Span as SqlxSpan, Timestamp as SqlxTs};
use rust_decimal::Decimal;
#[cfg(feature = "use-sqlx")]
use sqlx::{FromRow, Type};

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
/// Members purchase credits from treasury upfront
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

pub mod requests {
    use crate::{CommunityId, CurrencySettings};
    use rust_decimal::Decimal;
    use serde::{Deserialize, Serialize};

    pub const EMAIL_MAX_LEN: usize = 255;
    pub const USERNAME_MAX_LEN: usize = 50;
    pub const DISPLAY_NAME_MAX_LEN: usize = 255;

    #[derive(Serialize, Deserialize)]
    pub struct LoginCredentials {
        pub username: String,
        pub password: String,
    }

    #[derive(Serialize, Deserialize)]
    pub struct CreateAccount {
        pub email: String,
        pub username: String,
        pub password: String,
    }

    pub const COMMUNITY_NAME_MAX_LEN: usize = 255;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct CreateCommunity {
        pub name: String,
        pub currency: CurrencySettings,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct UpdateCurrencyConfig {
        pub community_id: CommunityId,
        pub currency: CurrencySettings,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct InviteCommunityMember {
        pub community_id: CommunityId,
        pub new_member_email: Option<String>,
        pub single_use: bool,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct DeleteInvite {
        pub community_id: CommunityId,
        pub invite_id: super::InviteId,
    }

    /// An empty schedule can be used to delete the schedule entirely.
    #[derive(Debug, Serialize, Deserialize)]
    pub struct SetMembershipSchedule {
        pub community_id: CommunityId,
        pub schedule: Vec<super::MembershipSchedule>,
    }

    /// Details about a community member for a community one is a part of.
    #[derive(Debug, Serialize, Deserialize)]
    pub struct UpdateSite {
        pub site_id: super::SiteId,
        pub site_details: super::Site,
    }

    /// Details about a community member for a community one is a part of.
    #[derive(Debug, Serialize, Deserialize)]
    pub struct UpdateSpace {
        pub space_id: super::SpaceId,
        pub space_details: super::Space,
    }

    /// Batch update multiple spaces at once
    #[derive(Debug, Serialize, Deserialize)]
    pub struct UpdateSpaces {
        pub spaces: Vec<UpdateSpace>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct UserValue {
        pub space_id: super::SpaceId,
        pub value: Decimal,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct UseProxyBidding {
        pub auction_id: super::AuctionId,
        pub max_items: i32,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct ForgotPassword {
        pub email: String,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct ResetPassword {
        pub token: String,
        pub password: String,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct ResendVerificationEmail {
        pub email: String,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct VerifyEmail {
        pub token: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct UpdateProfile {
        pub display_name: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct CreateSiteImage {
        pub community_id: CommunityId,
        pub name: String,
        pub image_data: Vec<u8>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct UpdateSiteImage {
        pub id: super::SiteImageId,
        pub name: Option<String>,
        pub image_data: Option<Vec<u8>>,
    }

    // Currency operations

    #[derive(Debug, Serialize, Deserialize)]
    pub struct UpdateCreditLimitOverride {
        pub community_id: super::CommunityId,
        pub member_user_id: super::UserId,
        pub credit_limit_override: Option<Decimal>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct GetMemberCreditLimitOverride {
        pub community_id: super::CommunityId,
        pub member_user_id: super::UserId,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct UpdateMemberActiveStatus {
        pub community_id: super::CommunityId,
        pub member_user_id: super::UserId,
        pub is_active: bool,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct GetMemberCurrencyInfo {
        pub community_id: super::CommunityId,
        /// If None, returns info for the authenticated user.
        /// If Some, returns info for specified user (coleader+ only).
        pub member_user_id: Option<super::UserId>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct GetMemberTransactions {
        pub community_id: super::CommunityId,
        /// If None, returns transactions for the authenticated user.
        /// If Some, returns transactions for specified user (coleader+ only).
        pub member_user_id: Option<super::UserId>,
        pub limit: i64,
        pub offset: i64,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct CreateTransfer {
        pub community_id: super::CommunityId,
        pub to_user_id: super::UserId,
        pub amount: Decimal,
        pub note: Option<String>,
        pub idempotency_key: super::IdempotencyKey,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct GetTreasuryAccount {
        pub community_id: super::CommunityId,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct GetTreasuryTransactions {
        pub community_id: super::CommunityId,
        pub limit: i64,
        pub offset: i64,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct TreasuryCreditOperation {
        pub community_id: super::CommunityId,
        pub recipient: super::TreasuryRecipient,
        pub amount_per_recipient: Decimal,
        pub note: Option<String>,
        pub idempotency_key: super::IdempotencyKey,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ResetAllBalances {
        pub community_id: super::CommunityId,
        pub note: Option<String>,
    }
}

pub mod responses {
    use crate::{CommunityId, InviteId, UserId};
    use jiff::Timestamp;
    #[cfg(feature = "use-sqlx")]
    use jiff_sqlx::Timestamp as SqlxTs;
    use rust_decimal::Decimal;
    use serde::{Deserialize, Serialize};

    /// User identification bundled with display information
    ///
    /// This is the standard way to reference users in API responses.
    /// The frontend should display display_name (if present) or username,
    /// but use user_id for any API calls that reference the user.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
    pub struct UserIdentity {
        pub user_id: UserId,
        pub username: String,
        /// Community-specific display name (if set for this community)
        pub display_name: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Community {
        pub id: CommunityId,
        pub name: String,
        pub created_at: Timestamp,
        pub updated_at: Timestamp,
        pub currency: super::CurrencySettings,
    }

    /// A community invite that has been issued from a given community.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
    pub struct IssuedCommunityInvite {
        pub id: InviteId,
        pub new_member_email: Option<String>,
        pub single_use: bool,
        #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
        pub created_at: Timestamp,
    }

    /// Details about a community invite, excluding the target community id.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
    pub struct CommunityInviteReceived {
        pub id: InviteId,
        pub community_name: String,
        #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
        pub created_at: Timestamp,
    }

    /// Details about a community member for a community one is a part of.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct CommunityMember {
        pub user: UserIdentity,
        pub role: super::Role,
        pub is_active: bool,
        /// Balance is included if user is coleader+ or
        /// balances_visible_to_members is true
        pub balance: Option<rust_decimal::Decimal>,
    }

    /// Community information with the current user's role in that community.
    /// This is used by the get_communities endpoint to provide role information
    /// so the frontend can show/hide controls based on permissions.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct CommunityWithRole {
        pub community: Community,
        /// The current user's role in this community
        pub user_role: super::Role,
        /// Whether the current user is active in this community
        pub user_is_active: bool,
    }

    impl std::ops::Deref for CommunityWithRole {
        type Target = Community;

        fn deref(&self) -> &Self::Target {
            &self.community
        }
    }

    /// Details about a community member for a community one is a part of.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Site {
        pub site_id: super::SiteId,
        pub site_details: super::Site,
        pub created_at: Timestamp,
        pub updated_at: Timestamp,
        pub deleted_at: Option<Timestamp>,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Space {
        pub space_id: super::SpaceId,
        pub space_details: super::Space,
        pub created_at: Timestamp,
        pub updated_at: Timestamp,
        pub deleted_at: Option<Timestamp>,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct UpdateSpaceResult {
        pub space: Space,
        /// True if copy-on-write was performed (space had auction history + nontrivial changes)
        pub was_copied: bool,
        /// If was_copied is true, this contains the old space ID that was soft-deleted
        pub old_space_id: Option<super::SpaceId>,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Auction {
        pub auction_id: super::AuctionId,
        pub auction_details: super::Auction,
        pub end_at: Option<Timestamp>,
        pub created_at: Timestamp,
        pub updated_at: Timestamp,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct AuctionRound {
        pub round_id: super::AuctionRoundId,
        pub round_details: super::AuctionRound,
        pub created_at: Timestamp,
        pub updated_at: Timestamp,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct UserValue {
        pub space_id: super::SpaceId,
        pub value: Decimal,
        pub created_at: Timestamp,
        pub updated_at: Timestamp,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct UseProxyBidding {
        pub auction_id: super::AuctionId,
        pub max_items: i32,
        pub created_at: Timestamp,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct UserProfile {
        pub user_id: UserId,
        pub username: String,
        pub email: String,
        pub display_name: Option<String>,
        pub email_verified: bool,
        pub balance: Decimal,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SuccessMessage {
        pub message: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
    pub struct SiteImage {
        pub id: super::SiteImageId,
        pub community_id: super::CommunityId,
        pub name: String,
        pub image_data: Vec<u8>,
        #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
        pub created_at: Timestamp,
        #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
        pub updated_at: Timestamp,
    }

    /// Currency information for a member account
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MemberCurrencyInfo {
        pub account_id: super::AccountId,
        pub balance: Decimal,
        pub credit_limit: Option<Decimal>,
        pub locked_balance: Decimal,
        pub available_credit: Option<Decimal>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MemberCreditLimitOverride {
        pub credit_limit_override: Option<Decimal>,
    }

    /// Represents a participant in a transaction (member or treasury)
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum TransactionParty {
        Member(UserIdentity),
        Treasury,
    }

    /// A line in a transaction showing who sent/received currency
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TransactionLine {
        pub party: TransactionParty,
        /// Positive = received, Negative = sent
        pub amount: Decimal,
    }

    /// Transaction history entry for display to members
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MemberTransaction {
        pub entry_type: super::EntryType,
        pub auction_id: Option<super::AuctionId>,
        pub note: Option<String>,
        pub created_at: Timestamp,
        /// Lines in the transaction relevant to the requesting user
        /// (typically shows who they sent to or received from)
        pub lines: Vec<TransactionLine>,
    }

    /// Result of resetting all member balances to zero
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct BalanceResetResult {
        /// Number of member accounts affected
        pub accounts_reset: usize,
        /// Total amount transferred to treasury
        pub total_transferred: Decimal,
    }
}

use derive_more::Display;
use reqwest::StatusCode;
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

type ReqwestResult = Result<reqwest::Response, reqwest::Error>;

/// An API client for interfacing with the backend.
pub struct APIClient {
    pub address: String,
    pub inner_client: reqwest::Client,
}

/// Helper methods for http actions
impl APIClient {
    fn format_url(&self, path: &str) -> String {
        format!("{}/api/{path}", &self.address)
    }

    async fn post(&self, path: &str, body: &impl Serialize) -> ReqwestResult {
        let request = self.inner_client.post(self.format_url(path)).json(body);

        #[cfg(target_arch = "wasm32")]
        let request = request.fetch_credentials_include();

        request.send().await
    }

    async fn empty_post(&self, path: &str) -> ReqwestResult {
        let request = self.inner_client.post(self.format_url(path));

        #[cfg(target_arch = "wasm32")]
        let request = request.fetch_credentials_include();

        request.send().await
    }

    async fn empty_get(&self, path: &str) -> ReqwestResult {
        let request = self.inner_client.get(self.format_url(path));

        #[cfg(target_arch = "wasm32")]
        let request = request.fetch_credentials_include();

        request.send().await
    }
}

/// Methods on the backend API
impl APIClient {
    pub async fn health_check(&self) -> Result<(), ClientError> {
        let response = self.empty_get("health_check").await?;
        ok_empty(response).await
    }

    pub async fn create_account(
        &self,
        details: &requests::CreateAccount,
    ) -> Result<(), ClientError> {
        let response = self.post("create_account", details).await?;
        ok_empty(response).await
    }

    pub async fn login(
        &self,
        details: &requests::LoginCredentials,
    ) -> Result<(), ClientError> {
        let response = self.post("login", &details).await?;
        ok_empty(response).await
    }

    pub async fn logout(&self) -> Result<(), ClientError> {
        let response = self.empty_post("logout").await?;
        ok_empty(response).await
    }

    /// Delete the current user's account.
    pub async fn delete_user(&self) -> Result<(), ClientError> {
        let response = self.empty_post("delete_user").await?;
        ok_empty(response).await
    }

    /// Delete a community (leader only).
    pub async fn delete_community(
        &self,
        community_id: &CommunityId,
    ) -> Result<(), ClientError> {
        let response = self.post("delete_community", community_id).await?;
        ok_empty(response).await
    }

    /// Check if the user is logged in.
    pub async fn login_check(&self) -> Result<bool, ClientError> {
        let response = self.empty_post("login_check").await?;
        match response.status() {
            StatusCode::OK => Ok(true),
            StatusCode::UNAUTHORIZED => Ok(false),
            _ => Err(ClientError::APIError(
                response.status(),
                response.text().await?,
            )),
        }
    }

    /// Get the current user's profile information.
    pub async fn user_profile(
        &self,
    ) -> Result<responses::UserProfile, ClientError> {
        let response = self.empty_get("user_profile").await?;
        ok_body(response).await
    }

    /// Verify email address using a token from the verification email.
    pub async fn verify_email(
        &self,
        details: &requests::VerifyEmail,
    ) -> Result<responses::SuccessMessage, ClientError> {
        let response = self.post("verify_email", details).await?;
        ok_body(response).await
    }

    /// Request a password reset email for the given email address.
    pub async fn forgot_password(
        &self,
        details: &requests::ForgotPassword,
    ) -> Result<responses::SuccessMessage, ClientError> {
        let response = self.post("forgot_password", details).await?;
        ok_body(response).await
    }

    /// Reset password using a token from the password reset email.
    pub async fn reset_password(
        &self,
        details: &requests::ResetPassword,
    ) -> Result<responses::SuccessMessage, ClientError> {
        let response = self.post("reset_password", details).await?;
        ok_body(response).await
    }

    /// Resend email verification for the given email address.
    pub async fn resend_verification_email(
        &self,
        details: &requests::ResendVerificationEmail,
    ) -> Result<responses::SuccessMessage, ClientError> {
        let response = self.post("resend_verification_email", details).await?;
        ok_body(response).await
    }

    pub async fn create_community(
        &self,
        details: &requests::CreateCommunity,
    ) -> Result<CommunityId, ClientError> {
        let response = self.post("create_community", &details).await?;
        ok_body(response).await
    }

    /// Update currency configuration for a community (coleader+ only).
    pub async fn update_currency_config(
        &self,
        details: &requests::UpdateCurrencyConfig,
    ) -> Result<(), ClientError> {
        let response = self.post("update_currency_config", &details).await?;
        ok_empty(response).await
    }

    /// Get the communities for the currently logged in user.
    pub async fn get_communities(
        &self,
    ) -> Result<Vec<responses::CommunityWithRole>, ClientError> {
        let response = self.empty_get("communities").await?;
        ok_body(response).await
    }

    pub async fn get_received_invites(
        &self,
    ) -> Result<Vec<responses::CommunityInviteReceived>, ClientError> {
        let response = self.empty_get("received_invites").await?;
        ok_body(response).await
    }

    pub async fn invite_member(
        &self,
        details: &requests::InviteCommunityMember,
    ) -> Result<InviteId, ClientError> {
        let response = self.post("invite_member", details).await?;
        ok_body(response).await
    }

    pub async fn get_issued_invites(
        &self,
        community_id: &CommunityId,
    ) -> Result<Vec<responses::IssuedCommunityInvite>, ClientError> {
        let response = self.post("issued_invites", community_id).await?;
        ok_body(response).await
    }

    pub async fn get_invite_community_name(
        &self,
        invite_id: &InviteId,
    ) -> Result<String, ClientError> {
        let response = self
            .empty_get(&format!("invite_community_name/{invite_id}"))
            .await?;
        ok_body(response).await
    }

    pub async fn accept_invite(
        &self,
        invite_id: &InviteId,
    ) -> Result<(), ClientError> {
        let response = self
            .empty_post(&format!("accept_invite/{invite_id}"))
            .await?;
        ok_empty(response).await
    }

    pub async fn delete_invite(
        &self,
        details: &requests::DeleteInvite,
    ) -> Result<(), ClientError> {
        let response = self.post("delete_invite", details).await?;
        ok_empty(response).await
    }

    /// Get the communities for the currently logged in user.
    pub async fn get_members(
        &self,
        community_id: &CommunityId,
    ) -> Result<Vec<responses::CommunityMember>, ClientError> {
        let response = self.post("members", community_id).await?;
        ok_body(response).await
    }

    /// Get the communities for the currently logged in user.
    pub async fn set_membership_schedule(
        &self,
        details: &requests::SetMembershipSchedule,
    ) -> Result<(), ClientError> {
        let response = self.post("membership_schedule", &details).await?;
        ok_empty(response).await
    }

    /// Get the communities for the currently logged in user.
    pub async fn get_membership_schedule(
        &self,
        community_id: &CommunityId,
    ) -> Result<Vec<MembershipSchedule>, ClientError> {
        let response =
            self.post("get_membership_schedule", &community_id).await?;
        ok_body(response).await
    }

    pub async fn update_member_active_status(
        &self,
        details: &requests::UpdateMemberActiveStatus,
    ) -> Result<(), ClientError> {
        let response =
            self.post("update_member_active_status", &details).await?;
        ok_empty(response).await
    }

    pub async fn create_site(
        &self,
        site: &Site,
    ) -> Result<SiteId, ClientError> {
        let response = self.post("create_site", &site).await?;
        ok_body(response).await
    }

    pub async fn get_site(
        &self,
        site_id: &SiteId,
    ) -> Result<responses::Site, ClientError> {
        let response = self.post("get_site", &site_id).await?;
        ok_body(response).await
    }

    pub async fn update_site(
        &self,
        details: &requests::UpdateSite,
    ) -> Result<responses::Site, ClientError> {
        let response = self.post("site", details).await?;
        ok_body(response).await
    }

    pub async fn delete_site(
        &self,
        site_id: &SiteId,
    ) -> Result<(), ClientError> {
        let response = self.post("delete_site", &site_id).await?;
        ok_empty(response).await
    }

    pub async fn soft_delete_site(
        &self,
        site_id: &SiteId,
    ) -> Result<(), ClientError> {
        let response = self.post("soft_delete_site", &site_id).await?;
        ok_empty(response).await
    }

    pub async fn restore_site(
        &self,
        site_id: &SiteId,
    ) -> Result<(), ClientError> {
        let response = self.post("restore_site", &site_id).await?;
        ok_empty(response).await
    }

    pub async fn list_sites(
        &self,
        community_id: &CommunityId,
    ) -> Result<Vec<responses::Site>, ClientError> {
        let response = self.post("sites", &community_id).await?;
        ok_body(response).await
    }

    pub async fn create_space(
        &self,
        space: &Space,
    ) -> Result<SpaceId, ClientError> {
        let response = self.post("create_space", &space).await?;
        ok_body(response).await
    }

    pub async fn get_space(
        &self,
        space_id: &SpaceId,
    ) -> Result<responses::Space, ClientError> {
        let response = self.post("get_space", &space_id).await?;
        ok_body(response).await
    }

    pub async fn update_space(
        &self,
        details: &requests::UpdateSpace,
    ) -> Result<responses::UpdateSpaceResult, ClientError> {
        let response = self.post("space", details).await?;
        ok_body(response).await
    }

    pub async fn update_spaces(
        &self,
        details: &requests::UpdateSpaces,
    ) -> Result<Vec<responses::UpdateSpaceResult>, ClientError> {
        let response = self.post("spaces_batch", details).await?;
        ok_body(response).await
    }

    pub async fn delete_space(
        &self,
        space_id: &SpaceId,
    ) -> Result<(), ClientError> {
        let response = self.post("delete_space", &space_id).await?;
        ok_empty(response).await
    }

    pub async fn soft_delete_space(
        &self,
        space_id: &SpaceId,
    ) -> Result<(), ClientError> {
        let response = self.post("soft_delete_space", &space_id).await?;
        ok_empty(response).await
    }

    pub async fn restore_space(
        &self,
        space_id: &SpaceId,
    ) -> Result<(), ClientError> {
        let response = self.post("restore_space", &space_id).await?;
        ok_empty(response).await
    }

    pub async fn list_spaces(
        &self,
        site_id: &SiteId,
    ) -> Result<Vec<responses::Space>, ClientError> {
        let response = self.post("spaces", &site_id).await?;
        ok_body(response).await
    }

    pub async fn create_auction(
        &self,
        auction: &Auction,
    ) -> Result<AuctionId, ClientError> {
        let response = self.post("create_auction", &auction).await?;
        ok_body(response).await
    }

    pub async fn get_auction(
        &self,
        auction_id: &AuctionId,
    ) -> Result<responses::Auction, ClientError> {
        let response = self.post("auction", &auction_id).await?;
        ok_body(response).await
    }

    pub async fn delete_auction(
        &self,
        auction_id: &AuctionId,
    ) -> Result<(), ClientError> {
        let response = self.post("delete_auction", &auction_id).await?;
        ok_empty(response).await
    }

    pub async fn list_auctions(
        &self,
        site_id: &SiteId,
    ) -> Result<Vec<responses::Auction>, ClientError> {
        let response = self.post("auctions", &site_id).await?;
        ok_body(response).await
    }

    pub async fn get_auction_round(
        &self,
        round_id: &AuctionRoundId,
    ) -> Result<responses::AuctionRound, ClientError> {
        let response = self.post("auction_round", &round_id).await?;
        ok_body(response).await
    }

    pub async fn list_auction_rounds(
        &self,
        auction_id: &AuctionId,
    ) -> Result<Vec<responses::AuctionRound>, ClientError> {
        let response = self.post("auction_rounds", &auction_id).await?;
        ok_body(response).await
    }

    pub async fn get_round_space_result(
        &self,
        space_id: &SpaceId,
        round_id: &AuctionRoundId,
    ) -> Result<RoundSpaceResult, ClientError> {
        let response = self
            .post("round_space_result", &(space_id, round_id))
            .await?;
        ok_body(response).await
    }

    pub async fn list_round_space_results_for_round(
        &self,
        round_id: &AuctionRoundId,
    ) -> Result<Vec<RoundSpaceResult>, ClientError> {
        let response = self
            .post("round_space_results_for_round", &round_id)
            .await?;
        ok_body(response).await
    }

    pub async fn create_bid(
        &self,
        space_id: &SpaceId,
        round_id: &AuctionRoundId,
    ) -> Result<(), ClientError> {
        let response = self.post("create_bid", &(space_id, round_id)).await?;
        ok_empty(response).await
    }

    pub async fn get_bid(
        &self,
        space_id: &SpaceId,
        round_id: &AuctionRoundId,
    ) -> Result<Bid, ClientError> {
        let response = self.post("bid", &(space_id, round_id)).await?;
        ok_body(response).await
    }

    pub async fn list_bids(
        &self,
        round_id: &AuctionRoundId,
    ) -> Result<Vec<Bid>, ClientError> {
        let response = self.post("bids", &round_id).await?;
        ok_body(response).await
    }

    pub async fn delete_bid(
        &self,
        space_id: &SpaceId,
        round_id: &AuctionRoundId,
    ) -> Result<(), ClientError> {
        let response = self.post("delete_bid", &(space_id, round_id)).await?;
        ok_empty(response).await
    }

    pub async fn get_eligibility(
        &self,
        round_id: &AuctionRoundId,
    ) -> Result<Option<f64>, ClientError> {
        let response = self.post("get_eligibility", &round_id).await?;
        ok_body(response).await
    }

    pub async fn list_eligibility(
        &self,
        auction_id: &AuctionId,
    ) -> Result<Vec<Option<f64>>, ClientError> {
        let response = self.post("list_eligibility", &auction_id).await?;
        ok_body(response).await
    }

    pub async fn create_or_update_user_value(
        &self,
        details: &requests::UserValue,
    ) -> Result<(), ClientError> {
        let response =
            self.post("create_or_update_user_value", details).await?;
        ok_empty(response).await
    }

    pub async fn get_user_value(
        &self,
        space_id: &SpaceId,
    ) -> Result<responses::UserValue, ClientError> {
        let response = self.post("get_user_value", space_id).await?;
        ok_body(response).await
    }

    pub async fn delete_user_value(
        &self,
        space_id: &SpaceId,
    ) -> Result<(), ClientError> {
        let response = self.post("delete_user_value", space_id).await?;
        ok_empty(response).await
    }

    pub async fn list_user_values(
        &self,
        site_id: &SiteId,
    ) -> Result<Vec<responses::UserValue>, ClientError> {
        let response = self.post("user_values", site_id).await?;
        ok_body(response).await
    }

    pub async fn create_or_update_proxy_bidding(
        &self,
        details: &requests::UseProxyBidding,
    ) -> Result<(), ClientError> {
        let response =
            self.post("create_or_update_proxy_bidding", details).await?;
        ok_empty(response).await
    }

    pub async fn get_proxy_bidding(
        &self,
        auction_id: &AuctionId,
    ) -> Result<Option<responses::UseProxyBidding>, ClientError> {
        let response = self.post("get_proxy_bidding", auction_id).await?;
        ok_body(response).await
    }

    pub async fn delete_proxy_bidding(
        &self,
        auction_id: &AuctionId,
    ) -> Result<(), ClientError> {
        let response = self.post("delete_proxy_bidding", auction_id).await?;
        ok_empty(response).await
    }

    // Currency operations

    pub async fn update_credit_limit_override(
        &self,
        details: &requests::UpdateCreditLimitOverride,
    ) -> Result<Account, ClientError> {
        let response =
            self.post("update_credit_limit_override", details).await?;
        ok_body(response).await
    }

    pub async fn get_member_credit_limit_override(
        &self,
        details: &requests::GetMemberCreditLimitOverride,
    ) -> Result<responses::MemberCreditLimitOverride, ClientError> {
        let response = self
            .post("get_member_credit_limit_override", details)
            .await?;
        ok_body(response).await
    }

    pub async fn get_member_currency_info(
        &self,
        details: &requests::GetMemberCurrencyInfo,
    ) -> Result<responses::MemberCurrencyInfo, ClientError> {
        let response = self.post("get_member_currency_info", details).await?;
        ok_body(response).await
    }

    pub async fn get_member_transactions(
        &self,
        details: &requests::GetMemberTransactions,
    ) -> Result<Vec<responses::MemberTransaction>, ClientError> {
        let response = self.post("get_member_transactions", details).await?;
        ok_body(response).await
    }

    pub async fn create_transfer(
        &self,
        details: &requests::CreateTransfer,
    ) -> Result<(), ClientError> {
        let response = self.post("create_transfer", details).await?;
        ok_empty(response).await
    }

    pub async fn get_treasury_account(
        &self,
        details: &requests::GetTreasuryAccount,
    ) -> Result<Account, ClientError> {
        let response = self.post("get_treasury_account", details).await?;
        ok_body(response).await
    }

    pub async fn get_treasury_transactions(
        &self,
        details: &requests::GetTreasuryTransactions,
    ) -> Result<Vec<responses::MemberTransaction>, ClientError> {
        let response = self.post("get_treasury_transactions", details).await?;
        ok_body(response).await
    }

    pub async fn treasury_credit_operation(
        &self,
        details: &requests::TreasuryCreditOperation,
    ) -> Result<TreasuryOperationResult, ClientError> {
        let response = self.post("treasury_credit_operation", details).await?;
        ok_body(response).await
    }

    pub async fn reset_all_balances(
        &self,
        details: &requests::ResetAllBalances,
    ) -> Result<responses::BalanceResetResult, ClientError> {
        let response = self.post("reset_all_balances", details).await?;
        ok_body(response).await
    }

    pub async fn update_profile(
        &self,
        details: &requests::UpdateProfile,
    ) -> Result<responses::UserProfile, ClientError> {
        let response = self.post("update_profile", details).await?;
        ok_body(response).await
    }

    pub async fn create_site_image(
        &self,
        details: &requests::CreateSiteImage,
    ) -> Result<SiteImageId, ClientError> {
        let response = self.post("create_site_image", details).await?;
        ok_body(response).await
    }

    pub async fn get_site_image(
        &self,
        site_image_id: &SiteImageId,
    ) -> Result<responses::SiteImage, ClientError> {
        let response = self.post("get_site_image", site_image_id).await?;
        ok_body(response).await
    }

    pub async fn update_site_image(
        &self,
        details: &requests::UpdateSiteImage,
    ) -> Result<responses::SiteImage, ClientError> {
        let response = self.post("update_site_image", details).await?;
        ok_body(response).await
    }

    pub async fn delete_site_image(
        &self,
        site_image_id: &SiteImageId,
    ) -> Result<(), ClientError> {
        let response = self.post("delete_site_image", site_image_id).await?;
        ok_empty(response).await
    }

    pub async fn list_site_images(
        &self,
        community_id: &CommunityId,
    ) -> Result<Vec<responses::SiteImage>, ClientError> {
        let response = self.post("list_site_images", community_id).await?;
        ok_body(response).await
    }
}
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// An unhandled API error to display, containing response text.
    #[error("{1}")]
    APIError(StatusCode, String),
    #[error("Network error. Please check your connection.")]
    Network(#[from] reqwest::Error),
}

/// Deserialize a successful request into the desired type, or return an
/// appropriate error.
pub async fn ok_body<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, ClientError> {
    if !response.status().is_success() {
        return Err(ClientError::APIError(
            response.status(),
            response.text().await?,
        ));
    }
    Ok(response.json::<T>().await?)
}

/// Check that an empty response is OK, returning a ClientError if not.
pub async fn ok_empty(response: reqwest::Response) -> Result<(), ClientError> {
    if !response.status().is_success() {
        return Err(ClientError::APIError(
            response.status(),
            response.text().await?,
        ));
    }
    Ok(())
}
