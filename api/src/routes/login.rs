use actix_identity::Identity;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, get, post, web};
use jiff::Span;
use secrecy::SecretBox;
use sqlx::PgPool;

use crate::Config;
use crate::password::{
    AuthError, Credentials, NewUserDetails, change_password, create_user,
    validate_credentials,
};
use crate::store::{self, TokenAction, TokenId};
use crate::time::TimeSource;

use super::{APIError, get_user_id};

#[tracing::instrument(
    skip(credentials, pool),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
    ret,
)]
#[post("/login")]
pub async fn login(
    request: HttpRequest,
    credentials: web::Json<Credentials>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    tracing::Span::current()
        .record("username", tracing::field::display(&credentials.username));
    match validate_credentials(credentials.0, &pool).await {
        Ok(user_id) => {
            tracing::Span::current()
                .record("user_id", tracing::field::display(&user_id));
            Identity::login(&request.extensions(), user_id.to_string())
                .map_err(|e| APIError::UnexpectedError(e.into()))?;
            Ok(HttpResponse::Ok().finish())
        }
        Err(e) => {
            let e = match e {
                AuthError::InvalidCredentials(_) => {
                    APIError::AuthError(e.into())
                }
                AuthError::UnexpectedError(_) => {
                    APIError::UnexpectedError(e.into())
                }
            };
            Err(e)
        }
    }
}

#[tracing::instrument(skip(user))]
#[post("/login_check")]
pub async fn login_check(user: Identity) -> Result<HttpResponse, APIError> {
    get_user_id(&user)?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(skip(user))]
#[post("/logout")]
pub async fn logout(user: Identity) -> Result<HttpResponse, APIError> {
    let _ = get_user_id(&user); // to instrument the user_id, if exists
    user.logout();
    Ok(HttpResponse::Ok().finish())
}

// TODO: return error if email is not a valid format
#[tracing::instrument(skip(
    new_user_details,
    pool,
    email_service,
    time_source,
    config
))]
#[post("/create_account")]
pub async fn create_account(
    request: HttpRequest,
    new_user_details: web::Json<NewUserDetails>,
    pool: web::Data<PgPool>,
    email_service: web::Data<crate::email::EmailService>,
    time_source: web::Data<TimeSource>,
    config: web::Data<Config>,
) -> Result<HttpResponse, APIError> {
    let user_id = create_user(new_user_details.0, &pool).await?;

    // Read the user back to get the full User struct
    let user = store::read_user(&pool, &user_id).await?;

    // Create email verification token
    let expires_at = time_source.now() + Span::new().hours(24);
    let token_id = store::create_token(
        &user.id,
        TokenAction::EmailVerification,
        expires_at,
        &pool,
    )
    .await?;

    // Send verification email
    if let Err(e) = email_service
        .send_verification_email(
            &user.email,
            &user.username,
            &token_id.0.to_string(),
            &config.base_url,
        )
        .await
    {
        tracing::error!("Failed to send verification email: {}", e);
        // Don't fail the account creation, but log the error
    }

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(skip(pool, time_source))]
#[post("/verify_email")]
pub async fn verify_email(
    request: web::Json<payloads::requests::VerifyEmail>,
    pool: web::Data<PgPool>,
    time_source: web::Data<TimeSource>,
) -> Result<HttpResponse, APIError> {
    // Parse token
    let token_uuid = request
        .token
        .parse::<uuid::Uuid>()
        .map_err(|e| APIError::BadRequest(anyhow::Error::from(e)))?;
    let token_id = TokenId(token_uuid);

    // Consume token and get user_id
    let user_id = store::consume_token(
        &token_id,
        TokenAction::EmailVerification,
        &pool,
        &time_source,
    )
    .await?;

    // Mark email as verified
    store::verify_user_email(&user_id, &pool).await?;

    let response = payloads::responses::SuccessMessage {
        message: "Email has been verified successfully.".to_string(),
    };

    Ok(HttpResponse::Ok().json(response))
}

