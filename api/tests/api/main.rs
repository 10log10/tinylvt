mod auction;
mod auction_sim;
mod billing;
mod bulk_activate;
mod community;
mod connect;
mod currency;
mod database;
mod email;
mod funding;
mod funding_auth;
mod funding_capture;
mod funding_checkout;
mod funding_schedule;
mod login;
mod member_removal;
mod payment_profile;
mod proxy_bidding;
mod pubsub;
mod purchases;
mod reconciliation;
mod reserve_pricing;
mod schema_reference;
mod security_headers;
mod site;
mod stripe_mock;
mod stripe_sandbox;

use test_helpers::spawn_app;

#[tokio::test]
async fn health_check() -> anyhow::Result<()> {
    let app = spawn_app().await;

    app.client.health_check().await?;

    Ok(())
}
