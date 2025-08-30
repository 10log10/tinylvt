use payloads::APIClient;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

mod logs;

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
    logs::init_logging();
    html! {
        <BrowserRouter>
            <div class="min-h-screen bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 transition-colors">
                <Switch<Route> render={switch} />
            </div>
        </BrowserRouter>
    }
}

#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[at("/")]
    Home,
    #[not_found]
    #[at("/404")]
    NotFound,
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! {
            <main class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
                <p>{"Hello World"}</p>
            </main>
        },
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
