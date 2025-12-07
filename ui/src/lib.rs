use payloads::{APIClient, AuctionId, CommunityId, InviteId, SiteId};
use yew::prelude::*;
use yew_router::prelude::*;

mod components;
mod contexts;
mod hooks;
pub mod logs;
mod pages;
mod state;
mod utils;

use components::ToastContainer;
use components::layout::MainLayout;
use contexts::ToastProvider;
use hooks::use_authentication;
use pages::{
    AcceptInvitePage, AuctionDetailPage, AuctionRoundsPage, CommunitiesPage,
    CommunityDetailPage, CommunityInvitesPage, CommunityMembersPage,
    CommunitySettingsPage, CreateAuctionPage, CreateCommunityPage,
    CreateSitePage, ForgotPasswordPage, HelpPage, LoggedInHomePage,
    LoggedOutHomePage, LoginPage, NotFoundPage, ProfilePage, ResetPasswordPage,
    SiteAuctionsPage, SiteDetailPage, SiteSettingsPage, VerifyEmailPage,
};
pub(crate) use state::{AuthState, State, ThemeMode};

#[function_component]
pub fn App() -> Html {
    // Check authentication status on startup
    use_authentication();

    html! {
        <ToastProvider>
            <BrowserRouter>
                <MainLayout>
                    <Switch<Route> render={switch} />
                    <ToastContainer />
                </MainLayout>
            </BrowserRouter>
        </ToastProvider>
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
    Landing,
    #[at("/home")]
    Home,
    #[at("/login")]
    Login,
    #[at("/verify_email")]
    VerifyEmail,
    #[at("/help")]
    Help,
    #[at("/profile")]
    Profile,
    #[at("/forgot_password")]
    ForgotPassword,
    #[at("/reset_password")]
    ResetPassword,
    #[at("/accept-invite/:invite_id")]
    AcceptInvite { invite_id: InviteId },
    #[at("/communities")]
    Communities,
    #[at("/communities/new")]
    CreateCommunity,
    #[at("/communities/:id")]
    CommunityDetail { id: CommunityId },
    #[at("/communities/:id/members")]
    CommunityMembers { id: CommunityId },
    #[at("/communities/:id/invites")]
    CommunityInvites { id: CommunityId },
    #[at("/communities/:id/settings")]
    CommunitySettings { id: CommunityId },
    #[at("/communities/:id/sites/new")]
    CreateSite { id: CommunityId },
    #[at("/sites/:id")]
    SiteAuctions { id: SiteId },
    #[at("/sites/:id/spaces")]
    SiteSpaces { id: SiteId },
    #[at("/sites/:id/auctions/new")]
    CreateAuction { id: SiteId },
    #[at("/auctions/:id")]
    AuctionDetail { id: AuctionId },
    #[at("/auctions/:id/rounds")]
    AuctionRounds { id: AuctionId },
    #[at("/sites/:id/settings")]
    SiteSettings { id: SiteId },
    #[not_found]
    #[at("/404")]
    NotFound,
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Landing => html! { <LoggedOutHomePage /> },
        Route::Home => html! { <LoggedInHomePage /> },
        Route::Login => html! { <LoginPage /> },
        Route::VerifyEmail => html! { <VerifyEmailPage /> },
        Route::Help => html! { <HelpPage /> },
        Route::Profile => html! { <ProfilePage /> },
        Route::ForgotPassword => html! { <ForgotPasswordPage /> },
        Route::ResetPassword => html! { <ResetPasswordPage /> },
        Route::AcceptInvite { invite_id } => {
            html! { <AcceptInvitePage invite_id={invite_id} /> }
        }
        Route::Communities => html! { <CommunitiesPage /> },
        Route::CreateCommunity => html! { <CreateCommunityPage /> },
        Route::CommunityDetail { id } => {
            html! { <CommunityDetailPage community_id={id} /> }
        }
        Route::CommunityMembers { id } => {
            html! { <CommunityMembersPage community_id={id} /> }
        }
        Route::CommunityInvites { id } => {
            html! { <CommunityInvitesPage community_id={id} /> }
        }
        Route::CommunitySettings { id } => {
            html! { <CommunitySettingsPage community_id={id} /> }
        }
        Route::CreateSite { id } => {
            html! { <CreateSitePage community_id={id} /> }
        }
        Route::SiteSpaces { id } => {
            html! { <SiteDetailPage site_id={id} /> }
        }
        Route::SiteAuctions { id } => {
            html! { <SiteAuctionsPage site_id={id} /> }
        }
        Route::CreateAuction { id } => {
            html! { <CreateAuctionPage site_id={id} /> }
        }
        Route::AuctionDetail { id } => {
            html! { <AuctionDetailPage auction_id={id} /> }
        }
        Route::AuctionRounds { id } => {
            html! { <AuctionRoundsPage auction_id={id} /> }
        }
        Route::SiteSettings { id } => {
            html! { <SiteSettingsPage site_id={id} /> }
        }
        Route::NotFound => html! { <NotFoundPage /> },
    }
}