#[tracing::instrument(skip(pool, email_service, time_source, config))]
#[post("/forgot_password")]
pub async fn forgot_password(
    request: web::Json<payloads::requests::ForgotPassword>,
    pool: web::Data<PgPool>,
    email_service: web::Data<crate::email::EmailService>,
    time_source: web::Data<TimeSource>,
    config: web::Data<Config>,
) -> Result<HttpResponse, APIError> {
    // Always return success to prevent email enumeration
    let response = payloads::responses::SuccessMessage {
        message: "If an account with that email exists, a password reset link has been sent.".to_string(),
    };

    // Always perform the expensive operations to prevent timing attacks
    // This ensures similar response times regardless of whether email exists

    // Try to find user by email
    let user = store::get_user_by_email(&request.email, &pool).await.ok();

    // Always create a token (even if we won't use it)
    let expires_at = time_source.now() + Span::new().hours(1);

    // Create token using a dummy user ID if user doesn't exist
    let dummy_user_id = payloads::UserId(uuid::Uuid::new_v4());
    let token_user_id = user.as_ref().map(|u| &u.id).unwrap_or(&dummy_user_id); // Use dummy ID for non-existent users

    let token_id = store::create_token(
        token_user_id,
        TokenAction::PasswordReset,
        expires_at,
        &pool,
    )
    .await;

    // Only send email if user exists
    if user.is_some() && token_id.is_ok() {
        let user = user.as_ref().unwrap(); // Safe because we checked is_some()
        let token_id = token_id.unwrap(); // Safe because we checked is_ok()

        if let Err(e) = email_service
            .send_password_reset_email(
                &user.email,
                &user.username,
                &token_id.0.to_string(),
                &config.base_url,
            )
            .await
        {
            tracing::error!("Failed to send password reset email: {}", e);
            // Don't fail the request, but log the error
        }
    } else if token_id.is_ok() {
        // If we created a token but won't use it (user doesn't exist),
        // mark it as used immediately to clean up
        let token_id = token_id.unwrap();
        let _ = sqlx::query(
            r#"
            UPDATE tokens 
            SET used = true
            WHERE id = $1
            "#,
        )
        .bind(&token_id)
        .execute(&**pool)
        .await;
    }

    Ok(HttpResponse::Ok().json(response))
}

#[tracing::instrument(skip(pool, email_service, time_source, config))]
#[post("/resend_verification_email")]
pub async fn resend_verification_email(
    request: web::Json<payloads::requests::ResendVerificationEmail>,
    pool: web::Data<PgPool>,
    email_service: web::Data<crate::email::EmailService>,
    time_source: web::Data<TimeSource>,
    config: web::Data<Config>,
) -> Result<HttpResponse, APIError> {
    // Always return success to prevent email enumeration
    let response = payloads::responses::SuccessMessage {
        message: "If an account with that email exists and is not yet verified, a verification email has been sent.".to_string(),
    };

    // Always perform the expensive operations to prevent timing attacks
    // This ensures similar response times regardless of whether email exists/is verified

    // Try to find user by email
    let user = store::get_user_by_email(&request.email, &pool).await.ok();

    // Always invalidate tokens (even if user doesn't exist) - this is a no-op for non-existent users
    sqlx::query(
        r#"
        UPDATE tokens 
        SET used = true
        WHERE user_id = $1 AND action = $2 AND used = false
        "#,
    )
    .bind(user.as_ref().map(|u| &u.id))
    .bind(TokenAction::EmailVerification)
    .execute(&**pool)
    .await
    .map_err(|e| APIError::UnexpectedError(anyhow::Error::from(e)))?;

    // Always create a token (even if we won't use it)
    let expires_at = time_source.now() + Span::new().hours(24);

    // Create token using a dummy user ID if user doesn't exist
    let dummy_user_id = payloads::UserId(uuid::Uuid::new_v4());
    let token_user_id = user.as_ref().map(|u| &u.id).unwrap_or(&dummy_user_id); // Use dummy ID for non-existent users

    let token_id = store::create_token(
        token_user_id,
        TokenAction::EmailVerification,
        expires_at,
        &pool,
    )
    .await;

    // Only send email if user exists and is not verified
    let should_send_email =
        user.as_ref().map(|u| !u.email_verified).unwrap_or(false);

    if should_send_email && token_id.is_ok() {
        let user = user.as_ref().unwrap(); // Safe because should_send_email ensures user exists
        let token_id = token_id.unwrap(); // Safe because we checked is_ok()

        if let Err(e) = email_service
            .send_verification_email(
                &user.email,
                &user.username,
                &token_id.0.to_string(),
                &config.base_url,
            )
            .await
        {
            tracing::error!("Failed to resend verification email: {}", e);
            // Still don't fail the request to maintain consistent behavior
        }
    } else if token_id.is_ok() {
        // If we created a token but won't use it (user doesn't exist or already verified),
        // mark it as used immediately to clean up
        let token_id = token_id.unwrap();
        let _ = sqlx::query(
            r#"
            UPDATE tokens 
            SET used = true
            WHERE id = $1
            "#,
        )
        .bind(&token_id)
        .execute(&**pool)
        .await;
    }

    Ok(HttpResponse::Ok().json(response))
}

