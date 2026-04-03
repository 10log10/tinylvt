use payloads::{CommunityId, responses::CommunityWithRole};
use yew::prelude::*;

use crate::components::{
    ActiveTab, BillingSection, CommunityPageWrapper, CommunityTabHeader,
    StorageUsageSection,
};
use crate::hooks::use_subscription_info;

#[derive(Clone, PartialEq)]
enum CheckoutStatus {
    Success,
    Canceled,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
}

#[function_component]
pub fn CommunityBillingPage(props: &Props) -> Html {
    let render_content = {
        let community_id = props.community_id;
        Callback::from(move |community: CommunityWithRole| {
            html! {
                <CommunityBillingContent
                    community={community}
                    community_id={community_id}
                />
            }
        })
    };

    html! {
        <CommunityPageWrapper
            community_id={props.community_id}
            children={render_content}
        />
    }
}

#[derive(Properties, PartialEq)]
struct ContentProps {
    pub community: CommunityWithRole,
    pub community_id: CommunityId,
}

/// Parse checkout status from URL query string.
fn parse_checkout_status() -> Option<CheckoutStatus> {
    let search = web_sys::window()?.location().search().unwrap_or_default();
    if search.contains("checkout=success") {
        Some(CheckoutStatus::Success)
    } else if search.contains("checkout=canceled") {
        Some(CheckoutStatus::Canceled)
    } else {
        None
    }
}

/// Remove query parameters from the current URL.
fn clean_query_params() {
    if let Some(window) = web_sys::window() {
        let _ = window.history().map(|h| {
            let path = window.location().pathname().unwrap_or_default();
            let _ = h.replace_state_with_url(
                &wasm_bindgen::JsValue::NULL,
                "",
                Some(&path),
            );
        });
    }
}

#[function_component]
fn CommunityBillingContent(props: &ContentProps) -> Html {
    let sub_hook = use_subscription_info(props.community_id);
    let checkout_status = use_state(|| None::<CheckoutStatus>);

    // Read ?checkout=success/canceled from URL on mount
    {
        let checkout_status = checkout_status.clone();
        let refetch = sub_hook.refetch.clone();
        use_effect_with((), move |_| {
            let status = parse_checkout_status();
            if status.is_some() {
                clean_query_params();
            }
            if matches!(status, Some(CheckoutStatus::Success)) {
                refetch.emit(());
            }
            checkout_status.set(status);
        });
    }

    // Poll for subscription data after successful checkout.
    // The webhook may not have fired yet when Stripe redirects
    // back.
    {
        let refetch = sub_hook.refetch.clone();
        let checkout_status = checkout_status.clone();
        let sub_data = sub_hook.data.clone();
        use_effect_with(checkout_status.clone(), move |status| {
            if **status != Some(CheckoutStatus::Success) {
                return;
            }
            // Stop polling once subscription data appears
            if let Some(Some(_)) = sub_data.as_ref() {
                return;
            }
            let refetch = refetch.clone();
            wasm_bindgen_futures::spawn_local(async move {
                for _ in 0..5 {
                    yew::platform::time::sleep(std::time::Duration::from_secs(
                        2,
                    ))
                    .await;
                    refetch.emit(());
                }
            });
        });
    }

    html! {
        <div>
            <CommunityTabHeader
                community={props.community.clone()}
                active_tab={ActiveTab::Billing}
            />

            <div class="py-6 space-y-8">
                // Checkout status banners
                {match &*checkout_status {
                    Some(CheckoutStatus::Success) => html! {
                        <div class="p-4 rounded-md bg-green-50 \
                                    dark:bg-green-900/20 border \
                                    border-green-200 \
                                    dark:border-green-800">
                            <p class="text-sm text-green-700 \
                                      dark:text-green-400">
                                {"Subscription activated \
                                  successfully!"}
                            </p>
                        </div>
                    },
                    Some(CheckoutStatus::Canceled) => html! {
                        <div class="p-4 rounded-md bg-neutral-50 \
                                    dark:bg-neutral-800 border \
                                    border-neutral-200 \
                                    dark:border-neutral-700">
                            <p class="text-sm text-neutral-600 \
                                      dark:text-neutral-400">
                                {"Checkout was canceled. No \
                                  changes were made."}
                            </p>
                        </div>
                    },
                    None => html! {},
                }}

                // Billing section
                {sub_hook.render(
                    "subscription",
                    |sub_info, is_loading, error| {
                        html! {
                            <div>
                                {if let Some(err) = error {
                                    html! {
                                        <div class="mb-4 p-4 \
                                                    rounded-md \
                                                    bg-red-50 \
                                                    dark:bg-red-900/20 \
                                                    border \
                                                    border-red-200 \
                                                    dark:border-red-800">
                                            <p class="text-sm \
                                                      text-red-700 \
                                                      dark:text-red-400">
                                                {err}
                                            </p>
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }}
                                <div class={classes!(
                                    is_loading.then_some(
                                        "opacity-50"
                                    )
                                )}>
                                    <BillingSection
                                        community_id={
                                            props.community_id
                                        }
                                        subscription={
                                            sub_info.clone()
                                        }
                                    />
                                </div>
                            </div>
                        }
                    },
                )}

                // Storage usage
                <StorageUsageSection
                    community_id={props.community_id}
                />
            </div>
        </div>
    }
}
