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
    pub possession_start_at: Timestamp,
    pub possession_end_at: Timestamp,
    pub start_at: Timestamp,
    pub auction_params: AuctionParams,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct AuctionRoundId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuctionRound {
    pub auction_id: AuctionId,
    pub round_num: i32,
    pub start_at: Timestamp,
    pub end_at: Timestamp,
    pub eligibility_threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
pub struct RoundSpaceResult {
    pub space_id: SpaceId,
    pub round_id: AuctionRoundId,
    pub winning_username: Option<String>,
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

pub mod requests {
    use crate::CommunityId;
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
        pub new_members_default_active: bool,
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
}

pub mod responses {
    use crate::{CommunityId, InviteId};
    use jiff::Timestamp;
    #[cfg(feature = "use-sqlx")]
    use jiff_sqlx::Timestamp as SqlxTs;
    use rust_decimal::Decimal;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
    pub struct Community {
        pub id: CommunityId,
        pub name: String,
        pub new_members_default_active: bool,
        #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
        pub created_at: Timestamp,
        #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
        pub updated_at: Timestamp,
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
    #[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
    pub struct CommunityMember {
        pub username: String,
        pub display_name: Option<String>,
        pub role: super::Role,
        pub is_active: bool,
    }

    /// Community information with the current user's role in that community.
    /// This is used by the get_communities endpoint to provide role information
    /// so the frontend can show/hide controls based on permissions.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
    pub struct CommunityWithRole {
        pub id: CommunityId,
        pub name: String,
        pub new_members_default_active: bool,
        #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
        pub created_at: Timestamp,
        #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
        pub updated_at: Timestamp,
        /// The current user's role in this community
        pub user_role: super::Role,
        /// Whether the current user is active in this community
        pub user_is_active: bool,
    }

    /// Details about a community member for a community one is a part of.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Site {
        pub site_id: super::SiteId,
        pub site_details: super::Site,
        pub created_at: Timestamp,
        pub updated_at: Timestamp,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Space {
        pub space_id: super::SpaceId,
        pub space_details: super::Space,
        pub created_at: Timestamp,
        pub updated_at: Timestamp,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Auction {
        pub auction_id: super::AuctionId,
        pub auction_details: super::Auction,
        pub end_at: Option<Timestamp>,
        pub created_at: Timestamp,
        pub updated_at: Timestamp,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
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
}

use derive_more::Display;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Id type wrappers help ensure we don't mix up ids for different tables.
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Display, Serialize, Deserialize,
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
    Debug, Copy, Clone, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct AuctionId(pub Uuid);

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
    ) -> Result<responses::Space, ClientError> {
        let response = self.post("space", details).await?;
        ok_body(response).await
    }

    pub async fn update_spaces(
        &self,
        details: &requests::UpdateSpaces,
    ) -> Result<Vec<responses::Space>, ClientError> {
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
        space_id: &SpaceId,
        round_id: &AuctionRoundId,
    ) -> Result<Vec<Bid>, ClientError> {
        let response = self.post("bids", &(space_id, round_id)).await?;
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
    ) -> Result<f64, ClientError> {
        let response = self.post("get_eligibility", &round_id).await?;
        ok_body(response).await
    }

    pub async fn list_eligibility(
        &self,
        auction_id: &AuctionId,
    ) -> Result<Vec<f64>, ClientError> {
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
    #[error("Network error")]
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
