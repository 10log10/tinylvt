use api::time::TimeSource;

pub mod mock;
use api::{Config, telemetry};
use base64::Engine;
use jiff::Span;
use jiff_sqlx::ToSqlx;
use payloads::{CommunityId, SiteId, requests, responses};
use reqwest::StatusCode;
use sqlx::{Error, PgPool, migrate::Migrator};
use tracing_log::LogTracer;
use tracing_subscriber::util::SubscriberInitExt;
use uuid::Uuid;

static MIGRATOR: Migrator = sqlx::migrate!("../api/migrations");
const DATABASE_URL: &str = "postgresql://user:password@localhost:5433";
const DEFAULT_DB: &str = "tinylvt";

pub struct TestApp {
    #[allow(unused)]
    pub port: u16,
    pub db_pool: PgPool,
    pub client: payloads::APIClient,
    pub time_source: TimeSource,
}

/// Email testing utilities for TestApp
impl TestApp {
    /// Create a test account without marking email as verified (for testing email verification flow)
    pub async fn create_unverified_user(
        &self,
        credentials: &payloads::requests::CreateAccount,
    ) -> anyhow::Result<()> {
        self.client.create_account(credentials).await?;
        Ok(())
    }

    /// Extract verification token from database for a given email
    pub async fn get_verification_token_from_db(
        &self,
        email: &str,
    ) -> anyhow::Result<String> {
        let token = sqlx::query_scalar::<_, String>(
            "SELECT t.id::text FROM tokens t 
             JOIN users u ON t.user_id = u.id 
             WHERE u.email = $1 AND t.action = 'email_verification' AND t.used = false
             ORDER BY t.created_at DESC LIMIT 1"
        )
        .bind(email)
        .fetch_one(&self.db_pool)
        .await?;
        Ok(token)
    }

    /// Extract password reset token from database for a given email
    pub async fn get_password_reset_token_from_db(
        &self,
        email: &str,
    ) -> anyhow::Result<String> {
        let token = sqlx::query_scalar::<_, String>(
            "SELECT t.id::text FROM tokens t 
             JOIN users u ON t.user_id = u.id 
             WHERE u.email = $1 AND t.action = 'password_reset' AND t.used = false
             ORDER BY t.created_at DESC LIMIT 1"
        )
        .bind(email)
        .fetch_one(&self.db_pool)
        .await?;
        Ok(token)
    }

