//! Space categories and per-bidder bidding caps: category CRUD and
//! community scoping, the bid-time cap check, cap CRUD permissions and
//! events, seeding from a concluded auction's results, and proxy
//! cap-awareness.

use jiff::Span;
use payloads::{ApiError, PermissionLevel, requests};
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

#[tokio::test]
async fn test_space_category_crud() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Members can list but not create.
    app.login_bob().await?;
    let result = app
        .client
        .create_space_category(&payloads::SpaceCategory {
            community_id,
            name: "Standard".into(),
        })
        .await;
    assert_api_error(
        result,
        ApiError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        },
    );

    app.login_alice().await?;
    let standard_id = app
        .client
        .create_space_category(&payloads::SpaceCategory {
            community_id,
            name: "Standard".into(),
        })
        .await?;
    app.client
        .create_space_category(&payloads::SpaceCategory {
            community_id,
            name: "Electric".into(),
        })
        .await?;

    // Duplicate name within the community is rejected, as is a blank one.
    let result = app
        .client
        .create_space_category(&payloads::SpaceCategory {
            community_id,
            name: "Standard".into(),
        })
        .await;
    assert_api_error(
        result,
        ApiError::SpaceCategoryNameNotUnique {
            name: "Standard".into(),
        },
    );
    let result = app
        .client
        .create_space_category(&payloads::SpaceCategory {
            community_id,
            name: "   ".into(),
        })
        .await;
    assert_api_error(result, ApiError::SpaceCategoryNameEmpty);

    app.login_bob().await?;
    let categories = app.client.list_space_categories(&community_id).await?;
    assert_eq!(
        categories
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Electric", "Standard"],
    );

    // Rename; names are stored trimmed.
    app.login_alice().await?;
    let renamed = app
        .client
        .update_space_category(&requests::UpdateSpaceCategory {
            category_id: standard_id,
            name: " Standard booth ".into(),
        })
        .await?;
    assert_eq!(renamed.name, "Standard booth");

    // Deletion is refused while a space references the category.
    let space_id = app
        .client
        .create_space(&one_point_space(
            site.site_id,
            "Booth A",
            Some(standard_id),
        ))
        .await?;
    let result = app.client.delete_space_category(&standard_id).await;
    assert_api_error(result, ApiError::SpaceCategoryInUse);

    // Un-referencing the category frees it for deletion.
    let space = app.client.get_space(&space_id).await?;
    let mut space_details = space.space_details;
    space_details.category_id = None;
    app.client
        .update_space(&requests::UpdateSpace {
            space_id,
            space_details,
        })
        .await?;
    app.client.delete_space_category(&standard_id).await?;

    let result = app
        .client
        .update_space_category(&requests::UpdateSpaceCategory {
            category_id: standard_id,
            name: "gone".into(),
        })
        .await;
    assert_api_error(result, ApiError::SpaceCategoryNotFound);

    Ok(())
}

#[tokio::test]
async fn test_space_category_community_scoping() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // A second community led by alice, with its own category.
    let other_community_id = app
        .client
        .create_community(&requests::CreateCommunity {
            name: "Other community".into(),
            description: None,
            currency: payloads::CurrencySettings {
                mode_config: test_helpers::default_currency_config(),
                name: "dollars".into(),
                symbol: "$".into(),
                minor_units: 2,
                balances_visible_to_members: true,
                new_members_default_active: true,
            },
        })
        .await?;
    let foreign_category_id = app
        .client
        .create_space_category(&payloads::SpaceCategory {
            community_id: other_community_id,
            name: "Foreign".into(),
        })
        .await?;

    // A space in community A cannot reference community B's category.
    let result = app
        .client
        .create_space(&one_point_space(
            site.site_id,
            "Booth A",
            Some(foreign_category_id),
        ))
        .await;
    assert_api_error(result, ApiError::SpaceCategoryCommunityMismatch);

    // Nor can an update, even one that also claims a community B site: the
    // check runs against the site the space actually lives in, and the
    // request's site id is ignored.
    let other_site = app.create_test_site(&other_community_id).await?;
    let space_id = app
        .client
        .create_space(&one_point_space(site.site_id, "Booth A", None))
        .await?;
    let result = app
        .client
        .update_space(&requests::UpdateSpace {
            space_id,
            space_details: one_point_space(
                other_site.site_id,
                "Booth A",
                Some(foreign_category_id),
            ),
        })
        .await;
    assert_api_error(result, ApiError::SpaceCategoryCommunityMismatch);
    let space = app.client.get_space(&space_id).await?;
    assert_eq!(space.space_details.site_id, site.site_id);
    assert_eq!(space.space_details.category_id, None);

    // Neither can a cap row in community A's capped auction.
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.capped = true;
    let auction_id = app.client.create_auction(&auction_details).await?;
    let members = app.client.get_members(&community_id).await?;
    let bob = &members
        .iter()
        .find(|m| m.user.username == "bob")
        .unwrap()
        .user;
    let result = app
        .client
        .set_bidder_cap(&requests::SetBidderCap {
            auction_id,
            user_id: bob.user_id,
            category_id: Some(foreign_category_id),
            points: 1.0,
        })
        .await;
    assert_api_error(result, ApiError::SpaceCategoryCommunityMismatch);

    Ok(())
}

