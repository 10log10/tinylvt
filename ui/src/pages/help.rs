use yew::prelude::*;

const SUPPORT_EMAIL: &str = env!("SUPPORT_EMAIL");

#[function_component]
pub fn HelpPage() -> Html {
    let support_email_href = format!("mailto:{}", SUPPORT_EMAIL);

    html! {
        <div class="max-w-4xl mx-auto px-4 py-8">
            <h1 class="text-3xl font-bold text-neutral-900 dark:text-white mb-8">
                {"Help & Support"}
            </h1>

            <div class="bg-white dark:bg-neutral-800 border border-neutral-200 dark:border-neutral-700 rounded-lg p-6 mb-6">
                <h2 class="text-xl font-semibold text-neutral-900 dark:text-white mb-4">
                    {"Need help with your account?"}
                </h2>
                <p class="text-neutral-700 dark:text-neutral-300 mb-4">
                    {"If you're experiencing issues with your account, email verification, or have any questions about using TinyLVT, please contact us:"}
                </p>
                <a
                    href={support_email_href}
                    class="text-neutral-900 dark:text-white font-semibold hover:underline"
                >
                    {SUPPORT_EMAIL}
                </a>
            </div>

            <div class="bg-white dark:bg-neutral-800 border border-neutral-200 dark:border-neutral-700 rounded-lg p-6">
                <h2 class="text-xl font-semibold text-neutral-900 dark:text-white mb-4">
                    {"Common Issues"}
                </h2>
                <div class="space-y-4 text-neutral-700 dark:text-neutral-300">
                    <div>
                        <h3 class="font-semibold text-neutral-900 dark:text-white mb-2">
                            {"Didn't receive verification email?"}
                        </h3>
                        <p>{"Check your spam folder. If you still can't find it, contact support."}</p>
                    </div>
                    <div>
                        <h3 class="font-semibold text-neutral-900 dark:text-white mb-2">
                            {"Lost your password?"}
                        </h3>
                        <p>{"Use the password reset option on the login page."}</p>
                    </div>
                    <div>
                        <h3 class="font-semibold text-neutral-900 dark:text-white mb-2">
                            {"Questions about auctions or bidding?"}
                        </h3>
                        <p>{"Contact your community leader or reach out to support for assistance."}</p>
                    </div>
                </div>
            </div>
        </div>
    }
}