    /// Check if user's email is verified
    pub async fn is_email_verified(&self, email: &str) -> anyhow::Result<bool> {
        let verified = sqlx::query_scalar::<_, bool>(
            "SELECT email_verified FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_one(&self.db_pool)
        .await?;
        Ok(verified)
    }

    /// Check if token exists and is unused
    pub async fn is_token_valid(&self, token: &str) -> anyhow::Result<bool> {
        let current_time = self.time_source.now();
        let valid = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM tokens WHERE id = $1::uuid AND used = false AND expires_at > $2)"
        )
        .bind(token)
        .bind(current_time.to_sqlx())
        .fetch_one(&self.db_pool)
        .await?;
        Ok(valid)
    }

    /// Test email verification flow end-to-end
    pub async fn test_email_verification_flow(&self) -> anyhow::Result<()> {
        let credentials = payloads::requests::CreateAccount {
            email: "test-verify@example.com".to_string(),
            username: "testverify".to_string(),
            password: "password123".to_string(),
        };

        // 1. Create unverified account
        self.create_unverified_user(&credentials).await?;

        // 2. Check email is not verified
        assert!(!self.is_email_verified(&credentials.email).await?);

        // 3. Get verification token from database (simulating extracting from email)
        let token = self
            .get_verification_token_from_db(&credentials.email)
            .await?;

        // 4. Verify email using the token
        let verify_request = payloads::requests::VerifyEmail {
            token: token.clone(),
        };
        self.client.verify_email(&verify_request).await?;

        // 5. Check email is now verified
        assert!(self.is_email_verified(&credentials.email).await?);

        // 6. Check token is now used/invalid
        assert!(!self.is_token_valid(&token).await?);

        Ok(())
    }

    /// Test password reset flow end-to-end
    pub async fn test_password_reset_flow(&self) -> anyhow::Result<()> {
        let original_credentials = payloads::requests::CreateAccount {
            email: "test-reset@example.com".to_string(),
            username: "testreset".to_string(),
            password: "oldpassword123".to_string(),
        };

        // 1. Create and verify account
        self.create_unverified_user(&original_credentials).await?;
        self.mark_user_email_verified(&original_credentials.username)
            .await?;

        // 2. Test original login works
        self.client
            .login(&to_login_credentials(&original_credentials))
            .await?;
        self.client.logout().await?;

        // 3. Request password reset
        let forgot_request = payloads::requests::ForgotPassword {
            email: original_credentials.email.clone(),
        };
        self.client.forgot_password(&forgot_request).await?;

        // 4. Get reset token from database
        let reset_token = self
            .get_password_reset_token_from_db(&original_credentials.email)
            .await?;

        // 5. Reset password using token
        let new_password = "newpassword456";
        let reset_request = payloads::requests::ResetPassword {
            token: reset_token.clone(),
            password: new_password.to_string(),
        };
        self.client.reset_password(&reset_request).await?;

        // 6. Check token is now used/invalid
        assert!(!self.is_token_valid(&reset_token).await?);

        // 7. Test old password no longer works
        let old_login_result = self
            .client
            .login(&to_login_credentials(&original_credentials))
            .await;
        assert!(old_login_result.is_err());

        // 8. Test new password works
        let new_login_credentials = requests::LoginCredentials {
            username: original_credentials.username,
            password: new_password.to_string(),
        };
        self.client.login(&new_login_credentials).await?;

        Ok(())
    }
}

/// Functions to populate test data
///
/// Using anyhow::Result lets us get a backtrace from when the error was fist
/// converted to anyhow::Result. Run with RUST_BACKTRACE=1 to view.
impl TestApp {
    /// Create a test account that is verified.
    pub async fn create_alice_user(&self) -> anyhow::Result<()> {
        let body = alice_credentials();
        self.client.create_account(&body).await?;
        self.mark_user_email_verified(&body.username).await?;

        // do login
        self.client.login(&alice_login_credentials()).await?;
        Ok(())
    }

    pub async fn create_bob_user(&self) -> anyhow::Result<()> {
        let body = bob_credentials();
        self.client.create_account(&body).await?;
        self.mark_user_email_verified(&body.username).await?;
        Ok(())
    }

    pub async fn create_charlie_user(&self) -> anyhow::Result<()> {
        let body = charlie_credentials();
        self.client.create_account(&body).await?;
        self.mark_user_email_verified(&body.username).await?;
        Ok(())
    }

    pub async fn login_alice(&self) -> anyhow::Result<()> {
        self.client.logout().await?;
        self.client.login(&alice_login_credentials()).await?;
        Ok(())
    }

    pub async fn login_bob(&self) -> anyhow::Result<()> {
        self.client.logout().await?;
        self.client.login(&bob_login_credentials()).await?;
        Ok(())
    }

    pub async fn login_charlie(&self) -> anyhow::Result<()> {
        self.client.logout().await?;
        self.client.login(&charlie_login_credentials()).await?;
        Ok(())
    }

    /// Returns the path component for the invite
    pub async fn invite_bob(&self) -> anyhow::Result<payloads::InviteId> {
        let communities = self.client.get_communities().await?;
        let community_id = communities.first().unwrap().id;
        let details = requests::InviteCommunityMember {
            community_id,
            new_member_email: Some(bob_credentials().email),
            single_use: false,
        };
        Ok(self.client.invite_member(&details).await?)
    }

    /// Creates a link-based invite (no email) for testing invite link acceptance
    pub async fn create_link_invite(
        &self,
    ) -> anyhow::Result<payloads::InviteId> {
        let communities = self.client.get_communities().await?;
        let community_id = communities.first().unwrap().id;
        let details = requests::InviteCommunityMember {
            community_id,
            new_member_email: None, // Link-based invite, no email
            single_use: true,
        };
        Ok(self.client.invite_member(&details).await?)
    }

