use actix_identity::Identity;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, get, post, web};
use sqlx::PgPool;

use crate::password::{
    AuthError, Credentials, NewUserDetails, create_user, validate_credentials,
};
use crate::store;

use super::{APIError, get_user_id};

#[tracing::instrument(
    skip(credentials, pool),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
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
#[tracing::instrument(skip(new_user_details, pool))]
#[post("/create_account")]
pub async fn create_account(
    request: HttpRequest,
    new_user_details: web::Json<NewUserDetails>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    create_user(new_user_details.0, &pool).await?;
    Ok(HttpResponse::Ok().finish())
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
