use crate::Route;
use crate::hooks::{use_push_route, use_title};
use std::collections::HashMap;
use yew::prelude::*;
use yew_router::prelude::*;

#[function_component]
pub fn PricingPage() -> Html {
    use_title("Pricing - TinyLVT");
    let push_route = use_push_route();
    let navigator = use_navigator().unwrap();

    let on_sign_up = {
        let navigator = navigator.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let mut query = HashMap::new();
            query.insert("signup".to_string(), "true".to_string());
            let _ = navigator.push_with_query(&Route::Login, &query);
        })
    };

    let on_learn_more = {
        let push_route = push_route.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            push_route.emit(Route::Docs);
        })
    };

    html! {
        <div class="max-w-4xl mx-auto px-4 py-12 space-y-12">
            // Header
            <div class="text-center space-y-4">
                <h1 class="text-4xl font-bold \
                           text-neutral-900 \
                           dark:text-neutral-100">
                    {"Simple pricing"}
                </h1>
                <p class="text-lg text-neutral-600 \
                          dark:text-neutral-400 max-w-xl \
                          mx-auto">
                    {"Pricing is per community. Full \
                      functionality on every plan — no \
                      feature gates, no member limits. \
                      Pay only when a community needs \
                      more storage."}
                </p>
            </div>

            // Pricing cards
            <div class="grid grid-cols-1 md:grid-cols-2 \
                        gap-6 max-w-3xl mx-auto">
                // Free tier
                <div class="bg-white dark:bg-neutral-800 \
                            border border-neutral-200 \
                            dark:border-neutral-700 \
                            rounded-lg p-6 space-y-4">
                    <h2 class="text-xl font-semibold \
                               text-neutral-900 \
                               dark:text-neutral-100">
                        {"Free"}
                    </h2>
                    <div>
                        <div class="flex items-baseline gap-1">
                            <span class="text-3xl font-bold \
                                         text-neutral-900 \
                                         dark:text-neutral-100">
                                {"$0"}
                            </span>
                            <span class="text-neutral-500 \
                                         dark:text-neutral-400">
                                {"/mo per community"}
                            </span>
                        </div>
                        // Spacer to match paid card's
                        // annual note line
                        <p class="text-sm mt-1"
                           aria-hidden="true">
                            {"\u{00a0}"}
                        </p>
                    </div>
                    <p class="text-sm text-neutral-600 \
                              dark:text-neutral-400">
                        {"Every community starts here."}
                    </p>
                    <ul class="space-y-2 text-sm \
                               text-neutral-700 \
                               dark:text-neutral-300">
                        <PricingFeature
                            text="50 MB storage" />
                        <PricingFeature
                            text="Unlimited members" />
                        <PricingFeature
                            text="Unlimited auctions" />
                        <PricingFeature
                            text="All features included" />
                    </ul>
                    <button
                        onclick={on_sign_up.clone()}
                        class="w-full px-4 py-2.5 text-sm \
                               font-medium \
                               text-neutral-700 \
                               dark:text-neutral-300 \
                               bg-white dark:bg-neutral-800 \
                               border border-neutral-300 \
                               dark:border-neutral-600 \
                               rounded-md \
                               hover:bg-neutral-50 \
                               dark:hover:bg-neutral-700 \
                               transition-colors"
                    >
                        {"Get started"}
                    </button>
                </div>

                // Paid tier
                <div class="bg-white dark:bg-neutral-800 \
                            border-2 border-neutral-900 \
                            dark:border-neutral-100 \
                            rounded-lg p-6 space-y-4">
                    <h2 class="text-xl font-semibold \
                               text-neutral-900 \
                               dark:text-neutral-100">
                        {"Paid"}
                    </h2>
                    <div>
                        <div class="flex items-baseline \
                                    gap-1">
                            <span class="text-3xl font-bold \
                                         text-neutral-900 \
                                         dark:text-neutral-100">
                                {"$5"}
                            </span>
                            <span class="text-neutral-500 \
                                         dark:text-neutral-400">
                                {"/mo per community"}
                            </span>
                        </div>
                        <p class="text-sm text-neutral-500 \
                                  dark:text-neutral-400 mt-1">
                            {"or $50/year (save ~17%)"}
                        </p>
                    </div>
                    <p class="text-sm text-neutral-600 \
                              dark:text-neutral-400">
                        {"For communities that need more storage."}
                    </p>
                    <ul class="space-y-2 text-sm \
                               text-neutral-700 \
                               dark:text-neutral-300">
                        <PricingFeature
                            text="2 GB storage" />
                        <PricingFeature
                            text="Unlimited members" />
                        <PricingFeature
                            text="Unlimited auctions" />
                        <PricingFeature
                            text="All features included" />
                    </ul>
                    <button
                        onclick={on_sign_up}
                        class="w-full px-4 py-2.5 text-sm \
                               font-medium text-white \
                               bg-neutral-900 \
                               dark:bg-neutral-100 \
                               dark:text-neutral-900 \
                               rounded-md \
                               hover:bg-neutral-700 \
                               dark:hover:bg-neutral-300 \
                               transition-colors"
                    >
                        {"Get started"}
                    </button>
                </div>
            </div>

            // What counts as storage
            <div class="max-w-2xl mx-auto space-y-4">
                <h2 class="text-xl font-semibold \
                           text-neutral-900 \
                           dark:text-neutral-100">
                    {"What counts as storage?"}
                </h2>
                <p class="text-neutral-600 \
                          dark:text-neutral-400">
                    {"Storage includes images you upload \
                      (site photos, floor plans) and the \
                      data your community generates \
                      (members, sites, spaces, auctions, \
                      and transaction history). Most \
                      communities stay well within the \
                      free tier."}
                </p>
                <p class="text-neutral-600 \
                          dark:text-neutral-400">
                    {"You can track your storage usage \
                      from the Billing tab in your \
                      community settings. Upgrade or \
                      downgrade at any time."}
                </p>
            </div>

            // CTA
            <div class="text-center space-y-4">
                <p class="text-neutral-600 \
                          dark:text-neutral-400">
                    {"Want to learn more about how \
                      TinyLVT works?"}
                </p>
                <button
                    onclick={on_learn_more}
                    class="px-5 py-2.5 text-sm font-medium \
                           text-neutral-700 \
                           dark:text-neutral-300 \
                           bg-white dark:bg-neutral-800 \
                           border border-neutral-300 \
                           dark:border-neutral-600 \
                           rounded-md hover:bg-neutral-50 \
                           dark:hover:bg-neutral-700 \
                           transition-colors"
                >
                    {"Read the docs"}
                </button>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct PricingFeatureProps {
    text: &'static str,
}

#[function_component]
fn PricingFeature(props: &PricingFeatureProps) -> Html {
    html! {
        <li class="flex items-start gap-2">
            <span class="text-neutral-400 \
                         dark:text-neutral-500 mt-0.5">
                {"\u{2713}"}
            </span>
            {props.text}
        </li>
    }
}