    pub async fn accept_invite(&self) -> anyhow::Result<()> {
        // get the first invite received
        let invites = self.client.get_received_invites().await?;
        let first = invites.first().unwrap();
        assert_eq!(first.community_name, "Test community");

        // accept the invite
        self.client.accept_invite(&first.id).await?;

        // check that we're now a part of the community
        let communities = self.client.get_communities().await?;
        assert!(!communities.is_empty());
        Ok(())
    }

    pub async fn mark_user_email_verified(
        &self,
        username: &str,
    ) -> anyhow::Result<()> {
        // mark email as verified
        sqlx::query("UPDATE users SET email_verified = $1 WHERE username = $2")
            .bind(true)
            .bind(username)
            .execute(&self.db_pool)
            .await
            .unwrap();
        Ok(())
    }

    pub async fn create_test_community(&self) -> anyhow::Result<CommunityId> {
        let body = requests::CreateCommunity {
            name: "Test community".into(),
            currency: payloads::CurrencySettings {
                mode_config: default_currency_config(),
                name: "dollars".into(),
                symbol: "$".into(),
                minor_units: 2,
                balances_visible_to_members: true,
                new_members_default_active: true,
            },
        };
        Ok(self.client.create_community(&body).await?)
    }

    pub async fn create_two_person_community(
        &self,
    ) -> anyhow::Result<CommunityId> {
        self.create_alice_user().await?;
        let community_id = self.create_test_community().await?;
        self.invite_bob().await?;
        self.create_bob_user().await?;
        self.login_bob().await?;
        self.accept_invite().await?;
        self.login_alice().await?;
        Ok(community_id)
    }

    pub async fn create_three_person_community(
        &self,
    ) -> anyhow::Result<CommunityId> {
        self.create_alice_user().await?;
        let community_id = self.create_test_community().await?;

        // Invite and add Bob
        self.invite_bob().await?;
        self.create_bob_user().await?;
        self.login_bob().await?;
        self.accept_invite().await?;

        // Invite and add Charlie
        self.login_alice().await?;
        let details = requests::InviteCommunityMember {
            community_id,
            new_member_email: Some(charlie_credentials().email),
            single_use: false,
        };
        self.client.invite_member(&details).await?;
        self.create_charlie_user().await?;
        self.login_charlie().await?;
        let invites = self.client.get_received_invites().await?;
        let charlie_invite = invites
            .iter()
            .find(|invite| invite.community_name == "Test community")
            .unwrap();
        self.client.accept_invite(&charlie_invite.id).await?;

        self.login_alice().await?;
        Ok(community_id)
    }

