use crate::helpers::spawn_app;
use backend::time;
use jiff::Span;

#[tokio::test]
async fn test_time_mock() -> anyhow::Result<()> {
    let initial_time = time::now();

    time::advance_mock_time(Span::new().hours(1));
    assert_eq!(time::now(), initial_time + Span::new().hours(1));

    let new_time = initial_time + Span::new().hours(7);
    time::set_mock_time(new_time);
    assert_eq!(time::now(), new_time);

    Ok(())
}

#[tokio::test]
async fn test_auction_crud() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let auction = app.create_test_auction(&site.site_id).await?;
    let retrieved = app.client.get_auction(&auction.auction_id).await?;
    assert_eq!(auction.auction_id, retrieved.auction_id);

    let auctions = app.client.list_auctions(&site.site_id).await?;
    assert_eq!(auctions.len(), 1);
    assert_eq!(auctions[0].auction_id, auction.auction_id);

    app.client.delete_auction(&auction.auction_id).await?;
    let auctions = app.client.list_auctions(&site.site_id).await?;
    assert!(auctions.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_auction_unauthorized() -> anyhow::Result<()> {
    use crate::helpers::assert_status_code;
    use reqwest::StatusCode;

    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let auction = app.create_test_auction(&site.site_id).await?;

    // new user that's not part of the community
    app.client.logout().await?;
    let details = payloads::requests::CreateAccount {
        username: "charlie".into(),
        password: "charliepw".into(),
        email: "charlie@example.com".into(),
    };
    app.client.create_account(&details).await?;
    app.client.login(&details).await?;

    assert_status_code(
        app.client.get_auction(&auction.auction_id).await,
        StatusCode::UNAUTHORIZED,
    );
    assert_status_code(
        app.client.list_auctions(&site.site_id).await,
        StatusCode::UNAUTHORIZED,
    );
    assert_status_code(
        app.client.delete_auction(&auction.auction_id).await,
        StatusCode::UNAUTHORIZED,
    );

    Ok(())
}