#[tokio::test]
async fn test_capped_auction_bid_cap_enforcement() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let standard_id = app
        .client
        .create_space_category(&payloads::SpaceCategory {
            community_id,
            name: "Standard".into(),
        })
        .await?;
    let x1 = app
        .client
        .create_space(&one_point_space(site.site_id, "x1", Some(standard_id)))
        .await?;
    let x2 = app
        .client
        .create_space(&one_point_space(site.site_id, "x2", Some(standard_id)))
        .await?;
    let u1 = app
        .client
        .create_space(&one_point_space(site.site_id, "u1", None))
        .await?;
    let z0 = app
        .client
        .create_space(&payloads::Space {
            eligibility_points: 0.0,
            ..one_point_space(site.site_id, "z0", None)
        })
        .await?;

    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(app.time_source.now());
    auction_details.capped = true;
    let auction_id = app.client.create_auction(&auction_details).await?;

    let members = app.client.get_members(&community_id).await?;
    let bob = &members
        .iter()
        .find(|m| m.user.username == "bob")
        .unwrap()
        .user;

    app.tick().await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_0 = &rounds[0];

    // The cap check runs in round 0 (the eligibility check does not), and
    // a missing cap row means 0: bidding requires a cap.
    app.login_bob().await?;
    let result = app.client.create_bid(&x1, &round_0.round_id).await;
    assert_api_error(
        result,
        ApiError::ExceedsBidderCap {
            available: 0.0,
            required: 1.0,
            category: Some("Standard".into()),
        },
    );

    // A zero-point space bids fine with no cap row, mirroring the
    // zero-eligibility semantics.
    app.client.create_bid(&z0, &round_0.round_id).await?;

    // Members cannot edit caps.
    let result = app
        .client
        .set_bidder_cap(&requests::SetBidderCap {
            auction_id,
            user_id: bob.user_id,
            category_id: Some(standard_id),
            points: 1.0,
        })
        .await;
    assert_api_error(
        result,
        ApiError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        },
    );
    let result = app.client.list_bidder_caps(&auction_id).await;
    assert_api_error(
        result,
        ApiError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        },
    );

    app.login_alice().await?;
    app.client
        .set_bidder_cap(&requests::SetBidderCap {
            auction_id,
            user_id: bob.user_id,
            category_id: Some(standard_id),
            points: 1.0,
        })
        .await?;

    // Within the category cap.
    app.login_bob().await?;
    app.client.create_bid(&x1, &round_0.round_id).await?;

    // A second bid in the category exceeds it (current-round bids count).
    let result = app.client.create_bid(&x2, &round_0.round_id).await;
    assert_api_error(
        result,
        ApiError::ExceedsBidderCap {
            available: 1.0,
            required: 2.0,
            category: Some("Standard".into()),
        },
    );

    // The NULL bucket is separate and also requires its own cap row.
    let result = app.client.create_bid(&u1, &round_0.round_id).await;
    assert_api_error(
        result,
        ApiError::ExceedsBidderCap {
            available: 0.0,
            required: 1.0,
            category: None,
        },
    );

    app.login_alice().await?;
    app.client
        .set_bidder_cap(&requests::SetBidderCap {
            auction_id,
            user_id: bob.user_id,
            category_id: None,
            points: 1.0,
        })
        .await?;
    app.login_bob().await?;
    app.client.create_bid(&u1, &round_0.round_id).await?;

    // Repeating a bid in a now-full bucket reports the existing bid, not
    // a double-counted cap rejection.
    let result = app.client.create_bid(&u1, &round_0.round_id).await;
    assert_api_error(result, ApiError::AlreadyBidOnSpace);

    // Bob sees his own caps.
    let my_caps = app.client.my_bidder_caps(&auction_id).await?;
    assert_eq!(my_caps.len(), 2);
    assert!(my_caps.iter().all(|c| c.points == 1.0));

    // Advance to round 1: bob's standing wins consume his budgets.
    app.time_source
        .advance(auction_details.auction_params.round_duration);
    app.tick().await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let round_1 = &rounds[1];

    let result = app.client.create_bid(&x2, &round_1.round_id).await;
    assert_api_error(
        result,
        ApiError::ExceedsBidderCap {
            available: 1.0,
            required: 2.0,
            category: Some("Standard".into()),
        },
    );

    // Re-bidding a standing win in a full bucket reports the win itself,
    // not a double-counted cap rejection.
    let result = app.client.create_bid(&x1, &round_1.round_id).await;
    assert_api_error(result, ApiError::AlreadyWinningSpace);

    // Setting a cap to 0 deletes the row: a 0-points row and a missing row
    // mean the same thing at bid time.
    app.login_alice().await?;
    app.client
        .set_bidder_cap(&requests::SetBidderCap {
            auction_id,
            user_id: bob.user_id,
            category_id: None,
            points: 0.0,
        })
        .await?;
    let caps = app.client.list_bidder_caps(&auction_id).await?;
    assert_eq!(caps.len(), 1);
    assert_eq!(caps[0].category_id, Some(standard_id));

    Ok(())
}