    /// Set community to points_allocation mode for testing treasury
    /// operations
    pub async fn set_points_allocation_mode(
        &self,
        community_id: CommunityId,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE communities
            SET currency_mode = 'points_allocation',
                default_credit_limit = 0,
                debts_callable = false,
                allowance_amount = 1000,
                allowance_period = INTERVAL '1 week',
                allowance_start = NOW()
            WHERE id = $1
            "#,
        )
        .bind(community_id)
        .execute(&self.db_pool)
        .await?;
        Ok(())
    }

    pub async fn create_schedule(
        &self,
        community_id: &CommunityId,
    ) -> anyhow::Result<()> {
        self.client.login(&alice_login_credentials()).await?;
        let schedule = vec![
            payloads::MembershipSchedule {
                // alice is active
                start_at: self.time_source.now() - Span::new().hours(1),
                end_at: self.time_source.now() + Span::new().hours(1),
                email: alice_credentials().email,
            },
            payloads::MembershipSchedule {
                // bob is not active
                start_at: self.time_source.now() - Span::new().hours(2),
                end_at: self.time_source.now() - Span::new().hours(1),
                email: bob_credentials().email,
            },
        ];
        let body = requests::SetMembershipSchedule {
            community_id: *community_id,
            schedule,
        };
        self.client.set_membership_schedule(&body).await?;
        // check that we can read it back
        let received_schedule = self
            .client
            .get_membership_schedule(&body.community_id)
            .await?;
        assert_eq!(body.schedule, received_schedule);
        Ok(())
    }

    pub async fn create_test_site(
        &self,
        community_id: &CommunityId,
    ) -> anyhow::Result<payloads::responses::Site> {
        let site = site_details_a(*community_id);
        let site_id = self.client.create_site(&site).await?;
        let site_response = self.client.get_site(&site_id).await?;
        let retrieved = &site_response.site_details;
        assert_site_equal(&site, retrieved)?;
        Ok(site_response)
    }

    pub async fn update_site_details(
        &self,
        prev: responses::Site,
    ) -> anyhow::Result<()> {
        let req = requests::UpdateSite {
            site_id: prev.site_id,
            site_details: site_details_b(prev.site_details.community_id),
        };
        let resp = self.client.update_site(&req).await?;
        assert_site_equal(&req.site_details, &resp.site_details)?;
        Ok(())
    }

    pub async fn create_test_space(
        &self,
        site_id: &SiteId,
    ) -> anyhow::Result<payloads::responses::Space> {
        let space = space_details_a(*site_id);
        let space_id = self.client.create_space(&space).await?;
        let space_response = self.client.get_space(&space_id).await?;
        let retrieved = &space_response.space_details;
        assert_space_equal(&space, retrieved)?;
        Ok(space_response)
    }

    pub async fn update_space_details(
        &self,
        prev: responses::Space,
    ) -> anyhow::Result<()> {
        let req = requests::UpdateSpace {
            space_id: prev.space_id,
            space_details: space_details_a_update(prev.space_details.site_id),
        };
        let resp = self.client.update_space(&req).await?;
        assert_space_equal(&req.space_details, &resp.space.space_details)?;
        Ok(())
    }

    pub async fn create_test_auction(
        &self,
        site_id: &SiteId,
    ) -> anyhow::Result<payloads::responses::Auction> {
        let auction = auction_details_a(*site_id, &self.time_source);
        let auction_id = self.client.create_auction(&auction).await?;
        let auction_response = self.client.get_auction(&auction_id).await?;
        let retrieved = &auction_response.auction_details;
        assert_auction_equal(&auction, retrieved)?;
        Ok(auction_response)
    }

    pub async fn create_test_site_image(
        &self,
        community_id: &CommunityId,
    ) -> anyhow::Result<payloads::responses::SiteImage> {
        let body = site_image_details_a(*community_id);
        let site_image_id = self.client.create_site_image(&body).await?;
        let site_image = self.client.get_site_image(&site_image_id).await?;
        Ok(site_image)
    }

    pub async fn update_site_image_details(
        &self,
        prev: payloads::responses::SiteImage,
    ) -> anyhow::Result<()> {
        let update_body = site_image_details_a_update(prev.id);
        let updated = self.client.update_site_image(&update_body).await?;
        assert_site_image_equal(
            &site_image_details_a_update_expected(prev.id, prev.community_id),
            &updated,
        )?;
        Ok(())
    }
}

pub fn alice_credentials() -> requests::CreateAccount {
    requests::CreateAccount {
        username: "alice".into(),
        password: "supersecret".into(),
        email: "alice@example.com".into(),
    }
}

pub fn alice_login_credentials() -> requests::LoginCredentials {
    to_login_credentials(&alice_credentials())
}

pub fn bob_credentials() -> requests::CreateAccount {
    requests::CreateAccount {
        username: "bob".into(),
        password: "bobspw".into(),
        email: "bob@example.com".into(),
    }
}

pub fn bob_login_credentials() -> requests::LoginCredentials {
    to_login_credentials(&bob_credentials())
}

pub fn charlie_credentials() -> requests::CreateAccount {
    requests::CreateAccount {
        username: "charlie".into(),
        password: "charliepw".into(),
        email: "charlie@example.com".into(),
    }
}

