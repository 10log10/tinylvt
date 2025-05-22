use crate::helpers::spawn_app;

#[tokio::test]
async fn create_and_read_site() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    app.create_test_site(&community_id).await?;

    Ok(())
}
