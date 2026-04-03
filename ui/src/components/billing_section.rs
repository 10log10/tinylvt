use payloads::{
    BillingInterval, CommunityId, SubscriptionInfo, SubscriptionStatus,
    requests,
};
use yew::prelude::*;

use crate::get_api_client;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    pub subscription: Option<SubscriptionInfo>,
}

#[function_component]
pub fn BillingSection(props: &Props) -> Html {
    let is_loading = use_state(|| false);
    let error = use_state(|| None::<String>);

    html! {
        <div>
            <h2 class="text-xl font-semibold text-neutral-900 \
                       dark:text-neutral-100 mb-6">
                {"Subscription"}
            </h2>

            // Error message
            {if let Some(err) = &*error {
                html! {
                    <div class="mb-4 p-4 rounded-md bg-red-50 \
                                dark:bg-red-900/20 border \
                                border-red-200 dark:border-red-800">
                        <p class="text-sm text-red-700 \
                                  dark:text-red-400">
                            {err}
                        </p>
                    </div>
                }
            } else {
                html! {}
            }}

            {match &props.subscription {
                Some(info)
                    if info.status
                        == SubscriptionStatus::PastDue =>
                {
                    html! {
                        <PaymentIssueView
                            community_id={props.community_id}
                            message="Your payment is past due. Please update your payment method to avoid losing access to paid features."
                            button_text="Update Payment Method"
                            is_loading={*is_loading}
                            set_loading={is_loading.clone()}
                            set_error={error.clone()}
                        />
                    }
                }
                Some(info)
                    if info.status
                        == SubscriptionStatus::Unpaid =>
                {
                    html! {
                        <PaymentIssueView
                            community_id={props.community_id}
                            message="Your subscription is suspended due to unpaid invoices. Pay the outstanding invoice to restore access to paid features."
                            button_text="Pay Outstanding Invoice"
                            is_loading={*is_loading}
                            set_loading={is_loading.clone()}
                            set_error={error.clone()}
                        />
                    }
                }
                Some(info)
                    if info.status
                        == SubscriptionStatus::Active =>
                {
                    html! {
                        <ActiveView
                            community_id={props.community_id}
                            info={info.clone()}
                            is_loading={*is_loading}
                            set_loading={is_loading.clone()}
                            set_error={error.clone()}
                        />
                    }
                }
                _ => {
                    // None or Canceled
                    html! {
                        <UpgradeView
                            community_id={props.community_id}
                            is_loading={*is_loading}
                            set_loading={is_loading.clone()}
                            set_error={error.clone()}
                        />
                    }
                }
            }}
        </div>
    }
}

// --- Upgrade view (free tier / canceled) ---

#[derive(Properties, PartialEq)]
struct UpgradeViewProps {
    community_id: CommunityId,
    is_loading: bool,
    set_loading: UseStateHandle<bool>,
    set_error: UseStateHandle<Option<String>>,
}

#[function_component]
fn UpgradeView(props: &UpgradeViewProps) -> Html {
    let on_upgrade = {
        let community_id = props.community_id;
        let set_loading = props.set_loading.clone();
        let set_error = props.set_error.clone();

        move |interval: BillingInterval| {
            let set_loading = set_loading.clone();
            let set_error = set_error.clone();

            set_loading.set(true);
            set_error.set(None);

            wasm_bindgen_futures::spawn_local(async move {
                let client = get_api_client();
                let request = requests::CreateCheckoutSession {
                    community_id,
                    billing_interval: interval,
                };

                match client.create_checkout_session(&request).await {
                    Ok(response) => {
                        if let Some(window) = web_sys::window() {
                            let _ = window
                                .location()
                                .set_href(&response.checkout_url);
                        }
                    }
                    Err(e) => {
                        set_error.set(Some(e.to_string()));
                        set_loading.set(false);
                    }
                }
            });
        }
    };

    let on_monthly = {
        let on_upgrade = on_upgrade.clone();
        Callback::from(move |_: MouseEvent| {
            on_upgrade(BillingInterval::Month);
        })
    };

    let on_annual = {
        Callback::from(move |_: MouseEvent| {
            on_upgrade(BillingInterval::Year);
        })
    };

    html! {
        <div class="space-y-4">
            <p class="text-neutral-600 dark:text-neutral-400">
                {"You're on the "}
                <span class="font-medium text-neutral-900 \
                             dark:text-neutral-100">
                    {"Free tier"}
                </span>
                {" (50 MB storage). Upgrade to unlock 2 GB \
                  storage."}
            </p>

            <div class="flex flex-col sm:flex-row gap-3">
                <button
                    onclick={on_monthly}
                    disabled={props.is_loading}
                    class="px-5 py-3 text-sm font-medium \
                           text-white \
                           bg-neutral-900 dark:bg-neutral-100 \
                           dark:text-neutral-900 rounded-md \
                           hover:bg-neutral-700 \
                           dark:hover:bg-neutral-300 \
                           transition-colors \
                           disabled:opacity-50 \
                           disabled:cursor-not-allowed"
                >
                    {if props.is_loading {
                        "Redirecting..."
                    } else {
                        "Monthly — $5/mo"
                    }}
                </button>
                <button
                    onclick={on_annual}
                    disabled={props.is_loading}
                    class="px-5 py-3 text-sm font-medium \
                           text-white \
                           bg-neutral-900 dark:bg-neutral-100 \
                           dark:text-neutral-900 rounded-md \
                           hover:bg-neutral-700 \
                           dark:hover:bg-neutral-300 \
                           transition-colors \
                           disabled:opacity-50 \
                           disabled:cursor-not-allowed"
                >
                    {if props.is_loading {
                        "Redirecting..."
                    } else {
                        "Annual — $50/yr (save ~17%)"
                    }}
                </button>
            </div>
        </div>
    }
}

