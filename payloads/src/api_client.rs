use crate::{
    Account, Auction, AuctionId, AuctionRoundId, Bid, CommunityId, InviteId,
    MembershipSchedule, RoundSpaceResult, Site, SiteId, SiteImageId, Space,
    SpaceId, TreasuryOperationResult, requests, responses,
};
use reqwest::StatusCode;
use serde::Serialize;

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

    /// Update community name and description (coleader+ only).
    pub async fn update_community_details(
        &self,
        details: &requests::UpdateCommunityDetails,
    ) -> Result<responses::Community, ClientError> {
        let response = self.post("update_community_details", &details).await?;
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

    pub async fn update_member_active_status(
        &self,
        details: &requests::UpdateMemberActiveStatus,
    ) -> Result<(), ClientError> {
        let response =
            self.post("update_member_active_status", &details).await?;
        ok_empty(response).await
    }

    pub async fn remove_member(
        &self,
        details: &requests::RemoveMember,
    ) -> Result<(), ClientError> {
        let response = self.post("remove_member", &details).await?;
        ok_empty(response).await
    }

    pub async fn change_member_role(
        &self,
        details: &requests::ChangeMemberRole,
    ) -> Result<(), ClientError> {
        let response = self.post("change_member_role", &details).await?;
        ok_empty(response).await
    }

    pub async fn leave_community(
        &self,
        details: &requests::LeaveCommunity,
    ) -> Result<(), ClientError> {
        let response = self.post("leave_community", &details).await?;
        ok_empty(response).await
    }

    pub async fn get_orphaned_accounts(
        &self,
        community_id: &CommunityId,
    ) -> Result<responses::OrphanedAccountsList, ClientError> {
        let response = self.post("orphaned_accounts", &community_id).await?;
        ok_body(response).await
    }

    pub async fn resolve_orphaned_balance(
        &self,
        details: &requests::ResolveOrphanedBalance,
    ) -> Result<TreasuryOperationResult, ClientError> {
        let response = self.post("resolve_orphaned_balance", &details).await?;
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

    /// Fetches full image data including metadata. Primarily for tests.
    /// For displaying images in the UI, use `site_image_url()` instead.
    pub async fn get_site_image(
        &self,
        site_image_id: &SiteImageId,
    ) -> Result<responses::SiteImage, ClientError> {
        let response = self.post("get_site_image", site_image_id).await?;
        ok_body(response).await
    }

    /// Returns the URL for fetching raw image bytes.
    /// Use this for `<img src>` attributes in the UI.
    pub fn site_image_url(&self, site_image_id: &SiteImageId) -> String {
        format!("{}/api/images/{}", self.address, site_image_id.0)
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
    ) -> Result<Vec<responses::SiteImageInfo>, ClientError> {
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