pub fn charlie_login_credentials() -> requests::LoginCredentials {
    to_login_credentials(&charlie_credentials())
}

// Helper function to convert CreateAccount to LoginCredentials
pub fn to_login_credentials(
    create_account: &requests::CreateAccount,
) -> requests::LoginCredentials {
    requests::LoginCredentials {
        username: create_account.username.clone(),
        password: create_account.password.clone(),
    }
}

fn auction_params_a() -> payloads::AuctionParams {
    payloads::AuctionParams {
        round_duration: Span::new().minutes(1),
        bid_increment: rust_decimal::dec!(1.0),
        activity_rule_params: payloads::ActivityRuleParams {
            eligibility_progression: vec![
                (0, 0.5),
                (10, 0.75),
                (20, 0.9),
                (30, 1.0),
            ],
        },
    }
}

fn site_details_a(community_id: CommunityId) -> payloads::Site {
    let open_hours = payloads::OpenHours {
        days_of_week: vec![payloads::OpenHoursWeekday {
            day_of_week: 1,
            open_time: "09:22:45".parse().unwrap(),
            close_time: "17:30:00".parse().unwrap(),
        }],
    };
    payloads::Site {
        community_id,
        name: "test site".into(),
        description: Some("test description".into()),
        default_auction_params: auction_params_a(),
        possession_period: Span::new().hours(1),
        auction_lead_time: Span::new().minutes(45),
        proxy_bidding_lead_time: Span::new().days(1),
        open_hours: Some(open_hours),
        auto_schedule: true,
        timezone: Some("America/Los_Angeles".into()),
        site_image_id: None,
    }
}

pub fn site_details_b(community_id: CommunityId) -> payloads::Site {
    let default_auction_params = payloads::AuctionParams {
        round_duration: Span::new().minutes(5),
        bid_increment: rust_decimal::dec!(2.5),
        activity_rule_params: payloads::ActivityRuleParams {
            eligibility_progression: vec![
                (0, 0.6),
                (10, 0.9),
                (20, 0.96),
                (30, 1.0),
            ],
        },
    };
    let open_hours = payloads::OpenHours {
        days_of_week: vec![payloads::OpenHoursWeekday {
            day_of_week: 2,
            open_time: "10:00".parse().unwrap(),
            close_time: "16:00".parse().unwrap(),
        }],
    };
    payloads::Site {
        community_id,
        name: "test site b".into(),
        description: Some("test description for b".into()),
        default_auction_params,
        possession_period: Span::new().hours(2),
        auction_lead_time: Span::new().minutes(60),
        proxy_bidding_lead_time: Span::new().days(2),
        open_hours: Some(open_hours),
        auto_schedule: true,
        timezone: Some("America/Los_Angeles".into()),
        site_image_id: None,
    }
}
pub fn assert_site_equal(
    site: &payloads::Site,
    retrieved: &payloads::Site,
) -> anyhow::Result<()> {
    assert_eq!(site.community_id, retrieved.community_id);
    assert_eq!(site.name, retrieved.name);
    assert_eq!(site.description, retrieved.description);
    assert_eq!(
        site.default_auction_params
            .round_duration
            .compare(retrieved.default_auction_params.round_duration)?,
        std::cmp::Ordering::Equal
    );
    assert_eq!(
        site.default_auction_params.bid_increment,
        retrieved.default_auction_params.bid_increment
    );
    assert_eq!(
        site.auction_lead_time
            .compare(retrieved.auction_lead_time)?,
        std::cmp::Ordering::Equal
    );
    assert_eq!(
        site.proxy_bidding_lead_time.fieldwise(),
        retrieved.proxy_bidding_lead_time
    );
    assert_eq!(site.auto_schedule, retrieved.auto_schedule);
    assert_eq!(site.open_hours, retrieved.open_hours);
    Ok(())
}

