use payloads::{
    AuctionId, CommunityId, Role, SiteId, SpaceCategoryId, UserId, requests,
    responses,
};
use std::collections::HashMap;
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

use crate::components::category_select::{category_name, format_points};
use crate::components::user_identity_display::{
    format_user_name_unambiguous, render_user_name,
};
use crate::components::{CategorySelect, InlineEdit};
use crate::hooks::{
    Fetch, render_section, stale_data_banner, use_auctions, use_bidder_caps,
    use_members, use_sites,
};

/// Coleader editor for a capped auction's per-bidder caps: list, add,
/// edit, and remove cap rows, and apply (seed) caps from a concluded
/// auction's results. Caps are denominated in eligibility points; with
/// 1-point spaces they read as item counts. Renders nothing for other
/// roles or uncapped auctions — this outer gate is a component boundary,
/// so the inner hooks (all cap rows + member list) don't fetch at all in
/// those cases; the cap-list request is coleader-only and would fail for
/// members.
///
/// Mutations refetch the cap list directly: the `BidderCapsChanged` SSE
/// event is scoped to the affected bidder, not the editing coleader.
#[derive(Properties, PartialEq)]
pub struct Props {
    pub auction: responses::Auction,
    pub community_id: CommunityId,
    pub user_role: Role,
    /// The community's categories, fetched once by the auction page and
    /// shared across its cap surfaces.
    pub categories: Fetch<Vec<responses::SpaceCategory>>,
}

#[function_component]
pub fn BidderCapsEditor(props: &Props) -> Html {
    if !props.user_role.is_ge_coleader()
        || !props.auction.auction_details.capped
    {
        return html! {};
    }
    html! {
        <BidderCapsEditorInner
            auction={props.auction.clone()}
            community_id={props.community_id}
            categories={props.categories.clone()}
        />
    }
}

#[derive(Properties, PartialEq)]
struct InnerProps {
    auction: responses::Auction,
    community_id: CommunityId,
    categories: Fetch<Vec<responses::SpaceCategory>>,
}

