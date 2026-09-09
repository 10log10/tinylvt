//! Cap delegations: effective caps at bid time, creation-order backing,
//! member-only permissions and the start-time freeze, and the bulk cap
//! assignment they pair with.

use jiff::Span;
use payloads::{ApiError, PermissionLevel, UserId, requests, responses};
use rust_decimal::Decimal;
use test_helpers::{self, assert_api_error, spawn_app};

fn one_point_space(
    site_id: payloads::SiteId,
    name: &str,
    category_id: Option<payloads::SpaceCategoryId>,
) -> payloads::Space {
    payloads::Space {
        site_id,
        name: name.into(),
        description: None,
        eligibility_points: 1.0,
        category_id,
        is_available: true,
        site_image_id: None,
        reserve_price: payloads::ReservePrice(Decimal::ZERO),
    }
}

fn user_id_of(
    members: &[responses::CommunityMember],
    username: &str,
) -> UserId {
    members
        .iter()
        .find(|m| m.user.username == username)
        .unwrap()
        .user
        .user_id
}

/// Everyone gets an office cap of 1; charlie delegates his to bob, who can
/// then hold two offices while charlie can hold none. Delegations freeze
/// once the auction starts.
#[tokio::test]
async fn test_delegation_effective_cap_at_bid_time() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_three_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let office_id = app
        .client
        .create_space_category(&payloads::SpaceCategory {
            community_id,
            name: "Office".into(),
        })
        .await?;
    let mut offices = Vec::new();
    for name in ["o1", "o2", "o3"] {
        offices.push(
            app.client
                .create_space(&one_point_space(
                    site.site_id,
                    name,
                    Some(office_id),
                ))
                .await?,
        );
    }

    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.capped = true;
    auction_details.start_at = None;
    let auction_id = app.client.create_auction(&auction_details).await?;

    let members = app.client.get_members(&community_id).await?;
    let bob = user_id_of(&members, "bob");
    let charlie = user_id_of(&members, "charlie");

    app.client
        .set_bidder_cap_for_all(&requests::SetBidderCapForAll {
            auction_id,
            category_id: Some(office_id),
            points: 1.0,
        })
        .await?;

    app.login_charlie().await?;
    app.client
        .set_cap_delegation(&requests::SetCapDelegation {
            auction_id,
            to_user_id: bob,
            category_id: Some(office_id),
            points: 1.0,
        })
        .await?;

    // Both parties see the delegation, fully backed by charlie's cap.
    let mine = app.client.my_cap_delegations(&auction_id).await?;
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].backed, 1.0);
    app.login_bob().await?;
    let mine = app.client.my_cap_delegations(&auction_id).await?;
    assert_eq!(mine.len(), 1);
    assert_eq!((mine[0].from_user_id, mine[0].to_user_id), (charlie, bob));

    // Effective caps: bob 2, charlie nothing (no row means 0).
    let bob_caps = app.client.my_bidder_caps(&auction_id).await?;
    assert_eq!(bob_caps.len(), 1);
    assert_eq!(bob_caps[0].points, 2.0);
    app.login_charlie().await?;
    assert!(app.client.my_bidder_caps(&auction_id).await?.is_empty());

    // The coleader's cap list still shows the assigned rows.
    app.login_alice().await?;
    let caps = app.client.list_bidder_caps(&auction_id).await?;
    assert_eq!(caps.len(), 3);
    assert!(caps.iter().all(|c| c.points == 1.0));

    app.client
        .schedule_auction(&requests::ScheduleAuction {
            auction_id,
            start_at: Some(app.time_source.now() + Span::new().seconds(1)),
        })
        .await?;
    app.time_source.advance(Span::new().seconds(2));
    app.tick().await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_0 = rounds[0].round_id;

    app.login_bob().await?;
    app.client.create_bid(&offices[0], &round_0).await?;
    app.client.create_bid(&offices[1], &round_0).await?;
    let result = app.client.create_bid(&offices[2], &round_0).await;
    assert_api_error(
        result,
        ApiError::ExceedsBidderCap {
            available: 2.0,
            required: 3.0,
            category: Some("Office".into()),
        },
    );

    app.login_charlie().await?;
    let result = app.client.create_bid(&offices[2], &round_0).await;
    assert_api_error(
        result,
        ApiError::ExceedsBidderCap {
            available: 0.0,
            required: 1.0,
            category: Some("Office".into()),
        },
    );

    // Frozen after start, for both the delegator's edit and a removal.
    for points in [2.0, 0.0] {
        let result = app
            .client
            .set_cap_delegation(&requests::SetCapDelegation {
                auction_id,
                to_user_id: bob,
                category_id: Some(office_id),
                points,
            })
            .await;
        assert_api_error(result, ApiError::CapDelegationsFrozenAfterStart);
    }

    Ok(())
}

