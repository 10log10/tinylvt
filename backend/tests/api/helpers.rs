use backend::time::TimeSource;
use backend::{Config, build, telemetry};
use jiff::Span;
use payloads::{CommunityId, SiteId, requests, responses};
use reqwest::StatusCode;
use sqlx::{Error, PgPool, migrate::Migrator};
use tracing_log::LogTracer;
use tracing_subscriber::util::SubscriberInitExt;
use uuid::Uuid;

static MIGRATOR: Migrator = sqlx::migrate!();
const DATABASE_URL: &str = "postgresql://user:password@localhost:5432";
const DEFAULT_DB: &str = "tinylvt";

pub struct TestApp {
    #[allow(unused)]
    pub port: u16,
    pub db_pool: PgPool,
    pub client: payloads::APIClient,
    pub time_source: TimeSource,
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
        self.client.login(&body).await?;
        Ok(())
    }

    pub async fn create_bob_user(&self) -> anyhow::Result<()> {
        let body = bob_credentials();
        self.client.create_account(&body).await?;
        self.mark_user_email_verified(&body.username).await?;
        Ok(())
    }

    pub async fn login_alice(&self) -> anyhow::Result<()> {
        self.client.logout().await?;
        self.client.login(&alice_credentials()).await?;
        Ok(())
    }

    pub async fn login_bob(&self) -> anyhow::Result<()> {
        self.client.logout().await?;
        self.client.login(&bob_credentials()).await?;
        Ok(())
    }

    /// Returns the path component for the invite
    pub async fn invite_bob(&self) -> anyhow::Result<String> {
        let communities = self.client.get_communities().await?;
        let community_id = communities.first().unwrap().id;
        let details = requests::InviteCommunityMember {
            community_id,
            new_member_email: Some(bob_credentials().email),
        };
        Ok(self.client.invite_member(&details).await?)
    }

    pub async fn accept_invite(&self) -> anyhow::Result<()> {
        // get the first invite received
        let invites = self.client.get_invites().await?;
        let first = invites.first().unwrap();
        assert_eq!(first.community_name, "Test community");

        // accept the invite
        self.client.accept_invite(&first.id).await?;

        // check that we're now a part of the community
        let communities = self.client.get_communities().await?;
        assert!(!communities.is_empty());
        Ok(())
    }

    async fn mark_user_email_verified(
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
            new_members_default_active: true,
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

    pub async fn create_schedule(
        &self,
        community_id: &CommunityId,
    ) -> anyhow::Result<()> {
        self.client.login(&alice_credentials()).await?;
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
        assert_space_equal(&req.space_details, &resp.space_details)?;
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
}

fn alice_credentials() -> requests::CreateAccount {
    requests::CreateAccount {
        username: "alice".into(),
        password: "supersecret".into(),
        email: "alice@example.com".into(),
    }
}

fn bob_credentials() -> requests::CreateAccount {
    requests::CreateAccount {
        username: "bob".into(),
        password: "bobspw".into(),
        email: "bob@example.com".into(),
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
        timezone: "America/Los_Angeles".into(),
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
        timezone: "America/Los_Angeles".into(),
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
    }
}

fn space_details_a_update(site_id: SiteId) -> payloads::Space {
    payloads::Space {
        site_id,
        name: "test space a updated".into(),
        description: Some("updated test space description".into()),
        eligibility_points: 15.0,
        is_available: false,
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

pub async fn spawn_app() -> TestApp {
    let subscriber = telemetry::get_subscriber("warn".into());
    let _ = LogTracer::init();
    let _ = subscriber.try_init();
    let time_source = TimeSource::new("2025-01-01T00:00:00Z".parse().unwrap());

    let (db_pool, new_db_name) = setup_database().await.unwrap();
    let db_url = format!("{DATABASE_URL}/{}", new_db_name);
    let mut config = Config {
        database_url: db_url,
        ip: "127.0.0.1".into(),
        port: 0,
    };

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    let server = build(&mut config, time_source.clone()).await.unwrap();
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
