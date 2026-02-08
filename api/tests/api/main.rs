mod auction;
mod community;
mod currency;
mod database;
mod email;
mod login;
mod member_removal;
mod proxy_bidding;
mod security_headers;
mod site;

use test_helpers::spawn_app;

#[tokio::test]
async fn health_check() -> anyhow::Result<()> {
    let app = spawn_app().await;

    app.client.health_check().await?;

    Ok(())
}
