use payloads::APIClient;
use yew::prelude::*;
use yew_router::prelude::*;

mod components;
mod hooks;
pub mod logs;
mod pages;
mod state;
mod test_components;
mod utils;

use components::layout::MainLayout;
use hooks::use_authentication;
use pages::{
    CommunitiesPage, CommunityDetailPage, CommunityMembersPage,
    CreateCommunityPage, HomePage, LoginPage, NotFoundPage, TestPage,
};
pub(crate) use state::{AuthState, State, ThemeMode};

#[function_component]
pub fn App() -> Html {
    // Check authentication status on startup
    use_authentication();

    html! {
        <BrowserRouter>
            <MainLayout>
                <Switch<Route> render={switch} />
            </MainLayout>
        </BrowserRouter>
    }
}

// Global API client - configurable via environment or same-origin fallback
pub(crate) fn get_api_client() -> APIClient {
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

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/login")]
    Login,
    #[at("/communities")]
    Communities,
    #[at("/communities/new")]
    CreateCommunity,
    #[at("/communities/:id")]
    CommunityDetail { id: String },
    #[at("/communities/:id/members")]
    CommunityMembers { id: String },
    #[at("/test")]
    Test,
    #[not_found]
    #[at("/404")]
    NotFound,
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <HomePage /> },
        Route::Login => html! { <LoginPage /> },
        Route::Communities => html! { <CommunitiesPage /> },
        Route::CreateCommunity => html! { <CreateCommunityPage /> },
        Route::CommunityDetail { id } => {
            html! { <CommunityDetailPage community_id={id} /> }
        }
        Route::CommunityMembers { id } => {
            html! { <CommunityMembersPage community_id={id} /> }
        }
        Route::Test => html! { <TestPage /> },
        Route::NotFound => html! { <NotFoundPage /> },
    }
}
