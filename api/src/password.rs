use crate::store::{self, StoreError};
use crate::telemetry::spawn_blocking_with_tracing;
use anyhow::Context;
use argon2::password_hash::SaltString;
use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier,
    Version,
};
use secrecy::{ExposeSecret, SecretBox};
use sqlx::PgPool;

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

#[derive(serde::Deserialize)]
pub struct Credentials {
    pub username: String,
    password: SecretBox<String>,
}

#[tracing::instrument(name = "Validate credentials", skip(credentials, pool))]
pub async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<payloads::UserId, AuthError> {
    let mut user_id = None;
    // fallback password hash to prevent timing differences
    let mut expected_password_hash = SecretBox::new(Box::new(
        "$argon2id$v=19$m=15000,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
            .to_string(),
    ));

    if let Some((stored_user_id, stored_password_hash)) =
        get_stored_credentials(&credentials.username, pool).await?
    {
        user_id = Some(stored_user_id);
        expected_password_hash = stored_password_hash;
    }

    spawn_blocking_with_tracing(move || {
        verify_password_hash(expected_password_hash, credentials.password)
    })
    .await
    .context("Failed to spawn blocking task.")??;

    user_id
        .ok_or_else(|| anyhow::anyhow!("Unknown username."))
        .map_err(AuthError::InvalidCredentials)
}

#[tracing::instrument(name = "Get stored credentials", skip(username, pool))]
async fn get_stored_credentials(
    username: &str,
    pool: &PgPool,
) -> Result<Option<(payloads::UserId, SecretBox<String>)>, anyhow::Error> {
    let user = sqlx::query_as::<_, store::User>(
        r#"SELECT * FROM users WHERE username = $1 AND deleted_at IS NULL;"#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .context("Failed to performed a query to retrieve stored credentials.")?
    .map(|user| (user.id, SecretBox::new(Box::new(user.password_hash))));
    Ok(user)
}

#[tracing::instrument(
    name = "Validate credentials",
    skip(expected_password_hash, password_candidate)
)]
fn verify_password_hash(
    expected_password_hash: SecretBox<String>,
    password_candidate: SecretBox<String>,
) -> Result<(), AuthError> {
    let expected_password_hash =
        PasswordHash::new(expected_password_hash.expose_secret())
            .context("Failed to parse hash in PHC string format.")?;

    Argon2::default()
        .verify_password(
            password_candidate.expose_secret().as_bytes(),
            &expected_password_hash,
        )
        .context("Invalid password.")
        .map_err(AuthError::InvalidCredentials)
}

#[tracing::instrument(name = "Change password", skip(password, pool), ret)]
pub async fn change_password(
    user_id: payloads::UserId,
    password: SecretBox<String>,
    pool: &PgPool,
) -> Result<(), anyhow::Error> {
    let password_hash =
        spawn_blocking_with_tracing(move || compute_password_hash(password))
            .await?
            .context("Failed to hash password")?;
    sqlx::query(
        r#"
        UPDATE users
        SET password_hash = $1
        WHERE id = $2
        "#,
    )
    .bind(password_hash.expose_secret())
    .bind(user_id)
    .execute(pool)
    .await
    .context("Failed to change user's password in the database.")?;
    Ok(())
}

#[derive(serde::Deserialize)]
pub struct NewUserDetails {
    pub username: String,
    pub email: String,
    password: SecretBox<String>,
}

#[tracing::instrument(
    name = "Create user",
    skip(new_user_details, pool, time_source),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn create_user(
    new_user_details: NewUserDetails,
    pool: &PgPool,
    time_source: &crate::time::TimeSource,
) -> Result<payloads::UserId, StoreError> {
    let password_hash = spawn_blocking_with_tracing(move || {
        compute_password_hash(new_user_details.password)
    })
    .await
    .map_err(anyhow::Error::from)?
    .context("Failed to hash password")?;
    let new_user_id = store::create_user(
        pool,
        &new_user_details.username,
        &new_user_details.email,
        password_hash.expose_secret(),
        time_source,
    )
    .await?
    .id;
    tracing::Span::current()
        .record(
            "username",
            tracing::field::display(&new_user_details.username),
        )
        .record("user_id", tracing::field::display(&new_user_id));
    Ok(new_user_id)
}

fn compute_password_hash(
    password: SecretBox<String>,
) -> Result<SecretBox<String>, anyhow::Error> {
    let salt = SaltString::generate(&mut rand_core::OsRng);
    let password_hash = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(15000, 2, 1, None).unwrap(),
    )
    .hash_password(password.expose_secret().as_bytes(), &salt)?
    .to_string();
    Ok(SecretBox::new(Box::new(password_hash)))
}
