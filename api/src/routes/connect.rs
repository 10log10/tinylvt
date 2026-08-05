use actix_identity::Identity;
use actix_web::{HttpRequest, HttpResponse, post, web};
use payloads::CheckoutSessionResponse;
use sqlx::PgPool;

use crate::AppConfig;
use crate::store;
use crate::stripe_service::StripeService;

use super::{RouteError, get_user_id, get_validated_member};

#[post("/connect_community_stripe")]
pub async fn connect_community_stripe(
    user: Identity,
    community_id: web::Json<payloads::CommunityId>,
    pool: web::Data<PgPool>,
    stripe_service: web::Data<StripeService>,
    app_config: web::Data<AppConfig>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let actor = get_validated_member(&user_id, &community_id, &pool).await?;

    let onboarding_url = store::connect::connect_community_stripe(
        &actor,
        &stripe_service,
        &app_config,
        &time_source,
        &pool,
    )
    .await?;

    Ok(HttpResponse::Ok().json(CheckoutSessionResponse {
        checkout_url: onboarding_url,
    }))
}

#[post("/get_community_stripe_status")]
pub async fn get_community_stripe_status(
    user: Identity,
    community_id: web::Json<payloads::CommunityId>,
    pool: web::Data<PgPool>,
    stripe_service: web::Data<StripeService>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let actor = get_validated_member(&user_id, &community_id, &pool).await?;

    let status = store::connect::get_community_stripe_status(
        &actor,
        &stripe_service,
        &time_source,
        &pool,
    )
    .await?;

    Ok(HttpResponse::Ok().json(status))
}

/// Webhook endpoint for events from connected accounts. Separate from
/// the platform endpoint: Connect events are signed with their own
/// secret.
#[post("/stripe_connect_webhook")]
pub async fn stripe_connect_webhook(
    req: HttpRequest,
    body: web::Bytes,
    stripe_service: web::Data<StripeService>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let payload = std::str::from_utf8(&body).map_err(|_| {
        RouteError::BadRequest(anyhow::anyhow!("Invalid UTF-8 payload"))
    })?;

    let signature = req
        .headers()
        .get("Stripe-Signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            RouteError::BadRequest(anyhow::anyhow!(
                "Missing Stripe-Signature header"
            ))
        })?;

    let event = stripe_service
        .verify_connect_webhook(payload, signature, &time_source)
        .map_err(|e| {
            RouteError::BadRequest(anyhow::anyhow!(
                "Webhook verification failed: {e:#}"
            ))
        })?;

    store::connect::handle_connect_webhook_event(
        &pool,
        &time_source,
        &stripe_service,
        &event,
    )
    .await?;

    Ok(HttpResponse::Ok().finish())
}
