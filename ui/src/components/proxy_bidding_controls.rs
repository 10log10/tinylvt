use payloads::{AuctionId, CommunityId, requests};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{
    ProxyBiddingSettingsHookReturn, render_section, use_card_charge_grant,
    use_payment_profile,
};
use crate::utils::checkout::run_action;

#[derive(Properties, PartialEq)]
pub struct Props {
    /// The auction whose page hosts this control; carries the SSE stream
    /// the card-charge grant fetch subscribes to.
    pub auction_id: AuctionId,
    /// The proxy-bidding settings hook. The component reads is_enabled /
    /// max_items from the fetched settings and triggers update/delete via
    /// the hook's callbacks.
    pub settings: ProxyBiddingSettingsHookReturn,
    /// Show the card-charge grant control for this community
    /// (backed_credits mode).
    #[prop_or_default]
    pub card_grant_community: Option<CommunityId>,
}

#[function_component]
pub fn ProxyBiddingControls(props: &Props) -> Html {
    let auction_id = props.auction_id;
    let card_grant_community = props.card_grant_community;
    render_section(&props.settings.inner, "proxy bidding settings", {
        let update = props.settings.update.clone();
        let delete = props.settings.delete.clone();
        move |settings_opt: &Option<payloads::responses::UseProxyBidding>,
              _is_loading,
              _errors| {
            let is_enabled = settings_opt.is_some();
            let max_items =
                settings_opt.as_ref().map(|s| s.max_items).unwrap_or(1);
            html! {
                <ProxyBiddingControlsLoaded
                    auction_id={auction_id}
                    is_enabled={is_enabled}
                    max_items={max_items}
                    update={update.clone()}
                    delete={delete.clone()}
                    card_grant_community={card_grant_community}
                />
            }
        }
    })
}

#[derive(Properties, PartialEq)]
struct LoadedProps {
    auction_id: AuctionId,
    is_enabled: bool,
    max_items: i32,
    update: Callback<i32>,
    delete: Callback<()>,
    card_grant_community: Option<CommunityId>,
}

