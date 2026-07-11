use payloads::{AuctionId, Role};
use yew::prelude::*;

use crate::components::user_identity_display::render_user_name;
use crate::hooks::{render_section, use_proxy_bidding_participants};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub auction_id: AuctionId,
    pub user_role: Role,
}

/// Coleader+ view of which members have enabled proxy bidding for an
/// auction that hasn't started yet. Lets leaders nudge interested members
/// who haven't opted in. Shows only who has enabled it, never per-space
/// values or the number of spaces a member intends to win. Renders nothing
/// for members and moderators.
///
/// Collapsed by default to conserve vertical space on the already-dense
/// auction page; the list is only fetched once expanded (the inner
/// component mounts on demand).
#[function_component]
pub fn ProxyBiddingParticipants(props: &Props) -> Html {
    if !props.user_role.is_ge_coleader() {
        return html! {};
    }

    let expanded = use_state(|| false);
    let on_toggle = {
        let expanded = expanded.clone();
        Callback::from(move |_| expanded.set(!*expanded))
    };

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg bg-white dark:bg-neutral-800">
            <button
                onclick={on_toggle}
                aria-expanded={(*expanded).to_string()}
                class="w-full flex items-center justify-between gap-3 p-4 \
                       text-left"
            >
                <span class="text-base font-medium text-neutral-900 \
                             dark:text-white">
                    {"Members using proxy bidding"}
                </span>
                <span class="text-neutral-500 dark:text-neutral-400 text-sm">
                    {if *expanded { "Hide" } else { "Show" }}
                </span>
            </button>
            {if *expanded {
                html! {
                    <div class="px-4 pb-4 space-y-3">
                        <p class="text-sm text-neutral-600 \
                                  dark:text-neutral-400">
                            {"Members who have enabled proxy bidding for this \
                              auction. Others may still be planning to \
                              participate; you can nudge them to set it up \
                              beforehand."}
                        </p>
                        <ParticipantsList auction_id={props.auction_id} />
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct ListProps {
    auction_id: AuctionId,
}

/// Fetches and renders the participant list. Separated so the fetch hook
/// only runs while the section is expanded.
#[function_component]
fn ParticipantsList(props: &ListProps) -> Html {
    let participants_hook = use_proxy_bidding_participants(props.auction_id);

    // This list is fetched once on mount and never refetched while
    // mounted (no SSE subscription, stable auction_id), so the on_value
    // closure only ever sees empty errors — no stale-data banner needed.
    render_section(
        &participants_hook.inner,
        "proxy bidding participants",
        |participants, _, _| render_participant_list(participants),
    )
}

fn render_participant_list(
    participants: &[payloads::responses::UserIdentity],
) -> Html {
    if participants.is_empty() {
        return html! {
            <p class="text-sm text-neutral-500 dark:text-neutral-400">
                {"No members have enabled proxy bidding yet."}
            </p>
        };
    }

    html! {
        <ul class="space-y-2">
            {for participants.iter().map(|user| html! {
                <li class="text-sm text-neutral-900 dark:text-neutral-100">
                    {render_user_name(user)}
                </li>
            })}
        </ul>
    }
}