#[function_component]
fn BidderCapsEditorInner(props: &InnerProps) -> Html {
    let caps_hook = use_bidder_caps(props.auction.auction_id);
    let members_hook = use_members(props.community_id);

    // Caps freeze at conclusion (the server rejects writes too): the
    // list stays visible as the auction's historical record, with the
    // edit affordances, add form, and seeding picker hidden.
    let ended = props.auction.end_at.is_some();

    let error = use_state(|| None::<String>);
    let success = use_state(|| None::<String>);

    let auction_id = props.auction.auction_id;

    let on_set_cap = {
        let error = error.clone();
        let success = success.clone();
        let refetch = caps_hook.refetch.clone();
        Callback::from(
            move |(user_id, category_id, points): (
                UserId,
                Option<SpaceCategoryId>,
                f64,
            )| {
                let error = error.clone();
                let success = success.clone();
                let refetch = refetch.clone();
                yew::platform::spawn_local(async move {
                    error.set(None);
                    success.set(None);
                    let api_client = crate::get_api_client();
                    match api_client
                        .set_bidder_cap(&requests::SetBidderCap {
                            auction_id,
                            user_id,
                            category_id,
                            points,
                        })
                        .await
                    {
                        Ok(_) => refetch.emit(()),
                        Err(e) => error.set(Some(e.to_string())),
                    }
                });
            },
        )
    };

    let on_set_cap_for_all = {
        let error = error.clone();
        let success = success.clone();
        let refetch = caps_hook.refetch.clone();
        Callback::from(
            move |(category_id, points): (Option<SpaceCategoryId>, f64)| {
                let error = error.clone();
                let success = success.clone();
                let refetch = refetch.clone();
                yew::platform::spawn_local(async move {
                    error.set(None);
                    success.set(None);
                    let api_client = crate::get_api_client();
                    match api_client
                        .set_bidder_cap_for_all(&requests::SetBidderCapForAll {
                            auction_id,
                            category_id,
                            points,
                        })
                        .await
                    {
                        Ok(_) => refetch.emit(()),
                        Err(e) => error.set(Some(e.to_string())),
                    }
                });
            },
        )
    };

    let on_seeded = {
        let success = success.clone();
        let refetch = caps_hook.refetch.clone();
        Callback::from(move |source_name: String| {
            success.set(Some(format!(
                "Added caps from the results of \"{}\". Applying the same \
                 source again adds them again.",
                source_name,
            )));
            refetch.emit(());
        })
    };

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-6 bg-white dark:bg-neutral-800 space-y-4">
            <h3 class="text-sm font-medium text-neutral-700 \
                       dark:text-neutral-300 uppercase tracking-wide">
                {"Bidder Caps"}
            </h3>
            <p class="text-xs text-neutral-500 dark:text-neutral-400">
                {if ended {
                    "The caps that governed this auction, frozen at its \
                     conclusion."
                } else {
                    "Each cap limits how many eligibility points of one \
                     category a bidder can hold active. A member without a \
                     cap for a category cannot bid on its spaces; \
                     uncategorized spaces have their own bucket. Caps can \
                     record pre-sold access directly, or carry over another \
                     auction's results below."
                }}
            </p>

            {if let Some(err) = &*error {
                html! {
                    <div class="p-3 rounded-md bg-red-50 dark:bg-red-900/20 \
                                border border-red-200 dark:border-red-800">
                        <p class="text-sm text-red-700 dark:text-red-400">
                            {err}
                        </p>
                    </div>
                }
            } else {
                html! {}
            }}
            {if let Some(msg) = &*success {
                html! {
                    <div class="p-3 rounded-md bg-green-50 \
                                dark:bg-green-900/20 border border-green-200 \
                                dark:border-green-800">
                        <p class="text-sm text-green-700 \
                                  dark:text-green-400">
                            {msg}
                        </p>
                    </div>
                }
            } else {
                html! {}
            }}

            {render_section(
                &caps_hook
                    .inner
                    .zip_ref(&members_hook.inner)
                    .zip_ref(&props.categories),
                "bidder caps",
                move |((caps, members), categories), _, errors| {
                    html! {
                        <>
                            {stale_data_banner(errors)}
                            <CapsList
                                caps={(*caps).clone()}
                                members={(*members).clone()}
                                categories={(*categories).clone()}
                                on_set_cap={on_set_cap.clone()}
                                read_only={ended}
                            />
                            {if ended {
                                html! {}
                            } else {
                                html! {
                                    <AddCapForm
                                        members={(*members).clone()}
                                        categories={(*categories).clone()}
                                        on_set_cap={on_set_cap.clone()}
                                        on_set_cap_for_all={
                                            on_set_cap_for_all.clone()
                                        }
                                    />
                                }
                            }}
                        </>
                    }
                },
            )}

            {if ended {
                html! {}
            } else {
                html! {
                    <div class="pt-4 border-t border-neutral-200 \
                                dark:border-neutral-700">
                        <SeedCapsControl
                            target_auction_id={auction_id}
                            community_id={props.community_id}
                            on_seeded={on_seeded}
                            on_error={Callback::from({
                                let error = error.clone();
                                move |e: String| error.set(Some(e))
                            })}
                        />
                    </div>
                }
            }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct CapsListProps {
    caps: Vec<responses::BidderCap>,
    members: Vec<responses::CommunityMember>,
    categories: Vec<responses::SpaceCategory>,
    /// Setting 0 points deletes the row (the backend's convention).
    on_set_cap: Callback<(UserId, Option<SpaceCategoryId>, f64)>,
    /// Ended auction: the rows are history, so no edit or remove.
    read_only: bool,
}

#[function_component]
fn CapsList(props: &CapsListProps) -> Html {
    if props.caps.is_empty() {
        return html! {
            <p class="text-sm text-neutral-500 dark:text-neutral-400">
                {if props.read_only {
                    "No caps were assigned in this auction."
                } else {
                    "No caps assigned yet. Nobody can bid in this auction \
                     until caps are added."
                }}
            </p>
        };
    }

    let member_names: HashMap<UserId, Html> = props
        .members
        .iter()
        .map(|m| (m.user.user_id, render_user_name(&m.user)))
        .collect();

    html! {
        <ul class="divide-y divide-neutral-200 dark:divide-neutral-700">
            {for props.caps.iter().map(|cap| {
                let user_id = cap.user_id;
                let category_id = cap.category_id;
                let on_set_cap = props.on_set_cap.clone();
                let on_remove_cap = props.on_set_cap.clone();
                let name = member_names
                    .get(&user_id)
                    .cloned()
                    .unwrap_or_else(|| html! { {"Former member"} });
                html! {
                    <li
                        key={format!("{}-{:?}", user_id, category_id)}
                        class="py-2 flex items-center gap-3 flex-wrap"
                    >
                        <span class="flex-1 min-w-32 text-sm \
                                     text-neutral-900 dark:text-neutral-100">
                            {name}
                        </span>
                        <span class="text-sm text-neutral-600 \
                                     dark:text-neutral-400">
                            {category_name(category_id, &props.categories)}
                        </span>
                        {if props.read_only {
                            html! {
                                <span class="w-16 text-right text-sm \
                                             text-neutral-900 \
                                             dark:text-neutral-100">
                                    {format_points(cap.points)}
                                </span>
                            }
                        } else {
                            html! {
                                <>
                                    <InlineEdit
                                        value={format_points(cap.points)}
                                        on_change={Callback::from(
                                            move |v: String| {
                                                let Ok(points) =
                                                    v.parse::<f64>()
                                                else {
                                                    return;
                                                };
                                                on_set_cap.emit((
                                                    user_id,
                                                    category_id,
                                                    points,
                                                ));
                                            },
                                        )}
                                        inputmode={
                                            AttrValue::Static("decimal")
                                        }
                                        display_class="w-16 text-right border \
                                            border-dashed border-neutral-400 \
                                            dark:border-neutral-500 \
                                            hover:bg-neutral-100 \
                                            dark:hover:bg-neutral-700"
                                        input_class="w-16 text-right"
                                    />
                                    <button
                                        type="button"
                                        onclick={Callback::from(move |_| {
                                            on_remove_cap.emit(
                                                (user_id, category_id, 0.0),
                                            );
                                        })}
                                        class="py-1.5 px-3 text-xs font-medium
                                               rounded-md border
                                               border-red-300
                                               dark:border-red-600
                                               text-red-700 dark:text-red-300
                                               bg-red-50 dark:bg-red-900/20
                                               hover:bg-red-100
                                               dark:hover:bg-red-900/30
                                               transition-colors duration-200"
                                    >
                                        {"Remove"}
                                    </button>
                                </>
                            }
                        }}
                    </li>
                }
            })}
        </ul>
    }
}

/// The member-select value for the bulk "every active member" target.
const ALL_ACTIVE_OPTION: &str = "all";

#[derive(Properties, PartialEq)]
struct AddCapFormProps {
    members: Vec<responses::CommunityMember>,
    categories: Vec<responses::SpaceCategory>,
    on_set_cap: Callback<(UserId, Option<SpaceCategoryId>, f64)>,
    /// The bulk target: one cap for every active member in the bucket.
    on_set_cap_for_all: Callback<(Option<SpaceCategoryId>, f64)>,
}

#[function_component]
fn AddCapForm(props: &AddCapFormProps) -> Html {
    let member_select_ref = use_node_ref();
    let points_ref = use_node_ref();
    let category = use_state(|| None::<SpaceCategoryId>);

    let select_classes = "px-3 py-2 border border-neutral-300 \
        dark:border-neutral-600 rounded-md shadow-sm bg-white \
        dark:bg-neutral-700 text-neutral-900 dark:text-neutral-100 text-sm \
        focus:outline-none focus:ring-2 focus:ring-neutral-500";

    let on_submit = {
        let member_select_ref = member_select_ref.clone();
        let points_ref = points_ref.clone();
        let category = category.clone();
        let on_set_cap = props.on_set_cap.clone();
        let on_set_cap_for_all = props.on_set_cap_for_all.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let member_select =
                member_select_ref.cast::<HtmlSelectElement>().unwrap();
            let points_input = points_ref.cast::<HtmlInputElement>().unwrap();
            let Ok(points) = points_input.value().parse::<f64>() else {
                return;
            };
            let target = member_select.value();
            if target == ALL_ACTIVE_OPTION {
                on_set_cap_for_all.emit((*category, points));
                return;
            }
            let Some(user_id) = target.parse().ok().map(UserId) else {
                return;
            };
            on_set_cap.emit((user_id, *category, points));
        })
    };

    html! {
        <form onsubmit={on_submit} class="flex gap-2 items-center flex-wrap">
            <select
                ref={member_select_ref}
                class={select_classes}
                required=true
            >
                <option value="" selected=true disabled=true>
                    {"Member"}
                </option>
                <option value={ALL_ACTIVE_OPTION}>
                    {"All active members"}
                </option>
                {for props.members.iter().map(|m| html! {
                    <option value={m.user.user_id.to_string()}>
                        {format_user_name_unambiguous(&m.user)}
                    </option>
                })}
            </select>
            <CategorySelect
                categories={props.categories.clone()}
                value={*category}
                on_change={Callback::from({
                    let category = category.clone();
                    move |id| category.set(id)
                })}
                class="text-sm"
            />
            <input
                ref={points_ref}
                type="number"
                step="any"
                min="0"
                placeholder="Points"
                required=true
                class="w-24 px-3 py-2 border border-neutral-300 \
                       dark:border-neutral-600 rounded-md shadow-sm bg-white \
                       dark:bg-neutral-700 text-neutral-900 \
                       dark:text-neutral-100 text-sm focus:outline-none \
                       focus:ring-2 focus:ring-neutral-500"
            />
            <button
                type="submit"
                class="py-2 px-4 rounded-md text-sm font-medium text-white
                       bg-neutral-900 hover:bg-neutral-800
                       dark:bg-neutral-100 dark:text-neutral-900
                       dark:hover:bg-neutral-200
                       transition-colors duration-200"
            >
                {"Set cap"}
            </button>
        </form>
    }
}