#[tokio::test]
async fn test_cap_crud_requires_capped_auction() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    assert!(!auction_details.capped);
    let auction_id = app.client.create_auction(&auction_details).await?;

    let members = app.client.get_members(&community_id).await?;
    let bob = &members
        .iter()
        .find(|m| m.user.username == "bob")
        .unwrap()
        .user;

    let result = app
        .client
        .set_bidder_cap(&requests::SetBidderCap {
            auction_id,
            user_id: bob.user_id,
            category_id: None,
            points: 1.0,
        })
        .await;
    assert_api_error(result, ApiError::AuctionNotCapped);

    Ok(())
}

#[tokio::test]
async fn test_caps_frozen_after_conclusion() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    app.client
        .create_space(&one_point_space(site.site_id, "Booth A", None))
        .await?;

    let members = app.client.get_members(&community_id).await?;
    let bob = &members
        .iter()
        .find(|m| m.user.username == "bob")
        .unwrap()
        .user;

    // A capped auction run to conclusion with no bids.
    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.capped = true;
    auction_details.start_at = Some(app.time_source.now());
    let concluded_id = app.client.create_auction(&auction_details).await?;
    app.tick().await;
    loop {
        let rounds = app.client.list_auction_rounds(&concluded_id).await?;
        let latest = rounds.last().unwrap();
        app.time_source
            .set(latest.round_details.end_at + Span::new().seconds(1));
        app.tick().await;
        let auction = app.client.get_auction(&concluded_id).await?;
        if auction.end_at.is_some() {
            break;
        }
    }

    // A canceled capped auction, the other terminal state.
    let mut canceled_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    canceled_details.capped = true;
    let canceled_id = app.client.create_auction(&canceled_details).await?;
    app.client.cancel_auction(&canceled_id).await?;

    // Cap writes on either terminal auction are rejected; the cap rows
    // are its frozen historical record.
    for auction_id in [concluded_id, canceled_id] {
        let result = app
            .client
            .set_bidder_cap(&requests::SetBidderCap {
                auction_id,
                user_id: bob.user_id,
                category_id: None,
                points: 1.0,
            })
            .await;
        assert_api_error(result, ApiError::CapsFrozenAfterConclusion);
    }

    // Seeding into a terminal target is rejected too; the target check
    // runs before the source is examined.
    let result = app
        .client
        .seed_bidder_caps(&requests::SeedBidderCaps {
            target_auction_id: concluded_id,
            source_auction_id: canceled_id,
        })
        .await;
    assert_api_error(result, ApiError::CapsFrozenAfterConclusion);
    let result = app
        .client
        .seed_bidder_caps(&requests::SeedBidderCaps {
            target_auction_id: canceled_id,
            source_auction_id: concluded_id,
        })
        .await;
    assert_api_error(result, ApiError::CapsFrozenAfterConclusion);

    Ok(())
}

