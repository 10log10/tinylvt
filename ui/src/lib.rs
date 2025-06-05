mod theme;

use payloads::APIClient;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

use theme::ThemeToggle;

#[derive(Default, Clone, PartialEq, Store)]
struct State {
    pub error_message: Option<String>,
}

// Global API client - configurable via environment or same-origin fallback
pub fn get_api_client() -> APIClient {
    // Try environment variable first (set at build time)
    let address = option_env!("BACKEND_URL")
        .map(|url| url.to_string())
        .unwrap_or_else(|| {
            // Fallback to same origin (current setup)
            let window = web_sys::window().unwrap();
            let location = window.location();
            location.origin().unwrap()
        });

    APIClient {
        address,
        inner_client: reqwest::Client::new(),
    }
}

#[function_component]
pub fn App() -> Html {
    html! {
        <BrowserRouter>
            <div class="min-h-screen bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 transition-colors">
                <Header />
                <ErrorMessage />
                <Switch<Route> render={switch} />
            </div>
        </BrowserRouter>
    }
}

#[function_component]
fn Header() -> Html {
    html! {
        <header class="bg-gray-100 dark:bg-gray-800 shadow-sm border-b border-gray-200 dark:border-gray-700">
            <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
                <div class="flex justify-between items-center h-16">
                    <div class="flex items-center">
                        <h1 class="text-xl font-bold text-gray-900 dark:text-white">{"TinyLVT"}</h1>
                    </div>
                    <div class="flex items-center space-x-4">
                        <ThemeToggle />
                        // Add navigation items here later
                    </div>
                </div>
            </div>
        </header>
    }
}

#[function_component]
fn ErrorMessage() -> Html {
    let (err, dispatch) = use_store::<State>();
    let error_message = err.error_message.clone();
    
    if error_message.is_some() {
        let dispatch = dispatch.clone();
        yew::platform::spawn_local(async move {
            yew::platform::time::sleep(std::time::Duration::from_secs(5)).await;
            dispatch.reduce_mut(|s| s.error_message = None);
        })
    }
    html! {
        <>
            if let Some(msg) = &error_message {
                <div class="bg-red-100 dark:bg-red-900 border border-red-400 dark:border-red-600 text-red-700 dark:text-red-200 px-4 py-3 rounded mx-4 mt-4">
                    {msg}
                </div>
            }
        </>
    }
}

#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[at("/")]
    Home,
    // #[at("/about")]
    // About,
    // #[at("/login")]
    // Login,
    // #[at("/profile")]
    // Profile,
    // #[at("/bids")]
    // Bids,
    #[not_found]
    #[at("/404")]
    NotFound,
}

#[function_component]
fn HealthCheck() -> Html {
    let health_status = use_state(|| "Checking...".to_string());
    
    {
        let health_status = health_status.clone();
        use_effect_with((), move |_| {
            let health_status = health_status.clone();
            yew::platform::spawn_local(async move {
                let client = get_api_client();
                match client.health_check().await {
                    Ok(_) => health_status.set("✅ Connected to backend".to_string()),
                    Err(e) => health_status.set(format!("❌ Backend error: {}", e)),
                }
            });
            || ()
        });
    }
    
    html! {
        <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 p-4 rounded-lg">
            <h3 class="font-bold text-blue-900 dark:text-blue-100">{"Backend Health"}</h3>
            <p class="text-blue-700 dark:text-blue-300">{(*health_status).clone()}</p>
        </div>
    }
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { 
            <main class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
                <div class="space-y-6">
                    <div>
                        <h1 class="text-3xl font-bold text-gray-900 dark:text-white">{"Welcome to TinyLVT"}</h1>
                        <p class="mt-2 text-gray-600 dark:text-gray-300">{"A community-based land value tax auction system"}</p>
                    </div>
                    <HealthCheck />
                    <div class="bg-gray-50 dark:bg-gray-800 p-6 rounded-lg">
                        <h2 class="text-xl font-semibold mb-4 text-gray-900 dark:text-white">{"Theme Demo"}</h2>
                        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                            <div class="bg-white dark:bg-gray-700 p-4 rounded border border-gray-200 dark:border-gray-600">
                                <h3 class="font-medium text-gray-900 dark:text-white">{"Card Example"}</h3>
                                <p class="text-gray-600 dark:text-gray-300">{"This card adapts to your theme preference."}</p>
                            </div>
                            <div class="bg-blue-500 text-white p-4 rounded">
                                <h3 class="font-medium">{"Primary Button"}</h3>
                                <p class="text-blue-100">{"Consistent across themes"}</p>
                            </div>
                            <div class="bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200 p-4 rounded border border-green-200 dark:border-green-700">
                                <h3 class="font-medium">{"Success Message"}</h3>
                                <p>{"Semantic colors work great!"}</p>
                            </div>
                        </div>
                    </div>
                </div>
            </main>
        },
        // Route::About => html! { <about::About /> },
        // Route::Login => html! { <login::Login /> },
        // Route::Profile => html! { <profile::Profile /> },
        // Route::Bids => html! { <bids::Bids /> },
        Route::NotFound => html! { 
            <main class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
                <div class="text-center">
                    <h1 class="text-4xl font-bold text-gray-900 dark:text-white">{"404"}</h1>
                    <p class="text-gray-600 dark:text-gray-300">{"Page not found"}</p>
                </div>
            </main>
        },
    }
}
