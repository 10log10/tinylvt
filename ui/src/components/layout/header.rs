use crate::{
    AuthState, Route, State, components::DarkModeToggle, hooks::use_logout,
};
use payloads::responses;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

#[derive(Properties, PartialEq)]
pub struct AuthenticatedUserInfoProps {
    pub profile: responses::UserProfile,
}

#[function_component]
fn AuthenticatedUserInfo(props: &AuthenticatedUserInfoProps) -> Html {
    let logout_handler = use_logout();

    html! {
        <div class="flex items-center gap-2 sm:gap-4">
            <Link<Route> to={Route::Profile}
                classes="text-sm text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white transition-colors">
                {format!("{}", props.profile.username)}
            </Link<Route>>
            <button
                onclick={logout_handler}
                class="text-sm text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white
                       border border-neutral-300 dark:border-neutral-600 px-3 py-1 rounded-md
                       hover:bg-neutral-100 dark:hover:bg-neutral-800 transition-colors"
            >
                {"Logout"}
            </button>
        </div>
    }
}

#[function_component]
fn UnauthenticatedUserActions() -> Html {
    html! {
        <Link<Route> to={Route::Login} classes="text-sm text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white
                                               border border-neutral-300 dark:border-neutral-600 px-3 py-1 rounded-md
                                               hover:bg-neutral-100 dark:hover:bg-neutral-800 transition-colors">
            {"Login"}
        </Link<Route>>
    }
}

#[derive(Properties, PartialEq)]
pub struct NavigationMenuProps {
    pub authenticated: bool,
}

#[function_component]
fn NavigationMenu(props: &NavigationMenuProps) -> Html {
    if props.authenticated {
        html! {
            <nav class="flex gap-4 sm:gap-6">
                <Link<Route> to={Route::Communities} classes="text-sm text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white">
                    {"Communities"}
                </Link<Route>>
            </nav>
        }
    } else {
        html! { <></> }
    }
}

#[derive(Properties, PartialEq)]
pub struct HeaderLeftProps {
    pub authenticated: bool,
}

#[function_component]
fn HeaderLeft(props: &HeaderLeftProps) -> Html {
    html! {
        <div class="flex items-center gap-4 sm:gap-8">
            <Link<Route> to={Route::Landing} classes="text-xl font-semibold text-neutral-900 dark:text-white hover:text-neutral-700 dark:hover:text-neutral-300">
                {"TinyLVT"}
            </Link<Route>>
            <NavigationMenu authenticated={props.authenticated} />
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct HeaderRightProps {
    pub auth_state: AuthState,
}

#[function_component]
fn HeaderRight(props: &HeaderRightProps) -> Html {
    html! {
        <div class="flex items-center gap-2 sm:gap-4">
            {
                match &props.auth_state {
                    AuthState::LoggedIn(profile) => html! {
                        <AuthenticatedUserInfo profile={profile.clone()} />
                    },
                    AuthState::LoggedOut => html! {
                        <UnauthenticatedUserActions />
                    },
                    AuthState::Unknown => html! { <></> }
                }
            }
            <DarkModeToggle />
        </div>
    }
}

#[function_component]
pub fn Header() -> Html {
    let (state, _) = use_store::<State>();

    html! {
        <header class="bg-white dark:bg-neutral-900 border-b border-neutral-200 dark:border-neutral-700">
            <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
                <div class="flex flex-wrap justify-between items-center gap-2 min-h-16 py-2">
                    <HeaderLeft authenticated={state.is_authenticated()} />
                    <HeaderRight auth_state={state.auth_state.clone()} />
                </div>
            </div>
        </header>
    }
}