#[derive(Properties, PartialEq)]
struct SeedCapsControlProps {
    target_auction_id: AuctionId,
    community_id: CommunityId,
    /// Emitted with the source auction's display name after a successful
    /// apply.
    on_seeded: Callback<String>,
    on_error: Callback<String>,
}

/// Site picker for cap seeding. The dependent auctions fetch lives in
/// `SeedSourcePicker`, mounted only once a site is chosen (dependent
/// hooks require a component boundary).
#[function_component]
fn SeedCapsControl(props: &SeedCapsControlProps) -> Html {
    let sites_hook = use_sites(props.community_id);
    let selected_site = use_state(|| None::<SiteId>);
    let site_select_ref = use_node_ref();

    let on_site_change = {
        let selected_site = selected_site.clone();
        Callback::from(move |e: Event| {
            let select =
                e.target().unwrap().dyn_into::<HtmlSelectElement>().unwrap();
            selected_site.set(select.value().parse().ok());
        })
    };

    // Deselect the source site after a successful apply so the apply
    // button disappears: seeding is additive, so re-applying should take
    // a deliberate re-selection, not a second click. The DOM select is
    // reset directly: an option's `selected` attribute only sets its
    // default, and the browser keeps the user's own pick across
    // re-renders once they've interacted with the select.
    let on_seeded = {
        let selected_site = selected_site.clone();
        let site_select_ref = site_select_ref.clone();
        let on_seeded = props.on_seeded.clone();
        Callback::from(move |source_name: String| {
            if let Some(select) = site_select_ref.cast::<HtmlSelectElement>() {
                select.set_value("");
            }
            selected_site.set(None);
            on_seeded.emit(source_name);
        })
    };

    html! {
        <div class="space-y-3">
            <h4 class="text-sm font-medium text-neutral-700 \
                       dark:text-neutral-300">
                {"Carry over auction results"}
            </h4>
            <p class="text-xs text-neutral-500 dark:text-neutral-400">
                {"Adds each winner's final points per category in a \
                  concluded auction to their caps here. Additive: applying \
                  on top of existing caps increases them."}
            </p>
            {render_section(&sites_hook.inner, "sites", |sites, _, _| {
                html! {
                    <select
                        ref={site_select_ref.clone()}
                        onchange={on_site_change.clone()}
                        class="px-3 py-2 border border-neutral-300 \
                               dark:border-neutral-600 rounded-md shadow-sm \
                               bg-white dark:bg-neutral-700 text-neutral-900 \
                               dark:text-neutral-100 text-sm \
                               focus:outline-none focus:ring-2 \
                               focus:ring-neutral-500"
                    >
                        <option value="" selected={selected_site.is_none()}>
                            {"Choose the source auction's site"}
                        </option>
                        {for sites.iter().map(|site| {
                            let id_str = site.site_id.to_string();
                            html! {
                                <option
                                    value={id_str.clone()}
                                    selected={
                                        *selected_site == Some(site.site_id)
                                    }
                                >
                                    {&site.site_details.name}
                                </option>
                            }
                        })}
                    </select>
                }
            })}
            {if let Some(site_id) = *selected_site {
                html! {
                    // Keyed by site so a site change remounts the picker,
                    // resetting its selected-auction state instead of
                    // carrying over the previous site's pick.
                    <SeedSourcePicker
                        key={site_id.to_string()}
                        target_auction_id={props.target_auction_id}
                        site_id={site_id}
                        on_seeded={on_seeded.clone()}
                        on_error={props.on_error.clone()}
                    />
                }
            } else {
                html! {}
            }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct SeedSourcePickerProps {
    target_auction_id: AuctionId,
    site_id: SiteId,
    on_seeded: Callback<String>,
    on_error: Callback<String>,
}

#[function_component]
fn SeedSourcePicker(props: &SeedSourcePickerProps) -> Html {
    let auctions_hook = use_auctions(props.site_id);
    let selected_auction = use_state(|| None::<AuctionId>);
    let is_applying = use_state(|| false);

    let on_auction_change = {
        let selected_auction = selected_auction.clone();
        Callback::from(move |e: Event| {
            let select =
                e.target().unwrap().dyn_into::<HtmlSelectElement>().unwrap();
            selected_auction.set(select.value().parse().ok());
        })
    };

    render_section(&auctions_hook.inner, "auctions", move |auctions, _, _| {
        // Only a concluded (not canceled) auction has final results to
        // carry over.
        let sources: Vec<&responses::Auction> = auctions
            .iter()
            .filter(|a| {
                a.end_at.is_some()
                    && !a.was_canceled
                    && a.auction_id != props.target_auction_id
            })
            .collect();

        if sources.is_empty() {
            return html! {
                <p class="text-sm text-neutral-500 dark:text-neutral-400">
                    {"This site has no concluded auctions to carry over."}
                </p>
            };
        }

        let source_name = |auction: &responses::Auction| {
            auction.auction_details.name.clone().unwrap_or_else(|| {
                format!(
                    "Auction concluded {}",
                    auction
                        .end_at
                        .expect("sources are concluded")
                        .to_zoned(jiff::tz::TimeZone::system())
                        .strftime("%B %d, %Y")
                )
            })
        };

        let on_apply = {
            let selected_auction = selected_auction.clone();
            let is_applying = is_applying.clone();
            let on_seeded = props.on_seeded.clone();
            let on_error = props.on_error.clone();
            let target_auction_id = props.target_auction_id;
            let names: HashMap<AuctionId, String> = sources
                .iter()
                .map(|a| (a.auction_id, source_name(a)))
                .collect();
            Callback::from(move |_: MouseEvent| {
                let Some(source_auction_id) = *selected_auction else {
                    return;
                };
                let is_applying = is_applying.clone();
                let on_seeded = on_seeded.clone();
                let on_error = on_error.clone();
                // The picker remounts on site change, so the selection
                // always comes from this map; a miss is an internal
                // inconsistency, not a state to proceed from.
                let Some(name) = names.get(&source_auction_id).cloned() else {
                    on_error.emit(
                        "Internal error: the selected auction is not \
                         among this site's concluded auctions. Re-select \
                         the source and try again."
                            .into(),
                    );
                    return;
                };
                yew::platform::spawn_local(async move {
                    is_applying.set(true);
                    let api_client = crate::get_api_client();
                    match api_client
                        .seed_bidder_caps(&requests::SeedBidderCaps {
                            target_auction_id,
                            source_auction_id,
                        })
                        .await
                    {
                        Ok(_) => on_seeded.emit(name),
                        Err(e) => on_error.emit(e.to_string()),
                    }
                    is_applying.set(false);
                });
            })
        };

        html! {
            <div class="flex gap-2 items-center flex-wrap">
                <select
                    onchange={on_auction_change.clone()}
                    class="px-3 py-2 border border-neutral-300 \
                           dark:border-neutral-600 rounded-md shadow-sm \
                           bg-white dark:bg-neutral-700 text-neutral-900 \
                           dark:text-neutral-100 text-sm focus:outline-none \
                           focus:ring-2 focus:ring-neutral-500"
                >
                    <option value="" selected={selected_auction.is_none()}>
                        {"Choose a concluded auction"}
                    </option>
                    {for sources.iter().map(|auction| {
                        let id_str = auction.auction_id.to_string();
                        html! {
                            <option
                                value={id_str.clone()}
                                selected={
                                    *selected_auction
                                        == Some(auction.auction_id)
                                }
                            >
                                {source_name(auction)}
                            </option>
                        }
                    })}
                </select>
                <button
                    type="button"
                    onclick={on_apply}
                    disabled={selected_auction.is_none() || *is_applying}
                    class="py-2 px-4 rounded-md text-sm font-medium text-white
                           bg-neutral-900 hover:bg-neutral-800
                           dark:bg-neutral-100 dark:text-neutral-900
                           dark:hover:bg-neutral-200
                           disabled:opacity-50 disabled:cursor-not-allowed
                           transition-colors duration-200"
                >
                    {if *is_applying { "Applying..." } else { "Apply caps" }}
                </button>
            </div>
        }
    })
}
