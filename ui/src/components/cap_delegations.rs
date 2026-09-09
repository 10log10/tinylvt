//! Cap delegation surfaces for capped auctions: the bidder's own panel
//! (delegate cap, revoke what they gave, see what they received) and the
//! coleader's read-only list. Delegations are delegator-owned, so neither
//! the received list nor the admin list has controls: a coleader
//! overrides one by changing the delegator's cap in the cap editor.

use payloads::{
    AuctionId, CommunityId, Role, SpaceCategoryId, UserId, requests, responses,
};
use std::collections::HashMap;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

use crate::components::CategorySelect;
use crate::components::category_select::{category_name, format_points};
use crate::components::user_identity_display::{
    format_user_name_unambiguous, render_user_name,
};
use crate::hooks::{
    Fetch, render_section, stale_data_banner, use_cap_delegations, use_members,
    use_my_cap_delegations,
};
use crate::utils::styles::error_banner;

#[derive(Properties, PartialEq)]
pub struct PanelProps {
    pub auction: responses::Auction,
    pub community_id: CommunityId,
    pub current_user_id: UserId,
    /// Whether rounds exist. Delegations freeze at start (the server
    /// rejects writes too); the lists stay visible afterwards.
    pub started: bool,
    pub categories: Fetch<Vec<responses::SpaceCategory>>,
}

/// The bidder's delegation panel. Renders nothing for uncapped auctions;
/// the outer gate is a component boundary so the inner hooks don't fetch
/// in that case.
#[function_component]
pub fn CapDelegationPanel(props: &PanelProps) -> Html {
    if !props.auction.auction_details.capped {
        return html! {};
    }
    html! {
        <CapDelegationPanelInner
            auction_id={props.auction.auction_id}
            community_id={props.community_id}
            current_user_id={props.current_user_id}
            editable={!props.started && props.auction.end_at.is_none()}
            categories={props.categories.clone()}
        />
    }
}

#[derive(Properties, PartialEq)]
struct PanelInnerProps {
    auction_id: AuctionId,
    community_id: CommunityId,
    current_user_id: UserId,
    editable: bool,
    categories: Fetch<Vec<responses::SpaceCategory>>,
}

