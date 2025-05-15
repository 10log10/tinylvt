use backend::{Config, build, telemetry};
use payloads::requests;
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
    _database_drop_guard: DropDatabaseGuard,
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

    pub async fn create_test_community(&self) -> anyhow::Result<()> {
        let body = requests::CreateCommunity {
            name: "Test community".into(),
        };
        self.client.create_community(&body).await?;
        Ok(())
    }

    pub async fn create_two_person_community(&self) -> anyhow::Result<()> {
        self.create_alice_user().await?;
        self.create_test_community().await?;
        self.invite_bob().await?;
        self.create_bob_user().await?;
        self.login_bob().await?;
        self.accept_invite().await?;
        Ok(())
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

pub async fn spawn_app() -> TestApp {
    let subscriber = telemetry::get_subscriber("warn".into());
    let _ = LogTracer::init();
    let _ = subscriber.try_init();

    let (conn, guard) = setup_database().await.unwrap();
    let db_url = format!("{DATABASE_URL}/{}", guard.1);
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

    let server = build(&mut config).await.unwrap();
    tokio::spawn(server);

    TestApp {
        port: config.port,
        db_pool: conn,
        client: payloads::APIClient {
            address: format!("http://127.0.0.1:{}", config.port),
            inner_client: client,
        },
        _database_drop_guard: guard,
    }
}

/// Drop guard for releasing a database that is used for a single test.
///
/// Contains a connection to the default database and the name of the
/// test-specific database to drop.
#[derive(Clone)]
pub struct DropDatabaseGuard(PgPool, String);

// TODO: currently this can emit a warning since the tokio runtime is already
// being torn down by the time sqlx is executing the command. Need some sort of
// test wrapper with catch_unwind.
impl Drop for DropDatabaseGuard {
    fn drop(&mut self) {
        let conn = self.0.clone();
        let name = self.1.clone();
        tokio::task::spawn(async move {
            let _ = sqlx::query(&format!(r#"DROP DATABASE "{}";"#, name))
                .execute(&conn)
                .await;
        });
    }
}

/// Create a new database specific for the test and migrate it.
async fn setup_database() -> Result<(PgPool, DropDatabaseGuard), Error> {
    let default_conn =
        PgPool::connect(&format!("{DATABASE_URL}/{DEFAULT_DB}")).await?;
    let new_db = Uuid::new_v4().to_string();
    sqlx::query(&format!(r#"CREATE DATABASE "{}";"#, new_db))
        .execute(&default_conn)
        .await?;
    // If anything fails, we clean up the database with the guard
    let guard = DropDatabaseGuard(default_conn, new_db.clone());
    let conn = PgPool::connect(&format!("{DATABASE_URL}/{new_db}")).await?;
    MIGRATOR.run(&conn).await?;
    Ok((conn, guard))
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
