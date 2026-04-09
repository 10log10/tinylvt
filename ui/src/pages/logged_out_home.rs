use std::collections::HashMap;

use crate::Route;
use crate::State;
use crate::components::{AuctionChartDemo, AuctionInterfaceWalkthrough};
use crate::hooks::{use_platform_stats, use_push_route, use_title};
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

#[function_component]
pub fn LoggedOutHomePage() -> Html {
    use_title("TinyLVT");
    let push_route = use_push_route();
    let navigator = use_navigator().unwrap();
    let (state, _dispatch) = use_store::<State>();
    let stats = use_platform_stats();

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
            // Two-column layout: intro text + demo
            // Desktop: side by side. Mobile: stacked.
            <div class="flex flex-col lg:flex-row gap-10 \
                lg:items-start max-w-6xl mx-auto">
                // Left column: intro text
                <div class="lg:w-5/12 space-y-8 pt-12">
                    // Tagline
                    <p class="text-4xl sm:text-3xl font-semibold \
                        text-neutral-900 dark:text-neutral-100">
                        {"Fair allocation for anything\u{00A0}shared."}
                    </p>

                    <p class="text-lg text-neutral-600 \
                        dark:text-neutral-400">
                        {"When housemates share a rental, who \
                        gets the master bedroom? When a team \
                        shares an office, who gets the window \
                        desk? Traditional methods leave someone \
                        feeling shortchanged."}
                    </p>

                    <p class="text-lg text-neutral-600 \
                        dark:text-neutral-400">
                        {"TinyLVT uses auctions to allocate \
                        spaces fairly. Everyone bids what each \
                        space is worth to them. Spaces go to \
                        those who value them most, and the \
                        proceeds are shared equally."}
                    </p>

                    // Key insight box
                    <div class="bg-neutral-100 dark:bg-neutral-800 \
                        rounded-lg p-6 border border-neutral-200 \
                        dark:border-neutral-700">
                        <p class="text-lg font-medium text-neutral-900 \
                            dark:text-neutral-100">
                            {"You only pay what others would have paid."}
                        </p>
                        <p class="text-neutral-600 dark:text-neutral-400 \
                            mt-2">
                            {"If you win a space, you pay just enough \
                            to outbid the next-highest bidder\u{2014}not \
                            your maximum. This encourages honest bidding \
                            and ensures fair prices."}
                        </p>
                    </div>

                    // CTAs
                    <div class="flex flex-col sm:flex-row gap-4 justify-center">
                        <button
                            onclick={on_sign_up.clone()}
                            class="inline-block px-8 py-3 text-lg \
                                font-semibold text-white bg-neutral-900 \
                                hover:bg-neutral-700 dark:bg-neutral-100 \
                                dark:text-neutral-900 \
                                dark:hover:bg-neutral-300 rounded \
                                transition-colors"
                        >
                            {"Sign Up"}
                        </button>
                        <button
                            onclick={on_learn_more.clone()}
                            class="inline-block px-8 py-3 text-lg \
                                font-semibold text-neutral-900 \
                                dark:text-neutral-100 border-2 \
                                border-neutral-900 \
                                dark:border-neutral-100 \
                                hover:bg-neutral-100 \
                                dark:hover:bg-neutral-800 rounded \
                                transition-colors"
                        >
                            {"Learn How It Works"}
                        </button>
                    </div>
                </div>

                // Right column: auction demo
                <div class="lg:w-7/12">
                    <AuctionChartDemo />
                </div>
            </div>

            // LVT background
            <div class="max-w-3xl mx-auto space-y-4">
                <h2 class="text-2xl font-semibold \
                    text-neutral-900 dark:text-neutral-100">
                    {"Why \"LVT\"?"}
                </h2>
                <p class="text-lg text-neutral-600 \
                    dark:text-neutral-400">
                    {"TinyLVT is based on the principles of \
                    land value taxation (LVT). Land value \
                    taxes:"}
                </p>
                <ul class="space-y-2 text-lg text-neutral-600 \
                    dark:text-neutral-400 list-disc pl-6">
                    <li>
                        {"allocate scarce resources "}
                        <span class="italic">{"(\"land\")"}</span>
                    </li>
                    <li>
                        {"by assessing their rental value "}
                        <span class="italic">{"(\"value\")"}</span>
                    </li>
                    <li>
                        {"and capturing and redistributing \
                        that value to the community "}
                        <span class="italic">{"(\"tax\")"}</span>
                    </li>
                </ul>
                <p class="text-lg text-neutral-600 \
                    dark:text-neutral-400">
                    {"Land value taxes ensure resources are \
                    used well and guarantee equal access, \
                    even if the resource possession itself \
                    is unequal. The redistribution \
                    compensates those who are excluded from \
                    the resource for their share of its \
                    value."}
                </p>
                <p class="text-lg text-neutral-600 \
                    dark:text-neutral-400">
                    {"TinyLVT is a pure implementation of \
                    land value taxation. Resource value and allocation are \
                    precisely determined with auctions. \
                    Distributions are direct payments to \
                    each community member."}
                </p>
            </div>

            // Interface walkthrough section
            <div class="max-w-5xl mx-auto space-y-8">
                <div class="text-center space-y-4">
                    <h2 class="text-2xl font-semibold text-neutral-900 \
                        dark:text-neutral-100">
                        {"The auction interface"}
                    </h2>
                </div>
                <AuctionInterfaceWalkthrough
                    dark_mode={state.is_dark_mode()}
                />
            </div>

            // Examples
            <div class="max-w-5xl mx-auto">
                <h2 class="text-2xl font-semibold text-neutral-900 \
                    dark:text-neutral-100 mb-6 text-center">
                    {"Examples"}
                </h2>
                <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
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
                            {"20 grad students, 14 desks. Each term:"}
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

                    // Market stall example
                    <div class="bg-neutral-50 dark:bg-neutral-800/50 rounded-lg \
                        p-6 border border-neutral-200 dark:border-neutral-700">
                        <h3 class="text-lg font-semibold text-neutral-900 \
                            dark:text-neutral-100 mb-3">
                            {"Assigning market stalls"}
                        </h3>
                        <p class="text-neutral-600 dark:text-neutral-400 text-sm \
                            mb-4">
                            {"A street fair with 30 vendor spots. Corner stalls \
                            have extra frontage; spots near the entrance get more \
                            foot traffic."}
                        </p>
                        <ul class="space-y-2 text-sm text-neutral-600 \
                            dark:text-neutral-400">
                            <li class="flex items-start gap-2">
                                <span class="text-neutral-400">{"•"}</span>
                                <span>{"Vendors bid on preferred spots"}</span>
                            </li>
                            <li class="flex items-start gap-2">
                                <span class="text-neutral-400">{"•"}</span>
                                <span>{"Prime locations cost more"}</span>
                            </li>
                            <li class="flex items-start gap-2">
                                <span class="text-neutral-400">{"•"}</span>
                                <span>{"Revenue offsets event costs"}</span>
                            </li>
                        </ul>
                        <p class="text-neutral-500 dark:text-neutral-500 text-xs \
                            mt-4">
                            {"Market-based pricing without awkward negotiations."}
                        </p>
                    </div>
                </div>
            </div>

            // Final CTAs
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

            // Platform stats
            <div class="max-w-2xl mx-auto">
                <div class="flex flex-col sm:flex-row gap-6 \
                    justify-center text-center">
                    {if let Some(s) = stats.data.as_ref() {
                        html! {
                            <>
                            <div class="flex-1">
                                <p class="text-3xl font-bold \
                                    text-neutral-900 \
                                    dark:text-neutral-100">
                                    {s.auctions_held.to_string()}
                                </p>
                                <p class="text-sm text-neutral-500 \
                                    dark:text-neutral-400 mt-1">
                                    {"Auctions held"}
                                </p>
                            </div>
                            <div class="flex-1">
                                <p class="text-3xl font-bold \
                                    text-neutral-900 \
                                    dark:text-neutral-100">
                                    {s.spaces_allocated.to_string()}
                                </p>
                                <p class="text-sm text-neutral-500 \
                                    dark:text-neutral-400 mt-1">
                                    {"Spaces allocated"}
                                </p>
                            </div>
                            </>
                        }
                    } else {
                        // Placeholder to reserve space and avoid
                        // layout shift while loading
                        html! {
                            <>
                            <div class="flex-1 invisible">
                                <p class="text-3xl font-bold">
                                    {"\u{00a0}"}
                                </p>
                                <p class="text-sm mt-1">{"\u{00a0}"}</p>
                            </div>
                            <div class="flex-1 invisible">
                                <p class="text-3xl font-bold">
                                    {"\u{00a0}"}
                                </p>
                                <p class="text-sm mt-1">{"\u{00a0}"}</p>
                            </div>
                            </>
                        }
                    }}
                </div>
            </div>
        </div>
    }
}