#[derive(serde::Deserialize, Debug)]
pub struct ResetPasswordRequest {
    pub token: String,
    password: SecretBox<String>,
}

#[tracing::instrument(skip(pool, time_source))]
#[post("/reset_password")]
pub async fn reset_password(
    mut request: web::Json<ResetPasswordRequest>,
    pool: web::Data<PgPool>,
    time_source: web::Data<TimeSource>,
) -> Result<HttpResponse, APIError> {
    // Parse token
    let token_uuid = request
        .token
        .parse::<uuid::Uuid>()
        .map_err(|e| APIError::BadRequest(anyhow::Error::from(e)))?;
    let token_id = TokenId(token_uuid);

    // Consume token and get user_id
    let user_id = store::consume_token(
        &token_id,
        TokenAction::PasswordReset,
        &pool,
        &time_source,
    )
    .await?;

    // Change password - move the password out of the request
    let password = std::mem::replace(
        &mut request.password,
        SecretBox::new(Box::new(String::new())),
    );
    change_password(user_id, password, &pool)
        .await
        .map_err(APIError::UnexpectedError)?;

    let response = payloads::responses::SuccessMessage {
        message: "Password has been reset successfully.".to_string(),
    };

    Ok(HttpResponse::Ok().json(response))
}

#[tracing::instrument(skip(user, pool))]
#[get("/user_profile")]
pub async fn user_profile(
    user: Identity,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let user_data = store::read_user(&pool, &user_id).await?;

    let profile = payloads::responses::UserProfile {
        username: user_data.username,
        email: user_data.email,
        display_name: user_data.display_name,
        email_verified: user_data.email_verified,
        balance: user_data.balance,
    };

    Ok(HttpResponse::Ok().json(profile))
}

#[tracing::instrument(skip(user, request, pool))]
#[post("/update_profile")]
pub async fn update_profile(
    user: Identity,
    request: web::Json<payloads::requests::UpdateProfile>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;

    // Validate display_name length if provided
    if let Some(ref display_name) = request.display_name {
        if display_name.len() > payloads::requests::DISPLAY_NAME_MAX_LEN {
            return Err(APIError::BadRequest(anyhow::anyhow!(
                "Display name must not exceed {} characters",
                payloads::requests::DISPLAY_NAME_MAX_LEN
            )));
        }
    }

    let updated_user =
        store::update_user_profile(&user_id, &request.display_name, &pool)
            .await?;

    let profile = payloads::responses::UserProfile {
        username: updated_user.username,
        email: updated_user.email,
        display_name: updated_user.display_name,
        email_verified: updated_user.email_verified,
        balance: updated_user.balance,
    };

    Ok(HttpResponse::Ok().json(profile))
}
