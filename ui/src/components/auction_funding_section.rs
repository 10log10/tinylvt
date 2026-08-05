use std::str::FromStr;

use payloads::{AuctionId, CommunityId, CurrencySettings, requests, responses};
use rust_decimal::Decimal;
use yew::prelude::*;
use yew_router::prelude::Link;

use crate::Route;
use crate::components::{CurrencyAmountInput, TimestampDisplay};
use crate::get_api_client;
use crate::hooks::{render_section, use_auction_funding};
use crate::utils::checkout::{
    checkout_return_banner, redirect_to_checkout, run_action,
    use_checkout_busy, use_checkout_return,
};
use crate::utils::styles::{SECONDARY_BUTTON, error_banner};

/// "Bid funding" section for the auction page (backed_credits mode): the
/// member's backing (balance allocation, live card hold with expiry), the
/// funding-regime indicator with in-context prompts for missing prerequisites,
/// and the pre-authorize control.
#[derive(Properties, PartialEq)]
pub struct Props {
    pub auction_id: AuctionId,
    pub community_id: CommunityId,
    pub currency: CurrencySettings,
    /// Whether the auction has ended (holds only release from here; no
    /// new authorizations).
    pub ended: bool,
}

#[function_component]
pub fn AuctionFundingSection(props: &Props) -> Html {
    let funding_hook = use_auction_funding(props.auction_id);
    let return_notice = use_checkout_return("funding");

    let auction_id = props.auction_id;
    let community_id = props.community_id;
    let currency = props.currency.clone();
    let ended = props.ended;

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-6 bg-white dark:bg-neutral-800">
            <h3 class="text-lg font-medium text-neutral-900 dark:text-white \
                       mb-4">
                {"Bid Funding"}
            </h3>
            {checkout_return_banner(
                return_notice,
                "Authorization complete. The card hold appears below as \
                 soon as Stripe confirms it, usually within seconds.",
                "Checkout canceled — no hold was placed.",
            )}
            {render_section(
                &funding_hook.inner,
                "funding",
                move |funding, _is_loading, _errors| html! {
                    <FundingDetails
                        auction_id={auction_id}
                        community_id={community_id}
                        currency={currency.clone()}
                        ended={ended}
                        funding={funding.clone()}
                    />
                },
            )}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct FundingDetailsProps {
    auction_id: AuctionId,
    community_id: CommunityId,
    currency: CurrencySettings,
    ended: bool,
    funding: responses::AuctionFunding,
}

/// Renders the fetched funding state: the backing stats, the contextual
/// hold/capture notes, and the regime indicator with its authorize
/// controls. Owns the busy/error state its actions share.
#[function_component]
fn FundingDetails(props: &FundingDetailsProps) -> Html {
    let is_busy = use_checkout_busy();
    let action_error = use_state(|| None::<String>);
    // Set when a saved-card authorize comes back demanding 3DS; the
    // unsaved-card checkout flow is the verification path. The
    // persisted counterpart (an off-session decline) arrives as
    // `funding.decline_pause`; either makes the regime indicator offer
    // the checkout form.
    let show_checkout_fallback = use_state(|| false);

    let funding = &props.funding;
    let currency = &props.currency;

    let on_grant = {
        let community_id = props.community_id;
        let is_busy = is_busy.clone();
        let action_error = action_error.clone();
        Callback::from(move |_: MouseEvent| {
            // Refetch rides the CardChargeGrantChanged event the
            // update emits — it also refreshes the card-charge
            // control in the proxy-bidding section.
            run_action(
                is_busy.clone(),
                action_error.clone(),
                async move {
                    get_api_client()
                        .update_card_charge_grant(
                            &requests::UpdateCardChargeGrant {
                                community_id,
                                grant: requests::ChargeGrant::Granted,
                            },
                        )
                        .await
                },
                |result| result.err().map(|e| e.to_string()),
            );
        })
    };

    let headroom = (funding.balance_backing + funding.authorized
        - funding.commitment)
        .max(Decimal::ZERO);
    // Balance actually committed here: the part of the commitment the
    // card hold doesn't cover, capped by what balance can deliver.
    let balance_committed = (funding.commitment - funding.authorized)
        .max(Decimal::ZERO)
        .min(funding.balance_backing);
    let uncovered_commitment =
        (funding.commitment - funding.balance_backing).max(Decimal::ZERO);

    let ctx = HoldContext {
        auction_id: props.auction_id,
        currency: currency.clone(),
        current_hold: funding.authorized,
        uncovered_commitment,
        is_busy: is_busy.clone(),
        error: action_error.clone(),
    };
    let checkout_form = html! {
        <CheckoutAuthorizeForm ctx={ctx.clone()} />
    };
    let authorize_controls = html! {
        <SavedCardAuthorizeControls
            ctx={ctx}
            preauth_target={funding.preauth_target}
            show_checkout_fallback={show_checkout_fallback.clone()}
        />
    };

    html! {
        <div class="space-y-4">
            <div class="grid grid-cols-2 sm:grid-cols-4 gap-4 text-sm">
                {funding_stat(
                    "Committed by bids",
                    currency.format_amount(funding.commitment),
                )}
                {funding_stat(
                    "Balance backing",
                    currency.format_amount(balance_committed),
                )}
                {funding_stat(
                    "Card hold",
                    currency.format_amount(funding.authorized),
                )}
                {funding_stat(
                    "Headroom",
                    currency.format_amount(headroom),
                )}
            </div>
            {match funding.capture_before {
                Some(expiry) if funding.authorized > Decimal::ZERO => html! {
                    <p class="text-sm text-neutral-600 \
                              dark:text-neutral-400">
                        {"Card hold expires "}
                        <TimestampDisplay timestamp={expiry} />
                        {". Winning bids settle from it; losing bids \
                          release it."}
                    </p>
                },
                _ => html! {},
            }}
            {capture_note(&funding.capture, currency)}
            {checkout_pending_note(funding.checkout_pending, currency)}
            {decline_pause_note(&funding.decline_pause)}
            <RegimeIndicator
                availability={funding.card_available}
                ended={props.ended}
                busy={*is_busy}
                offer_checkout={
                    *show_checkout_fallback
                        || funding.decline_pause.is_some()
                }
                on_grant={on_grant}
                authorize_controls={authorize_controls}
                checkout_form={checkout_form}
            />
            if let Some(error) = &*action_error {
                {error_banner(error)}
            }
        </div>
    }
}

/// Renders one label/value cell of the backing-stats grid.
fn funding_stat(label: &str, value: String) -> Html {
    html! {
        <div>
            <div class="text-neutral-500 dark:text-neutral-400">
                {label}
            </div>
            <div class="font-medium text-neutral-900 \
                        dark:text-neutral-100">
                {value}
            </div>
        </div>
    }
}

/// Renders the settlement card-charge note: what the member's card is
/// paying for this auction and where the payment stands.
fn capture_note(
    capture: &Option<payloads::responses::CaptureState>,
    currency: &CurrencySettings,
) -> Html {
    use payloads::FundingIntentStatus;

    let Some(capture) = capture else {
        return html! {};
    };
    let amount = currency.format_amount(capture.amount);
    let note_class = "text-sm text-neutral-600 dark:text-neutral-400";
    match capture.status {
        FundingIntentStatus::CapturePending => html! {
            <p class={note_class}>
                {format!(
                    "Card payment of {amount} is processing; the rest of \
                     your payment came from your credit balance."
                )}
            </p>
        },
        FundingIntentStatus::Captured => html! {
            <p class={note_class}>
                {format!("Paid {amount} by card.")}
            </p>
        },
        FundingIntentStatus::Failed => html! {
            <p class="text-sm text-red-700 dark:text-red-400">
                {format!(
                    "The card payment of {amount} could not be collected; \
                     the amount remains due on your credit balance."
                )}
            </p>
        },
        _ => html! {},
    }
}

/// Renders the note for an open checkout: an unsaved-card authorization
/// the member started but Stripe hasn't confirmed as paid yet.
fn checkout_pending_note(
    checkout_pending: Option<Decimal>,
    currency: &CurrencySettings,
) -> Html {
    let Some(amount) = checkout_pending else {
        return html! {};
    };
    html! {
        <p class="text-sm text-neutral-600 dark:text-neutral-400">
            {format!(
                "A {} card authorization is in progress. Complete it in \
                 the Stripe checkout tab if it's still open; starting a \
                 new authorization replaces it.",
                currency.format_amount(amount),
            )}
        </p>
    }
}

/// Renders the warning that the member's latest card-hold attempt was
/// declined (possibly off-session, via proxy raises), so automatic
/// holds are paused until a new authorization succeeds. The regime
/// indicator offers the unsaved-card checkout as the recovery path.
fn decline_pause_note(
    pause: &Option<payloads::responses::FundingDeclinePause>,
) -> Html {
    let Some(pause) = pause else {
        return html! {};
    };
    let message = match pause.code.as_deref() {
        Some("authentication_required") => {
            "A card hold was declined because your bank requires \
             verification. Automatic card holds are paused; authorize \
             below instead — Stripe runs the verification on its \
             checkout page."
                .to_string()
        }
        code => {
            let code_note = match code {
                Some(code) => format!(" (code: {code})"),
                None => String::new(),
            };
            format!(
                "A card hold was declined{code_note}. Automatic card \
                 holds are paused and bids fall back to your credit \
                 balance; authorize again below to resume card-backed \
                 bidding."
            )
        }
    };
    html! {
        <p class="text-sm text-red-700 dark:text-red-400">
            {message}
        </p>
    }
}

#[derive(Properties, PartialEq)]
struct RegimeIndicatorProps {
    availability: responses::CardAvailability,
    /// Whether the auction has ended (no new authorizations).
    ended: bool,
    busy: bool,
    /// Whether saved-card authorization is interrupted (a 3DS demand or
    /// a decline pause), so the checkout form shows as the recovery path
    /// even though a saved card is available.
    offer_checkout: bool,
    on_grant: Callback<MouseEvent>,
    authorize_controls: Html,
    checkout_form: Html,
}

/// Shows whether bids are balance-capped or card-extended, with the
/// in-context prompt for the first missing prerequisite. This is the
/// no-surprises guarantee, surfaced where bidding happens.
///
/// The unsaved-card checkout form shows whenever saved-card bidding
/// isn't fully lined up (no card, no grant, decline pause, or a 3DS
/// demand); the only fully closed door is a community that can't take
/// card payments at all.
#[function_component]
fn RegimeIndicator(props: &RegimeIndicatorProps) -> Html {
    use responses::CardAvailability;

    let note_class = "text-sm text-neutral-600 dark:text-neutral-400";
    match props.availability {
        CardAvailability::Available => html! {
            <div class="space-y-3">
                <p class={note_class}>
                    {"Bids beyond your balance place holds on your card. \
                      You only pay if you win."}
                </p>
                if !props.ended {
                    {props.authorize_controls.clone()}
                    if props.offer_checkout {
                        {props.checkout_form.clone()}
                    }
                }
            </div>
        },
        CardAvailability::CommunityNotChargesEnabled => html! {
            <p class={note_class}>
                {"Bids are limited by your credit balance in this \
                  community."}
            </p>
        },
        CardAvailability::NoSavedCard => html! {
            <div class="space-y-3">
                <p class={note_class}>
                    {"Bids are limited by your credit balance. "}
                    <Link<Route>
                        to={Route::Profile}
                        classes="underline text-neutral-900 \
                                 dark:text-neutral-100"
                    >
                        {"Save a card"}
                    </Link<Route>>
                    {" to bid beyond it, or place a one-time card hold \
                      below without saving anything."}
                </p>
                if !props.ended {
                    {props.checkout_form.clone()}
                }
            </div>
        },
        CardAvailability::NotGranted => html! {
            <div class="space-y-3">
                <p class={note_class}>
                    {"Bids are limited by your credit balance. To bid \
                      beyond it, allow this community to place holds on \
                      your saved card. Holds only settle for auctions you \
                      win; you can revoke this anytime."}
                </p>
                if !props.ended {
                    <button
                        onclick={props.on_grant.clone()}
                        disabled={props.busy}
                        class={SECONDARY_BUTTON}
                    >
                        {"Allow card holds"}
                    </button>
                    <p class={note_class}>
                        {"Or place a one-time hold below without granting \
                          ongoing access; you enter your card on Stripe's \
                          checkout page."}
                    </p>
                    {props.checkout_form.clone()}
                }
            </div>
        },
    }
}

/// The inputs the two hold-placement controls share: the auction and
/// currency their requests target, the replacement bounds a new hold
/// must respect, and the section's busy/error state.
#[derive(Clone, PartialEq)]
struct HoldContext {
    auction_id: AuctionId,
    currency: CurrencySettings,
    /// The live card hold's amount (zero when none). A new authorization
    /// replaces it: raise it, or reduce it no lower than the replacement
    /// floor — mirroring the backend's `HoldReplacementTooSmall` guard.
    current_hold: Decimal,
    /// The part of the member's commitment in this auction that their
    /// balance can't back, `max(0, commitment − balance backing)`; the
    /// lower bound for a reducing replacement.
    uncovered_commitment: Decimal,
    is_busy: UseStateHandle<bool>,
    error: UseStateHandle<Option<String>>,
}

impl HoldContext {
    /// Returns the smallest allowed replacement hold: any raise, or a
    /// reduction no lower than the balance-uncovered commitment.
    fn replacement_floor(&self) -> Decimal {
        self.uncovered_commitment.min(self.current_hold)
    }

    /// Parse and validate a hold amount against the denomination
    /// minimum, quantization, and the replacement floor. Returns the
    /// amount only when every check passes.
    fn parse_hold_amount(&self, input: &str) -> Option<Decimal> {
        let min_charge = payloads::denomination(&self.currency.name)
            .map(|d| d.stripe_min_charge)
            .unwrap_or(Decimal::ZERO);
        let amount = Decimal::from_str(input.trim()).ok()?;
        (amount >= min_charge
            && amount > Decimal::ZERO
            && amount >= self.replacement_floor()
            && payloads::to_minor_units(amount, self.currency.minor_units)
                .is_some())
        .then_some(amount)
    }
}

#[derive(Properties, PartialEq)]
struct SavedCardAuthorizeProps {
    ctx: HoldContext,
    /// The strategy-sized amount a request without an explicit amount
    /// would hold, from the funding response.
    preauth_target: Option<Decimal>,
    /// Set when the issuer demands 3DS; the parent then offers the
    /// checkout form as the verification path.
    show_checkout_fallback: UseStateHandle<bool>,
}

/// Renders the saved-card hold control. "Place hold now" resizes the
/// hold to the member's strategy target, or to an explicit amount,
/// raising or reducing the live hold to match; reductions are floored
/// at what committed bids need beyond balance.
#[function_component]
fn SavedCardAuthorizeControls(props: &SavedCardAuthorizeProps) -> Html {
    let amount_input = use_state(String::new);

    let input_empty = amount_input.trim().is_empty();
    let parsed = props.ctx.parse_hold_amount(amount_input.trim());
    let valid = input_empty || parsed.is_some();

    let on_authorize = {
        let ctx = props.ctx.clone();
        let show_checkout_fallback = props.show_checkout_fallback.clone();
        let amount_input = amount_input.clone();
        Callback::from(move |_: MouseEvent| {
            let auction_id = ctx.auction_id;
            let show_checkout_fallback = show_checkout_fallback.clone();
            let currency = ctx.currency.clone();
            let amount_input = amount_input.clone();
            // Refetch rides the FundingChanged event the activation
            // emits; a covered no-op changes nothing to refetch.
            run_action(
                ctx.is_busy.clone(),
                ctx.error.clone(),
                async move {
                    get_api_client()
                        .authorize_funding(&requests::AuthorizeFunding {
                            auction_id,
                            amount: parsed,
                        })
                        .await
                },
                move |result| match result {
                    Ok(()) => {
                        amount_input.set(String::new());
                        None
                    }
                    Err(e) => {
                        if is_authentication_required(&e) {
                            show_checkout_fallback.set(true);
                        }
                        Some(authorize_error_message(e, &currency))
                    }
                },
            );
        })
    };

    html! {
        <div class="space-y-2">
            <div class="flex gap-3 items-center">
                <CurrencyAmountInput
                    value={amount_input.clone()}
                    symbol={props.ctx.currency.symbol.clone()}
                    disabled={*props.ctx.is_busy}
                />
                <button
                    onclick={on_authorize}
                    disabled={!valid || *props.ctx.is_busy}
                    class={SECONDARY_BUTTON}
                >
                    {"Place hold now"}
                </button>
            </div>
            {authorize_help(props.preauth_target, &props.ctx)}
        </div>
    }
}

/// Whether a failed authorize was the issuer demanding 3D Secure — the
/// case the unsaved-card checkout flow resolves (Stripe runs the
/// verification on its page).
fn is_authentication_required(e: &payloads::ClientError) -> bool {
    matches!(
        e,
        payloads::ClientError::Api(
            _,
            payloads::ApiError::CardDeclined { code: Some(code) },
        ) if code == "authentication_required"
    )
}

/// Builds the contextual message for a failed authorize action. The
/// advance-gate error carries a timestamp, rendered in the member's timezone
/// rather than the variant's raw Display; the replacement-floor error carries
/// amounts, formatted in the community's currency.
fn authorize_error_message(
    e: payloads::ClientError,
    currency: &CurrencySettings,
) -> String {
    if is_authentication_required(&e) {
        return "Your bank requires verification for this hold. Use the \
                card authorization form instead — Stripe handles the \
                verification on its checkout page."
            .to_string();
    }
    match e {
        payloads::ClientError::Api(
            _,
            payloads::ApiError::PreauthNotYetOpen { authorize_from },
        ) => {
            let zoned =
                crate::utils::time::localize_timestamp(authorize_from, None);
            format!(
                "Too early to place a card hold for this auction; you can \
                 authorize starting {}",
                crate::utils::time::format_zoned_timestamp(&zoned),
            )
        }
        payloads::ClientError::Api(
            _,
            payloads::ApiError::HoldReplacementTooSmall { current, floor },
        ) => format!(
            "A replacement hold must raise your current {} hold, or stay \
             at or above {} so your committed bids stay backed",
            currency.format_amount(current),
            currency.format_amount(floor),
        ),
        e => e.to_string(),
    }
}

/// Renders the help line under the saved-card control: what leaving the
/// amount empty does (the strategy target, relative to the live hold)
/// and the bound on an explicit reduction.
fn authorize_help(target: Option<Decimal>, ctx: &HoldContext) -> Html {
    let floor = ctx.replacement_floor();
    let floor_note =
        if ctx.current_hold > Decimal::ZERO && floor > Decimal::ZERO {
            format!(
                "; reductions must stay at or above {} so your committed \
             bids stay backed",
                ctx.currency.format_amount(floor),
            )
        } else {
            String::new()
        };
    let text = match target {
        Some(target) if ctx.current_hold == Decimal::ZERO => format!(
            "Leave the amount empty to size the hold by your strategy: \
             {}.",
            ctx.currency.format_amount(target),
        ),
        Some(target) if target != ctx.current_hold => format!(
            "Leave the amount empty to resize to your strategy target, \
             {}; or enter an amount to set the hold yourself{floor_note}.",
            ctx.currency.format_amount(target),
        ),
        Some(_) => format!(
            "Your hold matches your strategy target. Enter an amount to \
             resize it yourself{floor_note}."
        ),
        None => format!(
            "Leave the amount empty to size the hold by your strategy, \
             or enter an amount{floor_note}."
        ),
    };
    html! {
        <p class="text-sm text-neutral-500 dark:text-neutral-400">
            {text}
        </p>
    }
}

#[derive(Properties, PartialEq)]
struct CheckoutFormProps {
    ctx: HoldContext,
}

/// Renders the unsaved-card authorization form. The member picks their
/// maximum spend for this auction and completes the hold on Stripe's
/// checkout page (card entry and any 3DS included); nothing is saved.
/// The hold caps their card-backed bidding; completing another checkout
/// replaces it, raising the hold or reducing it down to no less than
/// what the member's committed bids need beyond their balance.
#[function_component]
fn CheckoutAuthorizeForm(props: &CheckoutFormProps) -> Html {
    let amount_input = use_state(String::new);

    let replacement_floor = props.ctx.replacement_floor();
    let parsed = props.ctx.parse_hold_amount(amount_input.trim());

    let on_checkout = {
        let ctx = props.ctx.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(amount) = parsed {
                let auction_id = ctx.auction_id;
                redirect_to_checkout(
                    ctx.is_busy.clone(),
                    ctx.error.clone(),
                    async move {
                        get_api_client()
                            .checkout_funding(&requests::CheckoutFunding {
                                auction_id,
                                amount,
                            })
                            .await
                    },
                );
            }
        })
    };

    let description = {
        let base = "Choose the most you'd spend in this auction; the \
                    hold caps your bidding. You only pay if you win, \
                    and the unused amount is released when the auction \
                    settles.";
        if props.ctx.current_hold > Decimal::ZERO {
            let hold = props.ctx.currency.format_amount(props.ctx.current_hold);
            if replacement_floor > Decimal::ZERO {
                format!(
                    "{base} This replaces your current {hold} hold — \
                     raise it, or reduce it to no less than {} so your \
                     committed bids stay backed.",
                    props.ctx.currency.format_amount(replacement_floor),
                )
            } else {
                format!(
                    "{base} This replaces your current {hold} hold; \
                     nothing committed depends on it, so you can raise \
                     or reduce it freely.",
                )
            }
        } else {
            base.to_string()
        }
    };

    html! {
        <div class="space-y-2">
            <p class="text-sm text-neutral-600 dark:text-neutral-400">
                {description}
            </p>
            <div class="flex gap-3 items-center">
                <CurrencyAmountInput
                    value={amount_input.clone()}
                    symbol={props.ctx.currency.symbol.clone()}
                    disabled={*props.ctx.is_busy}
                />
                <button
                    onclick={on_checkout}
                    disabled={parsed.is_none() || *props.ctx.is_busy}
                    class={SECONDARY_BUTTON}
                >
                    {if *props.ctx.is_busy {
                        "Starting checkout..."
                    } else {
                        "Authorize with card"
                    }}
                </button>
            </div>
        </div>
    }
}
