use anyhow::Result;

use test_helpers::spawn_app;

#[tokio::test]
async fn test_security_headers_on_authenticated_endpoints() -> Result<()> {
    // Arrange
    let app = spawn_app().await;
    
    // Create and login a user
    app.create_alice_user().await?;
    
    // Get user communities - this is an authenticated endpoint
    let url = format!("{}/api/communities", app.client.address);
    let response = app.client.inner_client
        .get(&url)
        .send()
        .await?;
    
    // Verify security headers are present
    let headers = response.headers();
    
    // Check Cache-Control header
    let cache_control = headers.get("cache-control")
        .expect("Cache-Control header should be present")
        .to_str()?;
    assert!(cache_control.contains("no-store"), "Should contain no-store");
    assert!(cache_control.contains("no-cache"), "Should contain no-cache");
    assert!(cache_control.contains("must-revalidate"), "Should contain must-revalidate");
    assert!(cache_control.contains("private"), "Should contain private");
    
    // Check Pragma header
    let pragma = headers.get("pragma")
        .expect("Pragma header should be present")
        .to_str()?;
    assert_eq!(pragma, "no-cache", "Pragma should be no-cache");
    
    // Check Expires header
    let expires = headers.get("expires")
        .expect("Expires header should be present")
        .to_str()?;
    assert_eq!(expires, "0", "Expires should be 0");
    
    Ok(())
}

#[tokio::test]
async fn test_health_check_does_not_have_security_headers() -> Result<()> {
    // Arrange
    let app = spawn_app().await;
    
    // Get health check - this is not an authenticated endpoint and should not have security headers
    let url = format!("{}/api/health_check", app.client.address);
    let response = app.client.inner_client
        .get(&url)
        .send()
        .await?;
    
    // Verify security headers are NOT present for health check
    let headers = response.headers();
    
    // Health check should not have cache control headers since it's public
    assert!(headers.get("cache-control").is_none(), "Health check should not have Cache-Control header");
    assert!(headers.get("pragma").is_none(), "Health check should not have Pragma header");
    assert!(headers.get("expires").is_none(), "Health check should not have Expires header");
    
    Ok(())
} 