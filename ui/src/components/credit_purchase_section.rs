use std::str::FromStr;

use payloads::{
    CommunityId, CurrencySettings, PurchaseKind, PurchaseStatus, requests,
    responses,
};
use rust_decimal::Decimal;
use yew::prelude::*;

use crate::components::CurrencyAmountInput;
use crate::get_api_client;
use crate::hooks::use_credit_purchases;
use crate::utils::checkout::{
    CheckoutReturn, checkout_return_banner, redirect_to_checkout,
    use_checkout_busy, use_checkout_return,
};
use crate::utils::styles::PRIMARY_BUTTON;
use crate::utils::time::{format_zoned_timestamp, localize_timestamp};

/// Whether top-up purchases are visible: the payloads deployment gate,
/// with debug builds (trunk dev) as an escape so the flow is testable
/// before the gate flips.
fn top_ups_enabled() -> bool {
    payloads::CREDIT_PURCHASES_ENABLED || cfg!(debug_assertions)
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    pub currency: CurrencySettings,
    /// Whether the community can charge cards (connected and
    /// charges-enabled); hides the purchase actions otherwise.
    pub card_payments_enabled: bool,
    pub currency_info: responses::MemberCurrencyInfo,
    /// Stripe's minimum charge for the community's denomination; the
    /// parent gates rendering on `backed_denomination()` so it always
    /// exists here.
    pub min_charge: Decimal,
    /// Refetch balances after a return from Checkout.
    pub on_balance_changed: Callback<()>,
}

/// Card payment actions for the currency page (backed_credits mode):
/// settling a negative balance (always available) and buying credits
/// (deployment-gated), plus the pending list for delayed methods (ACH)
/// and the return notice from Checkout redirects.
#[function_component]
pub fn CreditPurchaseSection(props: &Props) -> Html {
    let purchases = use_credit_purchases(props.community_id);
    let is_busy = use_checkout_busy();
    let error = use_state(|| None::<String>);
    let return_notice = use_checkout_return("purchase");

    // A success return means a payment just completed; card payments
    // issue within webhook latency, so refetch once on mount.
    {
        let on_balance_changed = props.on_balance_changed.clone();
        use_effect_with((), move |_| {
            if return_notice == Some(CheckoutReturn::Success) {
                on_balance_changed.emit(());
            }
        });
    }

    let effective =
        props.currency_info.balance + props.currency_info.pending_captures;
    let debt = Decimal::ZERO.max(-effective);

    let show_top_up = top_ups_enabled() && props.card_payments_enabled;
    let show_debt = debt > Decimal::ZERO && props.card_payments_enabled;

    html! {
        <div class="space-y-4">
            {checkout_return_banner(
                return_notice,
                "Payment received. Card payments post right away; bank \
                 payments can take a few days and will appear below \
                 until they complete.",
                "Checkout canceled — no payment was made.",
            )}
            {if show_debt {
                html! {
                    <DebtSettlement
                        community_id={props.community_id}
                        currency={props.currency.clone()}
                        debt={debt}
                        pending_captures={props.currency_info.pending_captures}
                        min_charge={props.min_charge}
                        is_busy={is_busy.clone()}
                        error={error.clone()}
                    />
                }
            } else {
                html! {}
            }}
            {if show_top_up {
                html! {
                    <TopUpForm
                        community_id={props.community_id}
                        currency={props.currency.clone()}
                        min_charge={props.min_charge}
                        is_busy={is_busy.clone()}
                        error={error.clone()}
                    />
                }
            } else {
                html! {}
            }}
            {if let Some(err) = (*error).clone() {
                html! {
                    <p class="text-sm text-red-600 dark:text-red-400">
                        {err}
                    </p>
                }
            } else {
                html! {}
            }}
            {purchases.inner.render(
                |list, _is_loading, _errors| {
                    render_pending_purchases(list, &props.currency)
                },
                || html! {},
                |errors: &[String]| html! {
                    <div class="space-y-1">
                        {for errors.iter().map(|err| html! {
                            <p class="text-sm text-red-600 \
                                      dark:text-red-400">
                                {format!(
                                    "Error loading payments in \
                                     progress: {err}"
                                )}
                            </p>
                        })}
                    </div>
                },
            )}
        </div>
    }
}

/// Start a purchase and redirect to the returned Checkout URL.
fn start_purchase(
    community_id: CommunityId,
    kind: PurchaseKind,
    amount: Decimal,
    is_busy: UseStateHandle<bool>,
    error: UseStateHandle<Option<String>>,
) {
    redirect_to_checkout(is_busy, error, async move {
        let request = requests::CreateCreditPurchase {
            community_id,
            kind,
            amount,
        };
        get_api_client().create_credit_purchase(&request).await
    });
}

#[derive(Properties, PartialEq)]
struct DebtProps {
    community_id: CommunityId,
    currency: CurrencySettings,
    debt: Decimal,
    pending_captures: Decimal,
    min_charge: Decimal,
    is_busy: UseStateHandle<bool>,
    error: UseStateHandle<Option<String>>,
}