// --- Active subscription view ---

#[derive(Properties, PartialEq)]
struct ActiveViewProps {
    community_id: CommunityId,
    info: SubscriptionInfo,
    is_loading: bool,
    set_loading: UseStateHandle<bool>,
    set_error: UseStateHandle<Option<String>>,
}

#[function_component]
fn ActiveView(props: &ActiveViewProps) -> Html {
    let on_manage = make_portal_callback(
        props.community_id,
        props.set_loading.clone(),
        props.set_error.clone(),
    );

    let period_end = props
        .info
        .current_period_end
        .to_zoned(jiff::tz::TimeZone::system())
        .strftime("%B %d, %Y")
        .to_string();

    html! {
        <div class="space-y-4">
            // Plan info
            <div class="flex flex-wrap items-baseline \
                        gap-x-4 gap-y-1">
                <span class="text-lg font-medium \
                             text-neutral-900 \
                             dark:text-neutral-100">
                    {"Paid tier"}
                </span>
                <span class="text-sm text-neutral-500 \
                             dark:text-neutral-400">
                    {props.info.billing_interval.display_name()}
                    {" plan"}
                </span>
            </div>

            // Canceling notice
            {if props.info.cancel_at_period_end {
                html! {
                    <div class="p-3 rounded-md bg-amber-50 \
                                dark:bg-amber-900/20 border \
                                border-amber-200 \
                                dark:border-amber-800">
                        <p class="text-sm text-amber-700 \
                                  dark:text-amber-400">
                            {"Your plan will end on "}
                            {&period_end}
                            {". You'll revert to the free \
                              tier after this date."}
                        </p>
                    </div>
                }
            } else {
                html! {
                    <p class="text-sm text-neutral-500 \
                              dark:text-neutral-400">
                        {"Renews on "}{&period_end}
                    </p>
                }
            }}

            // Manage button
            <button
                onclick={on_manage}
                disabled={props.is_loading}
                class="px-4 py-2 text-sm font-medium \
                       text-neutral-700 \
                       dark:text-neutral-300 \
                       bg-white dark:bg-neutral-800 \
                       border border-neutral-300 \
                       dark:border-neutral-600 rounded-md \
                       hover:bg-neutral-50 \
                       dark:hover:bg-neutral-700 \
                       transition-colors \
                       disabled:opacity-50 \
                       disabled:cursor-not-allowed"
            >
                {if props.is_loading {
                    "Redirecting..."
                } else {
                    "Manage Subscription"
                }}
            </button>
        </div>
    }
}

// --- Payment issue view (past due / unpaid) ---

#[derive(Properties, PartialEq)]
struct PaymentIssueViewProps {
    community_id: CommunityId,
    message: &'static str,
    button_text: &'static str,
    is_loading: bool,
    set_loading: UseStateHandle<bool>,
    set_error: UseStateHandle<Option<String>>,
}

#[function_component]
fn PaymentIssueView(props: &PaymentIssueViewProps) -> Html {
    let on_manage = make_portal_callback(
        props.community_id,
        props.set_loading.clone(),
        props.set_error.clone(),
    );

    html! {
        <div class="space-y-4">
            <div class="p-4 rounded-md bg-red-50 \
                        dark:bg-red-900/20 border \
                        border-red-200 dark:border-red-800">
                <p class="text-sm font-medium text-red-700 \
                          dark:text-red-400">
                    {props.message}
                </p>
            </div>

            <button
                onclick={on_manage}
                disabled={props.is_loading}
                class="px-4 py-2 text-sm font-medium \
                       text-white \
                       bg-red-600 dark:bg-red-700 \
                       rounded-md \
                       hover:bg-red-700 \
                       dark:hover:bg-red-600 \
                       transition-colors \
                       disabled:opacity-50 \
                       disabled:cursor-not-allowed"
            >
                {if props.is_loading {
                    "Redirecting..."
                } else {
                    props.button_text
                }}
            </button>
        </div>
    }
}

// --- Shared portal redirect logic ---

fn make_portal_callback(
    community_id: CommunityId,
    set_loading: UseStateHandle<bool>,
    set_error: UseStateHandle<Option<String>>,
) -> Callback<MouseEvent> {
    Callback::from(move |_: MouseEvent| {
        let set_loading = set_loading.clone();
        let set_error = set_error.clone();

        set_loading.set(true);
        set_error.set(None);

        wasm_bindgen_futures::spawn_local(async move {
            let client = get_api_client();
            let request = requests::CreatePortalSession { community_id };

            match client.create_portal_session(&request).await {
                Ok(response) => {
                    if let Some(window) = web_sys::window() {
                        let _ =
                            window.location().set_href(&response.checkout_url);
                    }
                }
                Err(e) => {
                    set_error.set(Some(e.to_string()));
                    set_loading.set(false);
                }
            }
        });
    })
}
