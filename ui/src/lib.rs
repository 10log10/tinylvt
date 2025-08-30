use payloads::APIClient;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

pub mod logs;
pub mod components;
pub mod pages;

use components::layout::MainLayout;
use pages::{HomePage, NotFoundPage};

#[derive(Default, Clone, PartialEq, Store)]
struct State {
    pub error_message: Option<String>,
    pub dark_mode: bool,
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
            <MainLayout>
                <Switch<Route> render={switch} />
            </MainLayout>
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
        Route::Home => html! { <HomePage /> },
        Route::NotFound => html! { <NotFoundPage /> },
    }
}