#[tokio::test]
async fn test_category_change_triggers_copy_on_write() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let standard_id = app
        .client
        .create_space_category(&payloads::SpaceCategory {
            community_id,
            name: "Standard".into(),
        })
        .await?;
    let space_id = app
        .client
        .create_space(&one_point_space(site.site_id, "Booth A", None))
        .await?;

    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.start_at = Some(app.time_source.now());
    let auction_id = app.client.create_auction(&auction_details).await?;
    app.tick().await;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;

    // With auction history, a category change is nontrivial: it selects
    // which cap bucket governs the space, so it must copy-on-write.
    let space = app.client.get_space(&space_id).await?;
    let mut space_details = space.space_details;
    space_details.category_id = Some(standard_id);
    let result = app
        .client
        .update_space(&requests::UpdateSpace {
            space_id,
            space_details,
        })
        .await?;
    assert!(result.was_copied);
    assert_eq!(result.old_space_id, Some(space_id));
    assert_eq!(result.space.space_details.category_id, Some(standard_id));

    Ok(())
}

#[tokio::test]
async fn test_seed_bidder_caps() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    let standard_id = app
        .client
        .create_space_category(&payloads::SpaceCategory {
            community_id,
            name: "Standard".into(),
        })
        .await?;

    // Source (booth-style) auction: two 1-point spaces in the category,
    // uncapped, both won by bob.
    let source_site = app.create_test_site(&community_id).await?;
    let b1 = app
        .client
        .create_space(&one_point_space(
            source_site.site_id,
            "Booth slot 1",
            Some(standard_id),
        ))
        .await?;
    let b2 = app
        .client
        .create_space(&one_point_space(
            source_site.site_id,
            "Booth slot 2",
            Some(standard_id),
        ))
        .await?;
    let source_auction_details =
        test_helpers::auction_details_a(source_site.site_id, &app.time_source);
    let source_auction_id =
        app.client.create_auction(&source_auction_details).await?;
    app.tick().await;
    let rounds = app.client.list_auction_rounds(&source_auction_id).await?;
    app.login_bob().await?;
    app.client.create_bid(&b1, &rounds[0].round_id).await?;
    app.client.create_bid(&b2, &rounds[0].round_id).await?;

    // Target (placement-style) capped auction on another site of the same
    // community, with a pre-committed manual cap of 1 for bob.
    app.login_alice().await?;
    let target_site_id = app
        .client
        .create_site(&test_helpers::site_details_b(community_id))
        .await?;
    let mut target_auction_details =
        test_helpers::auction_details_a(target_site_id, &app.time_source);
    target_auction_details.capped = true;
    target_auction_details.start_at = None;
    let target_auction_id =
        app.client.create_auction(&target_auction_details).await?;

    let members = app.client.get_members(&community_id).await?;
    let bob = &members
        .iter()
        .find(|m| m.user.username == "bob")
        .unwrap()
        .user;
    app.client
        .set_bidder_cap(&requests::SetBidderCap {
            auction_id: target_auction_id,
            user_id: bob.user_id,
            category_id: Some(standard_id),
            points: 1.0,
        })
        .await?;

    // Seeding from a still-ongoing source is rejected.
    let result = app
        .client
        .seed_bidder_caps(&requests::SeedBidderCaps {
            target_auction_id,
            source_auction_id,
        })
        .await;
    assert_api_error(result, ApiError::SeedSourceNotConcluded);

    // Run the source auction to conclusion.
    loop {
        let rounds = app.client.list_auction_rounds(&source_auction_id).await?;
        let latest = rounds.last().unwrap();
        app.time_source
            .set(latest.round_details.end_at + Span::new().seconds(1));
        app.tick().await;
        let auction = app.client.get_auction(&source_auction_id).await?;
        if auction.end_at.is_some() {
            break;
        }
    }

    // Additive apply on top of the manual cap: 1 + 2 won points = 3.
    app.client
        .seed_bidder_caps(&requests::SeedBidderCaps {
            target_auction_id,
            source_auction_id,
        })
        .await?;
    let caps = app.client.list_bidder_caps(&target_auction_id).await?;
    assert_eq!(caps.len(), 1);
    assert_eq!(caps[0].user_id, bob.user_id);
    assert_eq!(caps[0].category_id, Some(standard_id));
    assert_eq!(caps[0].points, 3.0);

    // A repeated apply adds again; the cap list is the safety net.
    app.client
        .seed_bidder_caps(&requests::SeedBidderCaps {
            target_auction_id,
            source_auction_id,
        })
        .await?;
    let caps = app.client.list_bidder_caps(&target_auction_id).await?;
    assert_eq!(caps[0].points, 5.0);

    // A canceled source is rejected even though its end_at is set.
    let canceled_details =
        test_helpers::auction_details_a(target_site_id, &app.time_source);
    let canceled_id = app.client.create_auction(&canceled_details).await?;
    app.client.cancel_auction(&canceled_id).await?;
    let result = app
        .client
        .seed_bidder_caps(&requests::SeedBidderCaps {
            target_auction_id,
            source_auction_id: canceled_id,
        })
        .await;
    assert_api_error(result, ApiError::SeedSourceNotConcluded);

    // Seeding into an uncapped target is rejected.
    let uncapped_details =
        test_helpers::auction_details_a(target_site_id, &app.time_source);
    let uncapped_id = app.client.create_auction(&uncapped_details).await?;
    let result = app
        .client
        .seed_bidder_caps(&requests::SeedBidderCaps {
            target_auction_id: uncapped_id,
            source_auction_id,
        })
        .await;
    assert_api_error(result, ApiError::AuctionNotCapped);

    Ok(())
}