pub fn space_details_a(site_id: SiteId) -> payloads::Space {
    payloads::Space {
        site_id,
        name: "test space".into(),
        description: Some("test space description".into()),
        eligibility_points: 10.0,
        is_available: true,
        site_image_id: None,
    }
}

#[allow(unused)]
pub fn space_details_b(site_id: SiteId) -> payloads::Space {
    payloads::Space {
        site_id,
        name: "test space b".into(),
        description: None,
        eligibility_points: 10.0,
        is_available: true,
        site_image_id: None,
    }
}

#[allow(unused)]
pub fn space_details_c(site_id: SiteId) -> payloads::Space {
    payloads::Space {
        site_id,
        name: "test space c".into(),
        description: Some("test space c description".into()),
        eligibility_points: 10.0,
        is_available: true,
        site_image_id: None,
    }
}

fn space_details_a_update(site_id: SiteId) -> payloads::Space {
    payloads::Space {
        site_id,
        name: "test space a updated".into(),
        description: Some("updated test space description".into()),
        eligibility_points: 15.0,
        is_available: false,
        site_image_id: None,
    }
}

pub fn assert_space_equal(
    space: &payloads::Space,
    retrieved: &payloads::Space,
) -> anyhow::Result<()> {
    assert_eq!(space.site_id, retrieved.site_id);
    assert_eq!(space.name, retrieved.name);
    assert_eq!(space.description, retrieved.description);
    assert_eq!(space.eligibility_points, retrieved.eligibility_points);
    assert_eq!(space.is_available, retrieved.is_available);
    Ok(())
}

pub fn site_image_details_a(
    community_id: CommunityId,
) -> payloads::requests::CreateSiteImage {
    // Create a minimal valid 1x1 red PNG image using base64-encoded data
    // This is a proper PNG file that browsers can display
    let red_png_base64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";
    let red_png_data = base64::engine::general_purpose::STANDARD
        .decode(red_png_base64)
        .expect("Valid base64 for red PNG");

    payloads::requests::CreateSiteImage {
        community_id,
        name: "Red Square".into(),
        image_data: red_png_data,
    }
}

pub fn site_image_details_b(
    community_id: CommunityId,
) -> payloads::requests::CreateSiteImage {
    let blue_png_base64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGNgYPgPAAEDAQAIicLsAAAAAElFTkSuQmCC";
    let blue_png_data = base64::engine::general_purpose::STANDARD
        .decode(blue_png_base64)
        .expect("Valid base64 for blue PNG");

    payloads::requests::CreateSiteImage {
        community_id,
        name: "Blue Square".into(),
        image_data: blue_png_data,
    }
}

fn site_image_details_a_update(
    id: payloads::SiteImageId,
) -> payloads::requests::UpdateSiteImage {
    payloads::requests::UpdateSiteImage {
        id,
        name: Some("test image updated".into()),
        image_data: Some(vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00,
        ]), // Updated PNG
    }
}

fn site_image_details_a_update_expected(
    id: payloads::SiteImageId,
    community_id: CommunityId,
) -> payloads::responses::SiteImage {
    payloads::responses::SiteImage {
        id,
        community_id,
        name: "test image updated".into(),
        image_data: vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00,
        ],
        created_at: jiff::Timestamp::now(), // This will be overridden in the test
        updated_at: jiff::Timestamp::now(), // This will be overridden in the test
    }
}

pub fn assert_site_image_equal(
    expected: &payloads::responses::SiteImage,
    retrieved: &payloads::responses::SiteImage,
) -> anyhow::Result<()> {
    assert_eq!(expected.id, retrieved.id);
    assert_eq!(expected.community_id, retrieved.community_id);
    assert_eq!(expected.name, retrieved.name);
    assert_eq!(expected.image_data, retrieved.image_data);
    // We don't check timestamps as they are set by the database
    Ok(())
}

