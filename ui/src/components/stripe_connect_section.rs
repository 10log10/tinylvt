use payloads::{CommunityId, StripeConnectStatus};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{render_section, use_community_stripe_status};
use crate::utils::checkout::{redirect_to_checkout, use_checkout_busy};
use crate::utils::styles::{PRIMARY_BUTTON, error_banner};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
}

/// Stripe Connect section for community settings (coleader+, shown only
/// in backed_credits mode). Connecting redirects to Stripe-hosted
/// onboarding; status is refetched when the member returns.
#[function_component]
pub fn StripeConnectSection(props: &Props) -> Html {
    let status_hook = use_community_stripe_status(props.community_id);
    let is_connecting = use_checkout_busy();
    let connect_error = use_state(|| None::<String>);

    let on_connect = {
        let community_id = props.community_id;
        let is_connecting = is_connecting.clone();
        let connect_error = connect_error.clone();
        Callback::from(move |_: MouseEvent| {
            redirect_to_checkout(
                is_connecting.clone(),
                connect_error.clone(),
                async move {
                    get_api_client()
                        .connect_community_stripe(&community_id)
                        .await
                },
            );
        })
    };

    let connect_button = |label: &'static str| {
        html! {
            <button
                onclick={on_connect.clone()}
                disabled={*is_connecting}
                class={PRIMARY_BUTTON}
            >
                {if *is_connecting { "Redirecting..." } else { label }}
            </button>
        }
    };

    html! {
        <div>
            <h2 class="text-xl font-semibold text-neutral-900 \
                       dark:text-neutral-100 mb-6">
                {"Card Payments"}
            </h2>

            {render_section(
                &status_hook.inner,
                "payment status",
                |status, _is_loading, _errors| {
                    match status.status {
                        StripeConnectStatus::NotConnected => html! {
                            <div class="space-y-4">
                                <p class="text-sm text-neutral-600 \
                                          dark:text-neutral-400">
                                    {"Connect a Stripe account to let members \
                                      pay by card. Your community becomes the \
                                      merchant of record with its own Stripe \
                                      dashboard, paying Stripe's standard \
                                      processing fees plus a 1% platform fee \
                                      on card payments."}
                                </p>
                                {connect_button("Connect Stripe Account")}
                            </div>
                        },
                        StripeConnectStatus::OnboardingIncomplete => html! {
                            <div class="space-y-4">
                                <p class="text-sm text-neutral-600 \
                                          dark:text-neutral-400">
                                    {"Stripe onboarding is incomplete or has \
                                      outstanding requirements; card payments \
                                      are not enabled yet."}
                                </p>
                                {connect_button("Continue Onboarding")}
                            </div>
                        },
                        StripeConnectStatus::ChargesEnabled => {
                            let mismatch_warning = status
                                .settlement_currency_mismatch
                                .as_ref()
                                .map(|currency| {
                                    format!(
                                        "The Stripe account settles in {}, \
                                         but this community's currency is \
                                         denominated differently. Fix the \
                                         account's settlement currency in \
                                         the Stripe dashboard before \
                                         enabling card-backed bidding.",
                                        currency.to_uppercase(),
                                    )
                                });
                            html! {
                            <div class="space-y-4">
                                <p class="text-sm text-neutral-600 \
                                          dark:text-neutral-400">
                                    {"Stripe is connected and card payments \
                                      are enabled."}
                                </p>
                                {match mismatch_warning {
                                    Some(warning) => error_banner(&warning),
                                    None => html! {},
                                }}
                            </div>
                            }
                        },
                        StripeConnectStatus::Disconnected => html! {
                            <div class="space-y-4">
                                <p class="text-sm text-neutral-600 \
                                          dark:text-neutral-400">
                                    {"TinyLVT is no longer authorized in your \
                                      Stripe dashboard, so automated card \
                                      payments are paused. Reconnect to \
                                      restore them."}
                                </p>
                                {connect_button("Reconnect Stripe Account")}
                            </div>
                        },
                    }
                },
            )}

            if let Some(error) = &*connect_error {
                <div class="mt-4">
                    {error_banner(error)}
                </div>
            }
        </div>
    }
}