#[function_component]
fn CapDelegationPanelInner(props: &PanelInnerProps) -> Html {
    let delegations_hook = use_my_cap_delegations(props.auction_id);
    let members_hook = use_members(props.community_id);
    let error = use_state(|| None::<String>);

    let auction_id = props.auction_id;
    let me = props.current_user_id;

    // Own mutations refetch directly rather than waiting on the
    // `BidderCapsChanged` event the write also emits for this user.
    let on_set = {
        let error = error.clone();
        let refetch = delegations_hook.refetch.clone();
        Callback::from(move |details: requests::SetCapDelegation| {
            let error = error.clone();
            let refetch = refetch.clone();
            yew::platform::spawn_local(async move {
                error.set(None);
                let api_client = crate::get_api_client();
                match api_client.set_cap_delegation(&details).await {
                    Ok(_) => refetch.emit(()),
                    Err(e) => error.set(Some(e.to_string())),
                }
            });
        })
    };

    let on_delete = {
        let on_set = on_set.clone();
        Callback::from(move |d: responses::CapDelegation| {
            on_set.emit(requests::SetCapDelegation {
                auction_id: d.auction_id,
                to_user_id: d.to_user_id,
                category_id: d.category_id,
                points: 0.0,
            });
        })
    };

    let on_delegate = {
        let on_set = on_set.clone();
        Callback::from(
            move |(to_user_id, category_id, points): (
                UserId,
                Option<SpaceCategoryId>,
                f64,
            )| {
                on_set.emit(requests::SetCapDelegation {
                    auction_id,
                    to_user_id,
                    category_id,
                    points,
                });
            },
        )
    };

    let editable = props.editable;
    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-6 bg-white dark:bg-neutral-800 space-y-4">
            <h3 class="text-sm font-medium text-neutral-700 \
                       dark:text-neutral-300 uppercase tracking-wide">
                {"Cap Delegation"}
            </h3>
            <p class="text-xs text-neutral-500 dark:text-neutral-400">
                {if editable {
                    "Hand some of your cap to another member so they can \
                     bid for a group. A delegation only counts as far as \
                     your own assigned cap backs it, earliest first, and \
                     received cap can't be passed on. Delegations lock \
                     when the auction starts."
                } else {
                    "Delegations are locked once the auction starts."
                }}
            </p>

            {error.as_ref().map(|e| error_banner(e))}

            {render_section(
                &delegations_hook
                    .inner
                    .zip_ref(&members_hook.inner)
                    .zip_ref(&props.categories),
                "cap delegations",
                move |((delegations, members), categories), _, errors| {
                    let given: Vec<responses::CapDelegation> = delegations
                        .iter()
                        .filter(|d| d.from_user_id == me)
                        .cloned()
                        .collect();
                    let received: Vec<responses::CapDelegation> = delegations
                        .iter()
                        .filter(|d| d.to_user_id == me)
                        .cloned()
                        .collect();
                    html! {
                        <>
                            {stale_data_banner(errors)}
                            <DelegationGroup
                                title="Cap you've given"
                                empty="You haven't delegated any cap."
                                rows={given}
                                members={(*members).clone()}
                                categories={(*categories).clone()}
                                viewer={Some(me)}
                                on_revoke={editable.then(|| on_delete.clone())}
                            />
                            <DelegationGroup
                                title="Cap given to you"
                                empty="Nobody has delegated cap to you."
                                rows={received}
                                members={(*members).clone()}
                                categories={(*categories).clone()}
                                viewer={Some(me)}
                                on_revoke={
                                    None::<Callback<responses::CapDelegation>>
                                }
                            />
                            {if editable {
                                html! {
                                    <DelegateForm
                                        members={(*members).clone()}
                                        categories={(*categories).clone()}
                                        current_user_id={me}
                                        on_delegate={on_delegate.clone()}
                                    />
                                }
                            } else {
                                html! {}
                            }}
                        </>
                    }
                },
            )}
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct AdminListProps {
    pub auction: responses::Auction,
    pub community_id: CommunityId,
    pub user_role: Role,
    pub categories: Fetch<Vec<responses::SpaceCategory>>,
}

/// Read-only list of every delegation in the auction, for coleaders.
/// Renders nothing for other roles or uncapped auctions (component
/// boundary, so the coleader-only list request isn't made otherwise).
#[function_component]
pub fn CapDelegationsAdminList(props: &AdminListProps) -> Html {
    if !props.user_role.is_ge_coleader()
        || !props.auction.auction_details.capped
    {
        return html! {};
    }
    html! {
        <CapDelegationsAdminListInner
            auction_id={props.auction.auction_id}
            community_id={props.community_id}
            categories={props.categories.clone()}
        />
    }
}

#[derive(Properties, PartialEq)]
struct AdminListInnerProps {
    auction_id: AuctionId,
    community_id: CommunityId,
    categories: Fetch<Vec<responses::SpaceCategory>>,
}

#[function_component]
fn CapDelegationsAdminListInner(props: &AdminListInnerProps) -> Html {
    let delegations_hook = use_cap_delegations(props.auction_id);
    let members_hook = use_members(props.community_id);

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-6 bg-white dark:bg-neutral-800 space-y-4">
            <h3 class="text-sm font-medium text-neutral-700 \
                       dark:text-neutral-300 uppercase tracking-wide">
                {"Cap Delegations"}
            </h3>
            {render_section(
                &delegations_hook
                    .inner
                    .zip_ref(&members_hook.inner)
                    .zip_ref(&props.categories),
                "cap delegations",
                |((delegations, members), categories), _, errors| html! {
                    <>
                        {stale_data_banner(errors)}
                        <DelegationGroup
                            title=""
                            empty="No cap has been delegated in this auction."
                            rows={(*delegations).clone()}
                            members={(*members).clone()}
                            categories={(*categories).clone()}
                            viewer={None::<UserId>}
                            on_revoke={
                                None::<Callback<responses::CapDelegation>>
                            }
                        />
                    </>
                },
            )}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct DelegationGroupProps {
    title: AttrValue,
    empty: AttrValue,
    rows: Vec<responses::CapDelegation>,
    members: Vec<responses::CommunityMember>,
    categories: Vec<responses::SpaceCategory>,
    /// The viewing member, when the list is theirs: rows then name only
    /// the other party.
    viewer: Option<UserId>,
    /// Present only on the delegator's own editable list.
    on_revoke: Option<Callback<responses::CapDelegation>>,
}

/// One titled list of delegation rows with backing status.
#[function_component]
fn DelegationGroup(props: &DelegationGroupProps) -> Html {
    let member_names: HashMap<UserId, Html> = props
        .members
        .iter()
        .map(|m| (m.user.user_id, render_user_name(&m.user)))
        .collect();
    let name = |user_id: UserId| {
        member_names
            .get(&user_id)
            .cloned()
            .unwrap_or_else(|| html! { {"Former member"} })
    };

    let heading = if props.title.is_empty() {
        html! {}
    } else {
        html! {
            <h4 class="text-sm font-medium text-neutral-700 \
                       dark:text-neutral-300">
                {&props.title}
            </h4>
        }
    };

    if props.rows.is_empty() {
        return html! {
            <div class="space-y-1">
                {heading}
                <p class="text-sm text-neutral-500 dark:text-neutral-400">
                    {&props.empty}
                </p>
            </div>
        };
    }

    html! {
        <div class="space-y-1">
            {heading}
            <ul class="divide-y divide-neutral-200 dark:divide-neutral-700">
                {for props.rows.iter().map(|d| {
                    let parties = match props.viewer {
                        Some(me) if me == d.from_user_id => html! {
                            <>{"to "}{name(d.to_user_id)}</>
                        },
                        Some(me) if me == d.to_user_id => html! {
                            <>{"from "}{name(d.from_user_id)}</>
                        },
                        _ => html! {
                            <>
                                {name(d.from_user_id)}
                                {" to "}
                                {name(d.to_user_id)}
                            </>
                        },
                    };
                    let action = props.on_revoke.as_ref().map(|on_revoke| {
                        let on_revoke = on_revoke.clone();
                        let row = d.clone();
                        html! {
                            <button
                                type="button"
                                onclick={Callback::from(move |_| {
                                    on_revoke.emit(row.clone());
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
                                {"Revoke"}
                            </button>
                        }
                    });
                    html! {
                        <li
                            key={format!(
                                "{}-{}-{:?}",
                                d.from_user_id, d.to_user_id, d.category_id,
                            )}
                            class="py-2 flex items-center gap-3 flex-wrap"
                        >
                            <span class="flex-1 min-w-32 text-sm \
                                         text-neutral-900 \
                                         dark:text-neutral-100">
                                {parties}
                            </span>
                            <span class="text-sm text-neutral-600 \
                                         dark:text-neutral-400">
                                {category_name(
                                    d.category_id,
                                    &props.categories,
                                )}
                            </span>
                            <span class="text-sm text-neutral-900 \
                                         dark:text-neutral-100">
                                {format_points(d.points)}
                            </span>
                            <span class="text-xs text-neutral-500 \
                                         dark:text-neutral-400">
                                {backing_status(d)}
                            </span>
                            {action}
                        </li>
                    }
                })}
            </ul>
        </div>
    }
}

/// How much of a delegation the delegator's cap currently backs.
fn backing_status(d: &responses::CapDelegation) -> String {
    if d.backed >= d.points {
        "backed".into()
    } else if d.backed <= 0.0 {
        "not backed by a cap".into()
    } else {
        format!(
            "{} of {} backed",
            format_points(d.backed),
            format_points(d.points)
        )
    }
}

#[derive(Properties, PartialEq)]
struct DelegateFormProps {
    members: Vec<responses::CommunityMember>,
    categories: Vec<responses::SpaceCategory>,
    current_user_id: UserId,
    on_delegate: Callback<(UserId, Option<SpaceCategoryId>, f64)>,
}

#[function_component]
fn DelegateForm(props: &DelegateFormProps) -> Html {
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
        let on_delegate = props.on_delegate.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let member_select =
                member_select_ref.cast::<HtmlSelectElement>().unwrap();
            let Some(to_user_id) =
                member_select.value().parse().ok().map(UserId)
            else {
                return;
            };
            let points_input = points_ref.cast::<HtmlInputElement>().unwrap();
            let Ok(points) = points_input.value().parse::<f64>() else {
                return;
            };
            on_delegate.emit((to_user_id, *category, points));
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
                {for props
                    .members
                    .iter()
                    .filter(|m| m.user.user_id != props.current_user_id)
                    .map(|m| html! {
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
                {"Delegate"}
            </button>
        </form>
    }
}
