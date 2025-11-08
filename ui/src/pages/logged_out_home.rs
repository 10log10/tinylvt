use crate::Route;
use yew::prelude::*;
use yew_router::prelude::*;

#[function_component]
pub fn LoggedOutHomePage() -> Html {
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
                    {"Communities struggle to equitably share scarce resources, since they're difficult to divide equally among members. A minority often captures most of the resource value."}
                </p>
            </div>

            <div class="max-w-2xl mx-auto">
                <p class="text-lg text-neutral-600 dark:text-neutral-400">
                    {"TinyLVT solves this problem with auctions. Auction the resource to the highest bidder, then redistribute the proceeds equally. Repeat on a schedule."}
                </p>
            </div>

            <div class="max-w-2xl mx-auto">
                <h2 class="text-2xl font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                    {"What you can share"}
                </h2>
                <p class="text-lg text-neutral-600 dark:text-neutral-400">
                    {"Space is the most common scarce resource in need of sharing. Rooms in a home, desks in a workspace, stalls in a market, urban land. Even wireless spectrum, URLs, and patent rights are types of space—radio space, namespace, idea space. TinyLVT is designed for sharing space, but can also be used to auction many other things."}
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
                    <li>{"5. Repeat on a schedule"}</li>
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

            <div class="mt-8">
                <Link<Route> to={Route::Login} classes="inline-block px-8 py-3 text-lg font-semibold text-white bg-neutral-900 hover:bg-neutral-700 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-300 rounded transition-colors">
                    {"Get Started"}
                </Link<Route>>
            </div>

            <div class="max-w-2xl mx-auto mt-16">
                <h2 class="text-2xl font-semibold text-neutral-900 dark:text-neutral-100 mb-6">
                    {"FAQ"}
                </h2>
                <div class="space-y-6">
                    <div>
                        <h3 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100 mb-2">
                            {"Won't this favor wealthy members?"}
                        </h3>
                        <p class="text-lg text-neutral-600 dark:text-neutral-400 text-left">
                            {"Auction winners pay the community for using the resource. Wealthy members can only continue winning auctions if they spend more and more money, or if the rest of the community would rather keep the proceeds than bid for the resource. This is to the benefit of the community, which captures the resource value."}
                        </p>
                    </div>
                </div>
            </div>
        </div>
    }
}