/// Renders the settle-debt action: an exact-amount card payment that
/// clears the member's negative balance and nothing more, deliberately
/// separate from buying credits so settling never stores value.
#[function_component]
fn DebtSettlement(props: &DebtProps) -> Html {
    let below_min = props.debt < props.min_charge;
    let on_settle = {
        let community_id = props.community_id;
        let debt = props.debt;
        let is_busy = props.is_busy.clone();
        let error = props.error.clone();
        Callback::from(move |_: MouseEvent| {
            start_purchase(
                community_id,
                PurchaseKind::DebtSettlement,
                debt,
                is_busy.clone(),
                error.clone(),
            );
        })
    };

    html! {
        <div class="bg-white dark:bg-neutral-800 rounded-lg shadow p-6
                    space-y-3">
            <h3 class="font-medium text-neutral-900 dark:text-neutral-100">
                {"Outstanding balance"}
            </h3>
            <p class="text-sm text-neutral-600 dark:text-neutral-400">
                {format!(
                    "Your balance is {} short. You can settle it with a \
                     one-time card payment of exactly that amount; this \
                     only clears what you owe and does not add credits.",
                    props.currency.format_amount(props.debt),
                )}
            </p>
            {if props.pending_captures > Decimal::ZERO {
                html! {
                    <p class="text-sm text-neutral-600 dark:text-neutral-400">
                        {format!(
                            "{} in card charges from a recent auction is \
                             still being collected and already counts \
                             toward this amount.",
                            props.currency
                                .format_amount(props.pending_captures),
                        )}
                    </p>
                }
            } else {
                html! {}
            }}
            {if below_min {
                html! {
                    <p class="text-sm text-neutral-600 dark:text-neutral-400">
                        {"This amount is below the card processing \
                          minimum; a community leader can clear it \
                          directly."}
                    </p>
                }
            } else {
                html! {
                    <button
                        onclick={on_settle}
                        disabled={*props.is_busy}
                        class={PRIMARY_BUTTON}
                    >
                        {if *props.is_busy {
                            "Starting checkout...".to_string()
                        } else {
                            format!(
                                "Settle {}",
                                props.currency.format_amount(props.debt)
                            )
                        }}
                    </button>
                }
            }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct TopUpProps {
    community_id: CommunityId,
    currency: CurrencySettings,
    min_charge: Decimal,
    is_busy: UseStateHandle<bool>,
    error: UseStateHandle<Option<String>>,
}

/// Renders the buy-credits form: the member picks an amount (validated
/// against the card-processing minimum and the currency's quantization)
/// and completes the purchase on Stripe's checkout page.
#[function_component]
fn TopUpForm(props: &TopUpProps) -> Html {
    let amount_input = use_state(String::new);

    let parsed = Decimal::from_str(amount_input.trim()).ok();
    let valid = parsed.is_some_and(|amount| {
        amount >= props.min_charge
            && payloads::to_minor_units(amount, props.currency.minor_units)
                .is_some()
    });

    let on_buy = {
        let community_id = props.community_id;
        let is_busy = props.is_busy.clone();
        let error = props.error.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(amount) = parsed {
                start_purchase(
                    community_id,
                    PurchaseKind::TopUp,
                    amount,
                    is_busy.clone(),
                    error.clone(),
                );
            }
        })
    };

    html! {
        <div class="bg-white dark:bg-neutral-800 rounded-lg shadow p-6
                    space-y-3">
            <h3 class="font-medium text-neutral-900 dark:text-neutral-100">
                {"Buy credits"}
            </h3>
            <p class="text-sm text-neutral-600 dark:text-neutral-400">
                {format!(
                    "Credits are purchased at face value in {}. Refunds \
                     are at the community's discretion, and card \
                     processing fees may be deducted from refunds.",
                    props.currency.name,
                )}
            </p>
            <div class="flex gap-3 items-center">
                <CurrencyAmountInput
                    value={amount_input.clone()}
                    symbol={props.currency.symbol.clone()}
                    disabled={*props.is_busy}
                />
                <button
                    onclick={on_buy}
                    disabled={!valid || *props.is_busy}
                    class={PRIMARY_BUTTON}
                >
                    {if *props.is_busy {
                        "Starting checkout..."
                    } else {
                        "Buy credits"
                    }}
                </button>
            </div>
        </div>
    }
}

/// Purchases with a delayed payment method (ACH) still settling.
fn render_pending_purchases(
    list: &[responses::CreditPurchase],
    currency: &CurrencySettings,
) -> Html {
    let pending: Vec<_> = list
        .iter()
        .filter(|p| p.status == PurchaseStatus::Processing)
        .collect();
    if pending.is_empty() {
        return html! {};
    }
    html! {
        <div class="bg-white dark:bg-neutral-800 rounded-lg shadow p-6
                    space-y-2">
            <h3 class="font-medium text-neutral-900 dark:text-neutral-100">
                {"Payments in progress"}
            </h3>
            {for pending.iter().map(|p| {
                let started = format_zoned_timestamp(
                    &localize_timestamp(p.created_at, None),
                );
                let label = match p.kind {
                    PurchaseKind::TopUp => "credit purchase",
                    PurchaseKind::DebtSettlement => "balance settlement",
                };
                html! {
                    <p class="text-sm text-neutral-600 dark:text-neutral-400">
                        {format!(
                            "{} {} started {} — the bank payment is \
                             still processing; credits post when it \
                             completes.",
                            currency.format_amount(p.amount),
                            label,
                            started,
                        )}
                    </p>
                }
            })}
        </div>
    }
}
