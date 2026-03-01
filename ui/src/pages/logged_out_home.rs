use std::collections::HashMap;

use crate::Route;
use crate::State;
use crate::hooks::{use_push_route, use_title};
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

#[function_component]
pub fn LoggedOutHomePage() -> Html {
    use_title("TinyLVT");
    let push_route = use_push_route();
    let navigator = use_navigator().unwrap();
    let (state, _dispatch) = use_store::<State>();

    let on_learn_more = {
        let push_route = push_route.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            push_route.emit(Route::Docs);
        })
    };

    let on_sign_up = {
        let navigator = navigator.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let mut query = HashMap::new();
            query.insert("signup".to_string(), "true".to_string());
            let _ = navigator.push_with_query(&Route::Login, &query);
        })
    };

    html! {
            <div class="space-y-16">
                // Hero section with concrete use cases
                <div class="text-center space-y-6">
                    <h1 class="text-5xl sm:text-6xl font-bold text-neutral-900 \
                        dark:text-neutral-100">
                        {"TinyLVT"}
                    </h1>
                    <p class="text-xl text-neutral-600 dark:text-neutral-400 \
                        max-w-2xl mx-auto">
                        {"Fair allocation for shared spaces, from splitting rent among \
                        housemates to assigning desks in a workspace."}
                    </p>
                </div>

                // Problem statement
                <div class="max-w-2xl mx-auto space-y-4">
                    <h2 class="text-2xl font-semibold text-neutral-900 \
                        dark:text-neutral-100">
                        {"The problem"}
                    </h2>
                    <p class="text-lg text-neutral-600 dark:text-neutral-400">
                        {"When housemates share a rental, who gets the master bedroom? \
                        When a team shares an office, who gets the window desk?"}
                    </p>
                    <p class="text-lg text-neutral-600 dark:text-neutral-400">
                        {"Traditional methods—first-come-first-served, rotation, or \
                        awkward negotiations—leave someone feeling shortchanged."}
                    </p>
                </div>

                // Solution with key insight
                <div class="max-w-2xl mx-auto space-y-4">
                    <h2 class="text-2xl font-semibold text-neutral-900 \
                        dark:text-neutral-100">
                        {"The solution"}
                    </h2>
                    <p class="text-lg text-neutral-600 dark:text-neutral-400">
                        {"TinyLVT uses auctions to allocate spaces fairly. Everyone \
                        bids what each space is worth to them. Spaces go to those who \
                        value them most, and the proceeds are shared equally."}
                    </p>
                    <p class="text-lg text-neutral-600 dark:text-neutral-400">
                        {"TinyLVT uses "}
                        <span class="font-medium text-neutral-900 \
                            dark:text-neutral-100">
                            {"Simultaneous Ascending Auctions"}
                        </span>
                        {", the same mechanism used for high-stakes allocations like \
                        wireless spectrum licenses. It's the gold standard in \
                        mechanism design."}
                    </p>
                    <div class="bg-neutral-100 dark:bg-neutral-800 rounded-lg p-6 \
                        border border-neutral-200 dark:border-neutral-700">
                        <p class="text-lg font-medium text-neutral-900 \
                            dark:text-neutral-100">
                            {"You only pay what others would have \
                            paid."}
                        </p>
                        <p class="text-neutral-600 dark:text-neutral-400 mt-2">
                            {"If you win a space, you pay just enough to outbid the \
                            next-highest bidder—not your maximum. This encourages \
                            honest bidding and ensures fair prices."}
                        </p>
                    </div>
                </div>

                // Two concrete examples
                <div class="max-w-3xl mx-auto">
                    <h2 class="text-2xl font-semibold text-neutral-900 \
                        dark:text-neutral-100 mb-6 text-center">
    {"Examples"}
                    </h2>
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                        // Rent splitting example
                        <div class="bg-neutral-50 dark:bg-neutral-800/50 rounded-lg \
                            p-6 border border-neutral-200 dark:border-neutral-700">
                            <h3 class="text-lg font-semibold text-neutral-900 \
                                dark:text-neutral-100 mb-3">
                                {"Splitting rent"}
                            </h3>
                            <p class="text-neutral-600 dark:text-neutral-400 text-sm \
                                mb-4">
                                {"Three housemates, three rooms, $3,000/month total \
                                rent. The auction determines assignments and rent adjustments:"}
                            </p>
                            <div class="space-y-2 text-sm">
                                <div class="flex justify-between">
                                    <span class="text-neutral-600 \
                                        dark:text-neutral-400">
                                        {"Alice — Master bedroom"}
                                    </span>
                                    <span class="font-medium text-neutral-900 \
                                        dark:text-neutral-100">
                                        {"$1,150"}
                                    </span>
                                </div>
                                <div class="flex justify-between">
                                    <span class="text-neutral-600 \
                                        dark:text-neutral-400">
                                        {"Bob — Middle room"}
                                    </span>
                                    <span class="font-medium text-neutral-900 \
                                        dark:text-neutral-100">
                                        {"$950"}
                                    </span>
                                </div>
                                <div class="flex justify-between">
                                    <span class="text-neutral-600 \
                                        dark:text-neutral-400">
                                        {"Carol — Small room"}
                                    </span>
                                    <span class="font-medium text-neutral-900 \
                                        dark:text-neutral-100">
                                        {"$900"}
                                    </span>
                                </div>
                            </div>
                            <p class="text-neutral-500 dark:text-neutral-500 text-xs \
                                mt-4">
                                {"Everyone pays according to room value. Total: still \
                                $3,000."}
                            </p>
                        </div>

                        // Desk allocation example
                        <div class="bg-neutral-50 dark:bg-neutral-800/50 rounded-lg \
                            p-6 border border-neutral-200 dark:border-neutral-700">
                            <h3 class="text-lg font-semibold text-neutral-900 \
                                dark:text-neutral-100 mb-3">
                                {"Allocating desks"}
                            </h3>
                            <p class="text-neutral-600 dark:text-neutral-400 text-sm \
                                mb-4">
                                {"Each term:"}
                            </p>
                            <ul class="space-y-2 text-sm text-neutral-600 \
                                dark:text-neutral-400">
                                <li class="flex items-start gap-2">
                                    <span class="text-neutral-400">{"1."}</span>
                                    <span>{"Everyone receives 100 points"}</span>
                                </li>
                                <li class="flex items-start gap-2">
                                    <span class="text-neutral-400">{"2."}</span>
                                    <span>{"Bid on desks you want"}</span>
                                </li>
                                <li class="flex items-start gap-2">
                                    <span class="text-neutral-400">{"3."}</span>
                                    <span>{"Winners pay points; others save theirs"}
                                    </span>
                                </li>
                            </ul>
                            <p class="text-neutral-500 dark:text-neutral-500 text-xs \
                                mt-4">
                                {"Fair allocation without real money changing hands."}
                            </p>
                        </div>
                    </div>
                </div>

                // CTAs
                <div class="py-4 flex flex-col sm:flex-row gap-4 justify-center">
                    <button
                        onclick={on_sign_up}
                        class="inline-block px-8 py-3 text-lg font-semibold \
                            text-white bg-neutral-900 hover:bg-neutral-700 \
                            dark:bg-neutral-100 dark:text-neutral-900 \
                            dark:hover:bg-neutral-300 rounded transition-colors"
                    >
                        {"Sign Up"}
                    </button>
                    <button
                        onclick={on_learn_more}
                        class="inline-block px-8 py-3 text-lg font-semibold \
                            text-neutral-900 dark:text-neutral-100 border-2 \
                            border-neutral-900 dark:border-neutral-100 \
                            hover:bg-neutral-100 dark:hover:bg-neutral-800 \
                            rounded transition-colors"
                    >
                        {"Learn How It Works"}
                    </button>
                </div>

                // Screenshots
                <div class="max-w-7xl mx-auto px-4">
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-8">
                        if state.is_dark_mode() {
                            <img
                                src="/auction-list-dark.jpg"
                                alt="TinyLVT screenshot showing auction list"
                                class="w-full rounded-lg shadow-lg border \
                                    border-neutral-700"
                            />
                            <img
                                src="/auction-page-dark.jpg"
                                alt="TinyLVT screenshot showing auction page"
                                class="w-full rounded-lg shadow-lg border \
                                    border-neutral-700"
                            />
                        } else {
                            <img
                                src="/auction-list-light.jpg"
                                alt="TinyLVT screenshot showing auction list"
                                class="w-full rounded-lg shadow-lg border \
                                    border-neutral-300"
                            />
                            <img
                                src="/auction-page-light.jpg"
                                alt="TinyLVT screenshot showing auction page"
                                class="w-full rounded-lg shadow-lg border \
                                    border-neutral-300"
                            />
                        }
                    </div>
                </div>
            </div>
        }
}
