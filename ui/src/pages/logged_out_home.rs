use std::collections::HashMap;

use crate::Route;
use crate::State;
use crate::hooks::use_push_route;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

#[function_component]
pub fn LoggedOutHomePage() -> Html {
    let push_route = use_push_route();
    let navigator = use_navigator().unwrap();
    let (state, _dispatch) = use_store::<State>();

    let on_get_started = {
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
        <div class="text-center space-y-8">
            <div>
                <h1 class="text-6xl font-bold text-neutral-900 dark:text-neutral-100 mb-4">
                    {"TinyLVT"}
                </h1>
                <p class="text-xl text-neutral-900 dark:text-neutral-100 mb-8">
                    {"A structured way to share things"}
                </p>
            </div>

            <div class="max-w-2xl mx-auto">
                <p class="text-lg text-neutral-600 dark:text-neutral-400">
                    {"People regularly encounter situations where they need to share something. That thing cannot always be physically split into equal pieces."}
                </p>
            </div>

            <div class="max-w-2xl mx-auto">
                <p class="text-lg text-neutral-600 dark:text-neutral-400">
                    {"A perfect substitute is to determine the thing’s value, and share that equally, such that one person gets the thing and pays everyone else for their share of the thing’s value."}
                </p>
            </div>

            <div class="max-w-2xl mx-auto">
                <p class="text-lg text-neutral-600 dark:text-neutral-400">
                    {"Simply auction the thing to the highest bidder, then redistribute the proceeds. Repeat on a schedule."}
                </p>
            </div>

            <div class="max-w-2xl mx-auto">
                <h2 class="text-2xl font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                    {"What you can share"}
                </h2>
                <p class="text-lg text-neutral-600 dark:text-neutral-400">
                    {"Space is the most common thing in need of sharing: rooms in a home, desks in a workspace, stalls in a market, land."}
                </p>
            </div>

            <div class="max-w-2xl mx-auto">
                <h2 class="text-2xl font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                    {"How it works"}
                </h2>
                <ol class="text-lg text-neutral-600 dark:text-neutral-400 text-left space-y-2">
                    <li>{"1. Invite people to your community"}</li>
                    <li>{"2. Setup your site—home, workplace, market, city"}</li>
                    <li>{"3. Auction the spaces in the site"}</li>
                    <li>{"4. Redistribute the proceeds"}</li>
                    <li>{"5. Repeat every usage period"}</li>
                </ol>
            </div>

            <div class="max-w-2xl mx-auto">
                <h2 class="text-2xl font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                    {"Why it works"}
                </h2>
                <ul class="text-lg text-neutral-600 dark:text-neutral-400 text-left space-y-2 list-disc list-inside">
                    <li>{"Spaces go to the people that want them most"}</li>
                    <li>{"They only pay what others would have paid"}</li>
                    <li>{"Everyone gets an equal share of the value"}</li>
                </ul>
            </div>

            <div class="py-8 flex flex-col sm:flex-row gap-4 justify-center">
                <button
                    onclick={on_get_started}
                    class="inline-block px-8 py-3 text-lg font-semibold text-white bg-neutral-900 hover:bg-neutral-700 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-300 rounded transition-colors"
                >
                    {"Get Started"}
                </button>
                <button
                    onclick={on_sign_up}
                    class="inline-block px-8 py-3 text-lg font-semibold text-neutral-900 dark:text-neutral-100 border-2 border-neutral-900 dark:border-neutral-100 hover:bg-neutral-100 dark:hover:bg-neutral-800 rounded transition-colors"
                >
                    {"Sign Up"}
                </button>
            </div>

            <div class="max-w-7xl mx-auto my-12 px-4">
                <div class="grid grid-cols-1 md:grid-cols-2 gap-8">
                    if state.is_dark_mode() {
                        <img src="/auction-list-dark.jpg" alt="TinyLVT screenshot showing auction list" class="w-full rounded-lg shadow-lg border border-neutral-700" />
                        <img src="/auction-page-dark.jpg" alt="TinyLVT screenshot showing auction page" class="w-full rounded-lg shadow-lg border border-neutral-700" />
                    } else {
                        <img src="/auction-list-light.jpg" alt="TinyLVT screenshot showing auction list" class="w-full rounded-lg shadow-lg border border-neutral-300" />
                        <img src="/auction-page-light.jpg" alt="TinyLVT screenshot showing auction page" class="w-full rounded-lg shadow-lg border border-neutral-300" />
                    }
                </div>
            </div>
        </div>
    }
}
