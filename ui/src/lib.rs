use payloads::APIClient;
use yew::prelude::*;
use yew_router::prelude::*;

mod components;
pub mod logs;
mod pages;
mod state;
mod test_components;
mod utils;

use components::layout::MainLayout;
use pages::{HomePage, NotFoundPage, TestPage};
pub(crate) use state::{State, ThemeMode};

// Global API client - configurable via environment or same-origin fallback
fn get_api_client() -> APIClient {
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
pub enum Route {
    #[at("/")]
    Home,
    #[at("/test")]
    Test,
    #[not_found]
    #[at("/404")]
    NotFound,
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <HomePage /> },
        Route::Test => html! { <TestPage /> },
        Route::NotFound => html! { <NotFoundPage /> },
    }
}
