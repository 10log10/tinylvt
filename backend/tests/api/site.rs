use crate::helpers::spawn_app;

#[tokio::test]
async fn create_read_update_delete_site() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    let response = app.create_test_site(&community_id).await?;

    let site_id = response.site_id;
    app.update_site_details(response).await?;
    app.client.delete_site(&site_id).await?;
    assert!(
        app.client
            .get_site(&site_id)
            .await
            .unwrap_err()
            .to_string()
            .contains("Row not found")
    );

    Ok(())
}