/// Delegations are promises resolved against the delegator's cap in
/// creation order, so they can be recorded before the cap exists and an
/// over-promised cap honors the earliest ones first.
#[tokio::test]
async fn test_delegation_backing_order() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_three_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.capped = true;
    auction_details.start_at = None;
    let auction_id = app.client.create_auction(&auction_details).await?;

    let members = app.client.get_members(&community_id).await?;
    let alice = user_id_of(&members, "alice");
    let bob = user_id_of(&members, "bob");
    let charlie = user_id_of(&members, "charlie");

    // Bob promises 1 point to alice, then 1 to charlie, holding no cap yet.
    app.login_bob().await?;
    let delegate = |to: UserId, points: f64| requests::SetCapDelegation {
        auction_id,
        to_user_id: to,
        category_id: None,
        points,
    };
    app.client.set_cap_delegation(&delegate(alice, 1.0)).await?;
    app.time_source.advance(Span::new().seconds(1));
    app.client
        .set_cap_delegation(&delegate(charlie, 1.0))
        .await?;

    let backed_for = |delegations: &[responses::CapDelegation], to: UserId| {
        delegations
            .iter()
            .find(|d| d.to_user_id == to)
            .unwrap()
            .backed
    };

    app.login_alice().await?;
    let all = app.client.list_cap_delegations(&auction_id).await?;
    assert_eq!(all.len(), 2);
    assert_eq!(backed_for(&all, alice), 0.0);
    assert_eq!(backed_for(&all, charlie), 0.0);
    assert!(app.client.my_bidder_caps(&auction_id).await?.is_empty());

    // A cap of 1 backs the earlier delegation only.
    let set_bob_cap = |points: f64| requests::SetBidderCap {
        auction_id,
        user_id: bob,
        category_id: None,
        points,
    };
    app.client.set_bidder_cap(&set_bob_cap(1.0)).await?;
    let all = app.client.list_cap_delegations(&auction_id).await?;
    assert_eq!(backed_for(&all, alice), 1.0);
    assert_eq!(backed_for(&all, charlie), 0.0);
    let alice_caps = app.client.my_bidder_caps(&auction_id).await?;
    assert_eq!(alice_caps[0].points, 1.0);
    app.login_bob().await?;
    assert!(app.client.my_bidder_caps(&auction_id).await?.is_empty());

    // A cap of 2 backs both.
    app.login_alice().await?;
    app.client.set_bidder_cap(&set_bob_cap(2.0)).await?;
    let all = app.client.list_cap_delegations(&auction_id).await?;
    assert_eq!(backed_for(&all, alice), 1.0);
    assert_eq!(backed_for(&all, charlie), 1.0);

    // Raising the earlier delegation un-backs the later one: ordering is
    // by creation, not by last edit.
    app.login_bob().await?;
    app.time_source.advance(Span::new().seconds(1));
    app.client.set_cap_delegation(&delegate(alice, 2.0)).await?;
    app.login_alice().await?;
    let all = app.client.list_cap_delegations(&auction_id).await?;
    assert_eq!(backed_for(&all, alice), 2.0);
    assert_eq!(backed_for(&all, charlie), 0.0);

    // Revoking it frees the cap for the later delegation.
    app.login_bob().await?;
    app.client.set_cap_delegation(&delegate(alice, 0.0)).await?;
    let bob_caps = app.client.my_bidder_caps(&auction_id).await?;
    assert_eq!(bob_caps[0].points, 1.0);
    app.login_alice().await?;
    let all = app.client.list_cap_delegations(&auction_id).await?;
    assert_eq!(all.len(), 1);
    assert_eq!(backed_for(&all, charlie), 1.0);

    Ok(())
}