#[function_component]
fn ProxyBiddingControlsLoaded(props: &LoadedProps) -> Html {
    let max_items_input = use_state(|| props.max_items.to_string());
    let is_editing = use_state(|| false);

    // Reset input when the underlying max_items changes (e.g., after a save
    // round-trips and the hook updates).
    {
        let max_items_input = max_items_input.clone();
        let max_items = props.max_items;
        use_effect_with(max_items, move |max_items| {
            max_items_input.set(max_items.to_string());
        });
    }

    let on_input_change = {
        let max_items_input = max_items_input.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            max_items_input.set(input.value());
        })
    };

    let on_toggle_click = {
        let update = props.update.clone();
        let delete = props.delete.clone();
        let is_enabled = props.is_enabled;
        let max_items = props.max_items;
        Callback::from(move |_| {
            if is_enabled {
                delete.emit(());
            } else {
                update.emit(max_items);
            }
        })
    };

    let on_save_click = {
        let max_items_input = max_items_input.clone();
        let update = props.update.clone();
        let is_editing = is_editing.clone();
        Callback::from(move |_| {
            if let Ok(value) = (*max_items_input).parse::<i32>()
                && value > 0
            {
                update.emit(value);
                is_editing.set(false);
            }
        })
    };

    let on_edit_click = {
        let is_editing = is_editing.clone();
        Callback::from(move |_| {
            is_editing.set(true);
        })
    };

    let on_cancel_click = {
        let is_editing = is_editing.clone();
        let max_items_input = max_items_input.clone();
        let max_items = props.max_items;
        Callback::from(move |_| {
            max_items_input.set(max_items.to_string());
            is_editing.set(false);
        })
    };

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-6 bg-white dark:bg-neutral-800">
            <div class="space-y-4">
                <div class="flex items-center justify-between">
                    <h3 class="text-lg font-semibold text-neutral-900 \
                               dark:text-white">
                        {"Proxy Bidding"}
                    </h3>
                    <button
                        onclick={on_toggle_click}
                        class={format!(
                            "relative inline-flex h-6 w-11 items-center \
                             rounded-full transition-colors {}",
                            if props.is_enabled {
                                "bg-neutral-900 dark:bg-neutral-400"
                            } else {
                                "bg-neutral-300 dark:bg-neutral-600"
                            }
                        )}
                    >
                        <span class={format!(
                            "inline-block h-4 w-4 transform rounded-full \
                             bg-white transition-transform {}",
                            if props.is_enabled {
                                "translate-x-6"
                            } else {
                                "translate-x-1"
                            }
                        )} />
                    </button>
                </div>

                {if props.is_enabled {
                    html! {
                        <div class="space-y-3">
                            <p class="text-sm text-neutral-600 \
                                      dark:text-neutral-400">
                                {"Proxy bidding will automatically bid on your \
                                 behalf based on the maximum values you set for \
                                 each space."}
                            </p>

                            <div class="space-y-2">
                                <label class="block text-sm font-medium \
                                              text-neutral-700 \
                                              dark:text-neutral-300">
                                    {"Maximum Spaces to Win"}
                                </label>
                                {if *is_editing {
                                    html! {
                                        <div class="space-y-2">
                                            <input
                                                type="number"
                                                min="1"
                                                value={(*max_items_input).clone()}
                                                oninput={on_input_change}
                                                class="block w-full rounded-md \
                                                       border-neutral-300 \
                                                       dark:border-neutral-600 \
                                                       dark:bg-neutral-700 \
                                                       dark:text-white px-3 py-2 \
                                                       text-sm"
                                                placeholder="Enter max spaces"
                                            />
                                            <div class="flex gap-2">
                                                <button
                                                    onclick={on_save_click}
                                                    class="bg-neutral-900 \
                                                           hover:bg-neutral-800 \
                                                           dark:bg-neutral-100 \
                                                           dark:text-neutral-900 \
                                                           dark:hover:bg-neutral-200 \
                                                           text-white px-3 py-1.5 \
                                                           rounded-md text-sm \
                                                           font-medium \
                                                           transition-colors"
                                                >
                                                    {"Save"}
                                                </button>
                                                <button
                                                    onclick={on_cancel_click}
                                                    class="border border-neutral-300 \
                                                           dark:border-neutral-600 \
                                                           hover:bg-neutral-100 \
                                                           dark:hover:bg-neutral-700 \
                                                           px-3 py-1.5 rounded-md \
                                                           text-sm font-medium \
                                                           transition-colors"
                                                >
                                                    {"Cancel"}
                                                </button>
                                            </div>
                                        </div>
                                    }
                                } else {
                                    html! {
                                        <div class="flex items-center \
                                                    justify-between">
                                            <span class="text-2xl font-bold \
                                                         text-neutral-900 \
                                                         dark:text-white">
                                                {props.max_items}
                                            </span>
                                            <button
                                                onclick={on_edit_click}
                                                class="text-sm text-neutral-600 \
                                                       hover:text-neutral-900 \
                                                       dark:text-neutral-400 \
                                                       dark:hover:text-neutral-200 \
                                                       underline"
                                            >
                                                {"Edit"}
                                            </button>
                                        </div>
                                    }
                                }}
                                <p class="text-xs text-neutral-500 \
                                          dark:text-neutral-400">
                                    {"The proxy bidder will try to win up to this \
                                     many spaces, prioritizing those with the \
                                     highest surplus (value - price)."}
                                </p>
                            </div>
                        </div>
                    }
                } else {
                    html! {
                        <p class="text-sm text-neutral-600 \
                                  dark:text-neutral-400">
                            {"Enable proxy bidding to automatically bid on \
                             spaces based on your maximum values."}
                        </p>
                    }
                }}

                {if let Some(community_id) = props.card_grant_community {
                    html! {
                        <CardChargeGrantControls
                            auction_id={props.auction_id}
                            community_id={community_id}
                        />
                    }
                } else {
                    html! {}
                }}
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct GrantProps {
    auction_id: AuctionId,
    community_id: CommunityId,
}