pub async fn spawn_app_on_port(port: u16) -> TestApp {
    let subscriber = telemetry::get_subscriber("error".into());
    let _ = LogTracer::init();
    let _ = subscriber.try_init();

    #[cfg(any(feature = "mock-time", test))]
    let time_source = TimeSource::new("2025-01-01T00:00:00Z".parse().unwrap());

    #[cfg(not(any(feature = "mock-time", test)))]
    let time_source = TimeSource::new();

    let (db_pool, new_db_name) = setup_database().await.unwrap();
    let db_url = format!("{DATABASE_URL}/{}", new_db_name);
    let mut config = Config {
        database_url: db_url,
        ip: "127.0.0.1".into(),
        port,
        allowed_origins: vec!["*".to_string()],
        email_api_key: secrecy::SecretBox::new(Box::new(
            "test-api-key".to_string(),
        )),
        email_from_address: "test@example.com".to_string(),
        base_url: "http://localhost:8080".to_string(),
        session_master_key: None,
    };

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    let server = api::build(&mut config, time_source.clone()).await.unwrap();
    tokio::spawn(server);

    TestApp {
        port: config.port,
        db_pool,
        client: payloads::APIClient {
            address: format!("http://127.0.0.1:{}", config.port),
            inner_client: client,
        },
        time_source,
    }
}

/// Use OS-assigned port for parallel testing.
pub async fn spawn_app() -> TestApp {
    spawn_app_on_port(0).await
}

/// Create a new database specific for the test and migrate it, returning a
/// connection and the name of the new database.
async fn setup_database() -> Result<(PgPool, String), Error> {
    let default_conn =
        PgPool::connect(&format!("{DATABASE_URL}/{DEFAULT_DB}")).await?;
    let new_db = Uuid::new_v4().to_string();
    sqlx::query(&format!(r#"CREATE DATABASE "{}";"#, new_db))
        .execute(&default_conn)
        .await?;
    // If anything fails, we clean up the database with the guard
    let conn = PgPool::connect(&format!("{DATABASE_URL}/{new_db}")).await?;
    MIGRATOR.run(&conn).await?;
    Ok((conn, new_db))
}

/// Assert that the result of an API action results in a specific status code.
pub fn assert_status_code<T>(
    result: Result<T, payloads::ClientError>,
    expected: StatusCode,
) {
    match result {
        Err(payloads::ClientError::APIError(code, _)) => {
            assert_eq!(code, expected)
        }
        _ => panic!("Expected APIError"),
    };
}

pub fn auction_details_a(
    site_id: SiteId,
    time_source: &TimeSource,
) -> payloads::Auction {
    use jiff::Span;
    payloads::Auction {
        site_id,
        possession_start_at: time_source.now() + Span::new().hours(1),
        possession_end_at: time_source.now() + Span::new().hours(2),
        start_at: time_source.now(),
        auction_params: auction_params_a(),
    }
}

pub fn assert_auction_equal(
    auction: &payloads::Auction,
    retrieved: &payloads::Auction,
) -> anyhow::Result<()> {
    assert_eq!(auction.site_id, retrieved.site_id);
    assert_eq!(auction.possession_start_at, retrieved.possession_start_at);
    assert_eq!(auction.possession_end_at, retrieved.possession_end_at);
    assert_eq!(auction.start_at, retrieved.start_at);
    Ok(())
}

/// Default currency configuration for testing: distributed clearing with
/// unlimited credit and callable debts
pub fn default_currency_config() -> payloads::CurrencyModeConfig {
    payloads::CurrencyModeConfig::DistributedClearing(payloads::IOUConfig {
        default_credit_limit: None,
        debts_callable: true,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, sqlx::FromRow)]
#[sqlx(transparent)]
pub struct DBId(pub String);

/// See all databases that were created during testing.
///
/// ```
/// cargo test auction::check_all_databases -- --nocapture
/// ```
#[tokio::test]
async fn check_all_databases() -> anyhow::Result<()> {
    let app = spawn_app().await;

    let dbs = sqlx::query_as::<_, DBId>(
        "SELECT datname FROM pg_database
        WHERE datistemplate = false;",
    )
    .fetch_all(&app.db_pool)
    .await?;

    dbg!(dbs);

    Ok(())
}