#[tokio::test]
async fn test_seed_bidder_caps_skips_ex_members() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    let standard_id = app
        .client
        .create_space_category(&payloads::SpaceCategory {
            community_id,
            name: "Standard".into(),
        })
        .await?;

    // Source auction: alice and bob each win one 1-point space.
    let source_site = app.create_test_site(&community_id).await?;
    let s1 = app
        .client
        .create_space(&one_point_space(
            source_site.site_id,
            "Booth slot 1",
            Some(standard_id),
        ))
        .await?;
    let s2 = app
        .client
        .create_space(&one_point_space(
            source_site.site_id,
            "Booth slot 2",
            Some(standard_id),
        ))
        .await?;
    let source_auction_details =
        test_helpers::auction_details_a(source_site.site_id, &app.time_source);
    let source_auction_id =
        app.client.create_auction(&source_auction_details).await?;
    app.tick().await;
    let rounds = app.client.list_auction_rounds(&source_auction_id).await?;
    app.client.create_bid(&s1, &rounds[0].round_id).await?;
    app.login_bob().await?;
    app.client.create_bid(&s2, &rounds[0].round_id).await?;

    app.login_alice().await?;
    let members = app.client.get_members(&community_id).await?;
    let alice_id = members
        .iter()
        .find(|m| m.user.username == "alice")
        .unwrap()
        .user
        .user_id;
    let bob_id = members
        .iter()
        .find(|m| m.user.username == "bob")
        .unwrap()
        .user
        .user_id;

    // Run the source auction to conclusion.
    loop {
        let rounds = app.client.list_auction_rounds(&source_auction_id).await?;
        let latest = rounds.last().unwrap();
        app.time_source
            .set(latest.round_details.end_at + Span::new().seconds(1));
        app.tick().await;
        let auction = app.client.get_auction(&source_auction_id).await?;
        if auction.end_at.is_some() {
            break;
        }
    }

    // Bob leaves before the target auction's caps are seeded.
    app.client
        .remove_member(&requests::RemoveMember {
            community_id,
            member_user_id: bob_id,
        })
        .await?;

    let target_site_id = app
        .client
        .create_site(&test_helpers::site_details_b(community_id))
        .await?;
    let mut target_auction_details =
        test_helpers::auction_details_a(target_site_id, &app.time_source);
    target_auction_details.capped = true;
    target_auction_details.start_at = None;
    let target_auction_id =
        app.client.create_auction(&target_auction_details).await?;

    // Ex-member bob gets no row; remaining winner alice still seeds.
    app.client
        .seed_bidder_caps(&requests::SeedBidderCaps {
            target_auction_id,
            source_auction_id,
        })
        .await?;
    let caps = app.client.list_bidder_caps(&target_auction_id).await?;
    assert_eq!(caps.len(), 1);
    assert_eq!(caps[0].user_id, alice_id);
    assert_eq!(caps[0].category_id, Some(standard_id));
    assert_eq!(caps[0].points, 1.0);

    Ok(())
}