#[tokio::test]
async fn test_delegation_permissions() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_three_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let members = app.client.get_members(&community_id).await?;
    let bob = user_id_of(&members, "bob");
    let charlie = user_id_of(&members, "charlie");

    let mut uncapped_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    uncapped_details.start_at = None;
    let uncapped_id = app.client.create_auction(&uncapped_details).await?;
    let mut auction_details = uncapped_details.clone();
    auction_details.capped = true;
    let auction_id = app.client.create_auction(&auction_details).await?;

    let delegation = |auction_id, to, points| requests::SetCapDelegation {
        auction_id,
        to_user_id: to,
        category_id: None,
        points,
    };

    app.login_bob().await?;
    let result = app
        .client
        .set_cap_delegation(&delegation(uncapped_id, charlie, 1.0))
        .await;
    assert_api_error(result, ApiError::AuctionNotCapped);
    let result = app
        .client
        .set_cap_delegation(&delegation(auction_id, bob, 1.0))
        .await;
    assert_api_error(result, ApiError::CapDelegationToSelf);
    let result = app
        .client
        .set_cap_delegation(&delegation(auction_id, charlie, -1.0))
        .await;
    assert_api_error(result, ApiError::InvalidCapPoints);
    app.client
        .set_cap_delegation(&delegation(auction_id, charlie, 1.0))
        .await?;

    // Members see only their own; the full list is coleader-only.
    let result = app.client.list_cap_delegations(&auction_id).await;
    assert_api_error(
        result,
        ApiError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        },
    );

    // The recipient sees it but holds no delegation of their own: the
    // request always acts on the caller's delegations, so a recipient
    // "revoking" it just clears a row that doesn't exist.
    app.login_charlie().await?;
    let mine = app.client.my_cap_delegations(&auction_id).await?;
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].from_user_id, bob);
    app.client
        .set_cap_delegation(&delegation(auction_id, bob, 0.0))
        .await?;
    assert_eq!(app.client.my_cap_delegations(&auction_id).await?.len(), 1);

    app.login_bob().await?;
    app.client
        .set_cap_delegation(&delegation(auction_id, charlie, 0.0))
        .await?;
    assert!(app.client.my_cap_delegations(&auction_id).await?.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_set_bidder_cap_for_all() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_three_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.capped = true;
    let auction_id = app.client.create_auction(&auction_details).await?;

    let members = app.client.get_members(&community_id).await?;
    let charlie = user_id_of(&members, "charlie");
    app.client
        .update_member_active_status(&requests::UpdateMemberActiveStatus {
            community_id,
            member_user_id: charlie,
            is_active: false,
        })
        .await?;

    let for_all = |points| requests::SetBidderCapForAll {
        auction_id,
        category_id: None,
        points,
    };

    app.login_bob().await?;
    let result = app.client.set_bidder_cap_for_all(&for_all(1.0)).await;
    assert_api_error(
        result,
        ApiError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        },
    );

    // Active members only; a repeat overwrites rather than adds.
    app.login_alice().await?;
    app.client.set_bidder_cap_for_all(&for_all(1.0)).await?;
    app.client.set_bidder_cap_for_all(&for_all(2.0)).await?;
    let caps = app.client.list_bidder_caps(&auction_id).await?;
    assert_eq!(caps.len(), 2);
    assert!(caps.iter().all(|c| c.points == 2.0));
    assert!(caps.iter().all(|c| c.user_id != charlie));

    app.client.set_bidder_cap_for_all(&for_all(0.0)).await?;
    assert!(app.client.list_bidder_caps(&auction_id).await?.is_empty());

    Ok(())
}