/// View and change the community's permission to charge the member's
/// saved card. Revoking stops new authorizations only — holds already
/// backing bids release through settlement or cancellation. Hidden
/// while the member has no saved card — the grant is inert without one,
/// and the funding section already prompts to save a card.
#[function_component]
fn CardChargeGrantControls(props: &GrantProps) -> Html {
    let grant = use_card_charge_grant(props.auction_id, props.community_id);
    let profile = use_payment_profile();
    let is_busy = use_state(|| false);
    let error = use_state(|| None::<String>);

    let on_toggle = {
        let community_id = props.community_id;
        let is_busy = is_busy.clone();
        let error = error.clone();
        Callback::from(move |grant: requests::ChargeGrant| {
            // Refetch rides the CardChargeGrantChanged event the
            // update emits — it also refreshes the funding section.
            run_action(
                is_busy.clone(),
                error.clone(),
                async move {
                    get_api_client()
                        .update_card_charge_grant(
                            &requests::UpdateCardChargeGrant {
                                community_id,
                                grant,
                            },
                        )
                        .await
                },
                |result| result.err().map(|e| e.to_string()),
            );
        })
    };

    grant.inner.zip_ref(&profile.inner).render(
        |(granted, profile), _is_loading, _errors| {
            if profile.card.is_none() {
                return html! {};
            }
            let granted = **granted;
            let on_click = {
                let on_toggle = on_toggle.clone();
                Callback::from(move |_: MouseEvent| {
                    on_toggle.emit(if granted {
                        requests::ChargeGrant::Revoked
                    } else {
                        requests::ChargeGrant::Granted
                    });
                })
            };
            html! {
                <div class="pt-4 border-t border-neutral-200 \
                            dark:border-neutral-700 space-y-2">
                    <h4 class="text-sm font-medium text-neutral-900 \
                               dark:text-white">
                        {"Card charges"}
                    </h4>
                    <p class="text-sm text-neutral-600 \
                              dark:text-neutral-400">
                        {if granted {
                            "This community may charge your saved card \
                             when your bids need backing beyond your \
                             balance."
                        } else {
                            "This community cannot charge your saved \
                             card; bidding is limited to your balance."
                        }}
                    </p>
                    <button
                        onclick={on_click}
                        disabled={*is_busy}
                        class="border border-neutral-300 \
                               dark:border-neutral-600 \
                               hover:bg-neutral-100 \
                               dark:hover:bg-neutral-700 \
                               px-3 py-1.5 rounded-md text-sm font-medium \
                               transition-colors \
                               disabled:opacity-50 \
                               disabled:cursor-not-allowed"
                    >
                        {if granted {
                            "Revoke card charge permission"
                        } else {
                            "Allow card charges"
                        }}
                    </button>
                    {if granted {
                        html! {
                            <p class="text-xs text-neutral-500 \
                                      dark:text-neutral-400">
                                {"Revoking stops new card holds only; \
                                 holds already backing bids release when \
                                 the auction settles or is canceled."}
                            </p>
                        }
                    } else {
                        html! {}
                    }}
                    {if let Some(err) = (*error).clone() {
                        html! {
                            <p class="text-sm text-red-600 \
                                      dark:text-red-400">
                                {err}
                            </p>
                        }
                    } else {
                        html! {}
                    }}
                </div>
            }
        },
        || html! {},
        |errors: &[String]| {
            html! {
                <div class="pt-4 border-t border-neutral-200 \
                            dark:border-neutral-700 space-y-2">
                    {for errors.iter().map(|err| html! {
                        <p class="text-sm text-red-600 dark:text-red-400">
                            {format!(
                                "Error loading card charge permission: {err}"
                            )}
                        </p>
                    })}
                </div>
            }
        },
    )
}
