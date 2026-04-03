use actix_identity::Identity;
use actix_web::{HttpRequest, HttpResponse, post, web};
use payloads::{CheckoutSessionResponse, CommunityStorageUsage, TierLimits};
use sqlx::PgPool;

use crate::AppConfig;
use crate::store;
use crate::stripe_service::StripeService;

use super::{APIError, get_user_id, get_validated_member};

#[post("/get_community_storage_usage")]
pub async fn get_community_storage_usage(
    user: Identity,
    request: web::Json<payloads::requests::GetCommunityStorageUsage>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let actor =
        get_validated_member(&user_id, &request.community_id, &pool).await?;

    let usage =
        store::billing::get_storage_usage(&pool, &time_source, &actor).await?;

    let tier =
        store::billing::get_subscription_tier(&pool, request.community_id)
            .await?;

    let response = CommunityStorageUsage {
        usage,
        tier,
        limits: TierLimits::for_tier(tier),
    };

    Ok(HttpResponse::Ok().json(response))
}

#[post("/get_subscription_info")]
pub async fn get_subscription_info(
    user: Identity,
    request: web::Json<payloads::requests::GetSubscriptionInfo>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let actor =
        get_validated_member(&user_id, &request.community_id, &pool).await?;

    let info = store::billing::get_subscription_info(&pool, &actor).await?;

    Ok(HttpResponse::Ok().json(info))
}

#[post("/create_checkout_session")]
pub async fn create_checkout_session(
    user: Identity,
    request: web::Json<payloads::requests::CreateCheckoutSession>,
    pool: web::Data<PgPool>,
    stripe_service: web::Data<StripeService>,
    app_config: web::Data<AppConfig>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let actor =
        get_validated_member(&user_id, &request.community_id, &pool).await?;

    let checkout_url = store::billing::create_checkout_session(
        &pool,
        &stripe_service,
        &app_config,
        &actor,
        request.billing_interval,
    )
    .await?;

    Ok(HttpResponse::Ok().json(CheckoutSessionResponse { checkout_url }))
}

#[post("/create_portal_session")]
pub async fn create_portal_session(
    user: Identity,
    request: web::Json<payloads::requests::CreatePortalSession>,
    pool: web::Data<PgPool>,
    stripe_service: web::Data<StripeService>,
    app_config: web::Data<AppConfig>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let actor =
        get_validated_member(&user_id, &request.community_id, &pool).await?;

    let portal_url = store::billing::create_portal_session(
        &pool,
        &stripe_service,
        &app_config,
        &actor,
    )
    .await?;

    Ok(HttpResponse::Ok().json(CheckoutSessionResponse {
        checkout_url: portal_url,
    }))
}

#[post("/stripe_webhook")]
pub async fn stripe_webhook(
    req: HttpRequest,
    body: web::Bytes,
    stripe_service: web::Data<StripeService>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let payload = std::str::from_utf8(&body).map_err(|_| {
        APIError::BadRequest(anyhow::anyhow!("Invalid UTF-8 payload"))
    })?;

    let signature = req
        .headers()
        .get("Stripe-Signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            APIError::BadRequest(anyhow::anyhow!(
                "Missing Stripe-Signature header"
            ))
        })?;

    let event = stripe_service
        .verify_webhook(payload, signature, &time_source)
        .map_err(|e| {
            APIError::BadRequest(anyhow::anyhow!(
                "Webhook verification failed: {e:#}"
            ))
        })?;

    store::billing::handle_webhook_event(
        &pool,
        &time_source,
        &stripe_service,
        &event,
    )
    .await?;

    Ok(HttpResponse::Ok().finish())
}