#[tokio::test]
async fn test_proxy_respects_caps() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;

    let standard_id = app
        .client
        .create_space_category(&payloads::SpaceCategory {
            community_id,
            name: "Standard".into(),
        })
        .await?;
    let x1 = app
        .client
        .create_space(&one_point_space(site.site_id, "x1", Some(standard_id)))
        .await?;
    let x2 = app
        .client
        .create_space(&one_point_space(site.site_id, "x2", Some(standard_id)))
        .await?;
    let x3 = app
        .client
        .create_space(&one_point_space(site.site_id, "x3", Some(standard_id)))
        .await?;

    let mut auction_details =
        test_helpers::auction_details_a(site.site_id, &app.time_source);
    auction_details.capped = true;
    auction_details.start_at = None;
    let auction_id = app.client.create_auction(&auction_details).await?;

    let members = app.client.get_members(&community_id).await?;
    let bob = &members
        .iter()
        .find(|m| m.user.username == "bob")
        .unwrap()
        .user;
    app.client
        .set_bidder_cap(&requests::SetBidderCap {
            auction_id,
            user_id: bob.user_id,
            category_id: Some(standard_id),
            points: 2.0,
        })
        .await?;

    // Bob values all three spaces and allows up to 3 items, but his cap
    // only fits 2: the proxy must select the two best-surplus spaces and
    // never attempt the third.
    app.login_bob().await?;
    for (space_id, value) in [(x1, 5), (x2, 4), (x3, 3)] {
        app.client
            .create_or_update_user_value(&requests::UserValue {
                space_id,
                value: Decimal::new(value, 0),
            })
            .await?;
    }
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 3,
        })
        .await?;

    app.login_alice().await?;
    app.client
        .schedule_auction(&requests::ScheduleAuction {
            auction_id,
            start_at: Some(app.time_source.now() + Span::new().seconds(1)),
        })
        .await?;
    app.time_source.advance(Span::new().seconds(2));
    app.tick().await;

    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.login_bob().await?;
    let bob_bids = app.client.list_bids(&rounds[0].round_id).await?;
    assert_eq!(bob_bids.len(), 2, "cap of 2 limits the proxy to 2 bids");
    let bid_spaces: std::collections::HashSet<payloads::SpaceId> =
        bob_bids.iter().map(|b| b.space_id).collect();
    assert!(bid_spaces.contains(&x1) && bid_spaces.contains(&x2));

    // Run to conclusion: standing wins keep consuming the budget, so bob
    // ends with exactly the two best spaces.
    loop {
        let rounds = app.client.list_auction_rounds(&auction_id).await?;
        let latest = rounds.last().unwrap();
        app.time_source
            .set(latest.round_details.end_at + Span::new().seconds(1));
        app.tick().await;
        let auction = app.client.get_auction(&auction_id).await?;
        if auction.end_at.is_some() {
            break;
        }
    }
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let results = app
        .client
        .list_round_space_results_for_round(&rounds.last().unwrap().round_id)
        .await?;
    let bob_wins: Vec<_> = results
        .iter()
        .filter(|r| r.winner.user_id == bob.user_id)
        .collect();
    assert_eq!(bob_wins.len(), 2);
    assert!(!bob_wins.iter().any(|r| r.space_id == x3));

    Ok(())
}
