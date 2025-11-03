use yew::prelude::*;

#[function_component]
pub fn LoggedOutHomePage() -> Html {
    html! {
        <div class="text-center space-y-8">
            <div>
                <h1 class="text-4xl font-bold text-neutral-900 dark:text-neutral-100 mb-4">
                    {"Welcome to TinyLVT"}
                </h1>
                <p class="text-xl text-neutral-600 dark:text-neutral-400 mb-8">
                    {"Land value taxation for small-scale shared spaces"}
                </p>
            </div>

            <div class="max-w-2xl mx-auto">
                <p class="text-lg text-neutral-600 dark:text-neutral-400">
                    {"TinyLVT implements land value taxation through auction-based allocation.
                     Allocate spaces like coworking seats to highest-value uses while ensuring
                     users only pay the social cost of excluding others."}
                </p>
            </div>

            <div class="grid grid-cols-1 md:grid-cols-3 gap-8 mt-12">
                <div class="text-center">
                    <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-2">
                        {"Auction-Based Allocation"}
                    </h3>
                    <p class="text-neutral-600 dark:text-neutral-400">
                        {"Fair and efficient space allocation through simultaneous ascending auctions"}
                    </p>
                </div>

                <div class="text-center">
                    <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-2">
                        {"Community Ownership"}
                    </h3>
                    <p class="text-neutral-600 dark:text-neutral-400">
                        {"Rent redistribution and activity tracking for equitable community management"}
                    </p>
                </div>

                <div class="text-center">
                    <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-2">
                        {"Proxy Bidding"}
                    </h3>
                    <p class="text-neutral-600 dark:text-neutral-400">
                        {"Set your values once and let the system bid automatically for you"}
                    </p>
                </div>
            </div>
        </div>
    }
}
