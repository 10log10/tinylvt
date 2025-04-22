use backend::{Config, build};
use sqlx::{Error, PgPool, migrate::Migrator};
use uuid::Uuid;

static MIGRATOR: Migrator = sqlx::migrate!();
const DATABASE_URL: &str = "postgresql://user:password@localhost:5432";
const DEFAULT_DB: &str = "tinylvt";

pub struct TestApp {
    pub address: String,
    #[allow(unused)]
    pub port: u16,
    pub db_pool: PgPool,
    pub api_client: reqwest::Client,
    _database_drop_guard: DropDatabaseGuard,
}

impl TestApp {
    pub async fn post_login<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/login", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }
}

pub async fn spawn_app() -> TestApp {
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
        address: format!("http://127.0.0.1:{}", config.port),
        port: config.port,
        db_pool: conn,
        api_client: client,
        _database_drop_guard: guard,
    }
}

/// Drop guard for releasing a database that is used for a single test.
///
/// Contains a connection to the default database and the name of the
/// test-specific database to drop.
#[derive(Clone)]
pub struct DropDatabaseGuard(PgPool, String);

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

pub fn assert_is_redirect_to(response: &reqwest::Response, location: &str) {
    assert_eq!(response.status().as_u16(), 303);
    assert_eq!(response.headers().get("Location").unwrap(), location);
}
