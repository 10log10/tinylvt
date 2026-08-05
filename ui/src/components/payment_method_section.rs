use payloads::{requests, responses};
use yew::prelude::*;

use crate::components::ConfirmationModal;
use crate::get_api_client;
use crate::hooks::{render_section, use_payment_profile};
use crate::utils::capitalize;
use crate::utils::checkout::{
    CheckoutReturn, checkout_return_banner, redirect_to_checkout, run_action,
    use_checkout_busy, use_checkout_return,
};
use crate::utils::styles::{PRIMARY_BUTTON, SECONDARY_BUTTON, error_banner};

/// "Payment method" section for the profile page: saved-card display
/// with save/replace/remove, the merchant-initiated-charge consent
/// copy, the authorization hold-strategy setting, and the return notice
/// from card-setup Checkout redirects.
#[function_component]
pub fn PaymentMethodSection() -> Html {
    let profile_hook = use_payment_profile();
    let is_busy = use_checkout_busy();
    let error = use_state(|| None::<String>);
    let show_remove_modal = use_state(|| false);
    let remove_error = use_state(|| None::<String>);
    let setup_notice = use_checkout_return("card_setup");

    // A success return refetches the profile: the fetch's server-side
    // sync adopts the new card without waiting on webhook latency.
    {
        let refetch = profile_hook.refetch.clone();
        use_effect_with((), move |_| {
            if setup_notice == Some(CheckoutReturn::Success) {
                refetch.emit(());
            }
        });
    }

    let on_setup = {
        let is_busy = is_busy.clone();
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            redirect_to_checkout(is_busy.clone(), error.clone(), async {
                get_api_client().create_card_setup_session().await
            });
        })
    };

    let on_open_remove_modal = {
        let show_remove_modal = show_remove_modal.clone();
        let remove_error = remove_error.clone();
        Callback::from(move |_: MouseEvent| {
            remove_error.set(None);
            show_remove_modal.set(true);
        })
    };

    let on_close_remove_modal = {
        let show_remove_modal = show_remove_modal.clone();
        Callback::from(move |()| {
            show_remove_modal.set(false);
        })
    };

    let on_remove = {
        let is_busy = is_busy.clone();
        let remove_error = remove_error.clone();
        let show_remove_modal = show_remove_modal.clone();
        let refetch = profile_hook.refetch.clone();
        Callback::from(move |()| {
            let show_remove_modal = show_remove_modal.clone();
            let refetch = refetch.clone();
            run_action(
                is_busy.clone(),
                remove_error.clone(),
                async { get_api_client().remove_payment_method().await },
                move |result| match result {
                    Ok(()) => {
                        show_remove_modal.set(false);
                        refetch.emit(());
                        None
                    }
                    Err(e) => Some(e.to_string()),
                },
            );
        })
    };

    let on_strategy_change = {
        let is_busy = is_busy.clone();
        let error = error.clone();
        let refetch = profile_hook.refetch.clone();
        Callback::from(move |budget_holds: bool| {
            let refetch = refetch.clone();
            run_action(
                is_busy.clone(),
                error.clone(),
                async move {
                    get_api_client()
                        .update_hold_strategy(&requests::UpdateHoldStrategy {
                            budget_holds,
                        })
                        .await
                },
                move |result| match result {
                    Ok(()) => {
                        refetch.emit(());
                        None
                    }
                    Err(e) => Some(e.to_string()),
                },
            );
        })
    };

    let busy = *is_busy;
    html! {
        <div class="bg-white dark:bg-neutral-800 rounded-lg shadow-sm \
                    border border-neutral-200 dark:border-neutral-700 \
                    p-6 mb-8">
            <h2 class="text-lg font-semibold text-neutral-900 \
                       dark:text-neutral-100 mb-4">
                {"Payment Method"}
            </h2>

            {checkout_return_banner(
                setup_notice,
                "Card saved. Communities you approve can now place \
                 holds on it when your bids need backing beyond your \
                 balance.",
                "Card setup canceled — no card was saved.",
            )}

            {render_section(
                &profile_hook.inner,
                "payment settings",
                move |profile, _is_loading, _errors| html! {
                    <div class="space-y-6">
                        <CardPanel
                            card={profile.card.clone()}
                            busy={busy}
                            on_setup={on_setup.clone()}
                            on_remove={on_open_remove_modal.clone()}
                        />
                        <HoldStrategyPicker
                            budget_holds={profile.budget_holds}
                            busy={busy}
                            on_change={on_strategy_change.clone()}
                        />
                    </div>
                },
            )}

            if let Some(error) = &*error {
                <div class="mt-4">
                    {error_banner(error)}
                </div>
            }

            if *show_remove_modal {
                <ConfirmationModal
                    title="Remove Card"
                    message="This stops all further card activity: no new \
                             holds, and bids you're winning in active \
                             auctions can no longer be raised beyond what's \
                             already backed. Existing holds stay in place — \
                             winning bids settle normally and losing bids \
                             release their holds. You can save a card again \
                             at any time."
                    confirm_text="Remove Card"
                    on_confirm={on_remove}
                    on_close={on_close_remove_modal}
                    is_loading={*is_busy}
                    error_message={(*remove_error).clone().map(AttrValue::from)}
                    is_irreversible={false}
                />
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct CardPanelProps {
    card: Option<responses::SavedCard>,
    busy: bool,
    /// Starts the card-setup Checkout redirect (save or replace).
    on_setup: Callback<MouseEvent>,
    /// Opens the remove-card confirmation modal.
    on_remove: Callback<MouseEvent>,
}

/// The saved-card panel: the card summary with replace/remove controls,
/// or the save-a-card prompt with its consent copy.
#[function_component]
fn CardPanel(props: &CardPanelProps) -> Html {
    match &props.card {
        Some(card) => html! {
            <div class="space-y-3">
                <p class="text-sm text-neutral-900 \
                          dark:text-neutral-100">
                    {format!(
                        "{} •••• {}, expires {}/{}",
                        capitalize(&card.brand),
                        card.last4,
                        card.exp_month,
                        card.exp_year,
                    )}
                </p>
                <div class="flex gap-3">
                    <button
                        onclick={props.on_setup.clone()}
                        disabled={props.busy}
                        class={SECONDARY_BUTTON}
                    >
                        {"Replace Card"}
                    </button>
                    <button
                        onclick={props.on_remove.clone()}
                        disabled={props.busy}
                        class={SECONDARY_BUTTON}
                    >
                        {"Remove Card"}
                    </button>
                </div>
            </div>
        },
        None => html! {
            <div class="space-y-3">
                <p class="text-sm text-neutral-600 \
                          dark:text-neutral-400">
                    {"Save a card to bid beyond your credit balance in \
                      communities that enable card payments: holds are \
                      placed while you bid, and you only pay if you win. \
                      Saving a card authorizes communities you approve \
                      to place those holds and charges. Buying credits \
                      never requires a saved card."}
                </p>
                <button
                    onclick={props.on_setup.clone()}
                    disabled={props.busy}
                    class={PRIMARY_BUTTON}
                >
                    {if props.busy {
                        "Redirecting..."
                    } else {
                        "Save a Card"
                    }}
                </button>
            </div>
        },
    }
}

#[derive(Properties, PartialEq)]
struct HoldStrategyPickerProps {
    /// The profile's current setting: TRUE = budget holds, FALSE =
    /// minimum start.
    budget_holds: bool,
    busy: bool,
    /// Emits the newly selected `budget_holds` value.
    on_change: Callback<bool>,
}

/// The authorization-sizing setting: two radio cards choosing between
/// budget holds and minimum-start holds.
#[function_component]
fn HoldStrategyPicker(props: &HoldStrategyPickerProps) -> Html {
    let strategy_option = |value: bool, title: &str, description: &str| {
        let checked = props.budget_holds == value;
        let onchange = {
            let on_change = props.on_change.clone();
            Callback::from(move |_: Event| {
                on_change.emit(value);
            })
        };
        html! {
            <label class="flex items-start gap-3 p-3 rounded-md border \
                          border-neutral-200 dark:border-neutral-700 \
                          cursor-pointer">
                <input
                    type="radio"
                    name="hold-strategy"
                    checked={checked}
                    onchange={onchange}
                    disabled={props.busy}
                    class="mt-1"
                />
                <span>
                    <span class="block text-sm font-medium \
                                 text-neutral-900 \
                                 dark:text-neutral-100">
                        {title}
                    </span>
                    <span class="block text-sm text-neutral-600 \
                                 dark:text-neutral-400">
                        {description}
                    </span>
                </span>
            </label>
        }
    };

    html! {
        <div>
            <h3 class="text-sm font-semibold text-neutral-900 \
                       dark:text-neutral-100 mb-2">
                {"Authorization sizing"}
            </h3>
            <div class="space-y-2">
                {strategy_option(
                    true,
                    "Budget holds",
                    "One hold sized to your auction budget (your values \
                     and item limits). A single entry on your statement, \
                     no mid-auction card activity.",
                )}
                {strategy_option(
                    false,
                    "Minimum start",
                    "Start with a minimal hold that grows as your bids \
                     need it. Holds stay small when bidding stays low, \
                     but each increase replaces the hold on your \
                     statement.",
                )}
            </div>
        </div>
    }
}
