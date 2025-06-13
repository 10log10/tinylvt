mod auth;
mod theme;
mod communities;
mod profile;

use payloads::APIClient;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

use auth::{AuthState, ForgotPassword, Login, Register, ResetPassword, VerifyEmailPrompt, use_auth};
use theme::ThemeToggle;
use communities::{Communities, CreateCommunity as CreateCommunityComponent, CommunityInvites, CommunityManage};

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
    let (auth_state, check_auth) = use_auth();
    let (_, auth_dispatch) = use_store::<AuthState>();
    let navigator = use_navigator().unwrap();

    // Check authentication status on mount
    use_effect_with((), move |_| {
        check_auth.emit(());
        || ()
    });

    let on_logout = {
        let auth_dispatch = auth_dispatch.clone();
        let navigator = navigator.clone();

        Callback::from(move |_: MouseEvent| {
            let auth_dispatch = auth_dispatch.clone();
            let navigator = navigator.clone();

            yew::platform::spawn_local(async move {
                let client = get_api_client();
                match client.logout().await {
                    Ok(()) => {
                        // Update auth state
                        auth_dispatch.reduce_mut(|state| {
                            state.is_authenticated = false;
                            state.username = None;
                        });

                        // Navigate to home
                        navigator.push(&Route::Home);
                    }
                    Err(_) => {
                        // Even if logout fails, clear local state
                        auth_dispatch.reduce_mut(|state| {
                            state.is_authenticated = false;
                            state.username = None;
                        });
                        navigator.push(&Route::Home);
                    }
                }
            });
        })
    };

    html! {
        <header class="bg-gray-100 dark:bg-gray-800 shadow-sm border-b border-gray-200 dark:border-gray-700">
            <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
                <div class="flex justify-between items-center h-16">
                    <div class="flex items-center space-x-8">
                        <Link<Route> to={Route::Home} classes="text-xl font-bold text-gray-900 dark:text-white hover:text-gray-700 dark:hover:text-gray-300">
                            {"TinyLVT"}
                        </Link<Route>>
                        
                        if auth_state.is_authenticated {
                            <nav class="flex space-x-4">
                                <Link<Route> 
                                    to={Route::Communities} 
                                    classes="text-gray-600 hover:text-gray-900 dark:text-gray-400 dark:hover:text-white text-sm font-medium transition-colors"
                                >
                                    {"Communities"}
                                </Link<Route>>
                            </nav>
                        }
                    </div>
                    <div class="flex items-center space-x-4">
                        if auth_state.is_authenticated {
                            <Link<Route> 
                                to={Route::Profile} 
                                classes="text-gray-600 hover:text-gray-900 dark:text-gray-400 dark:hover:text-white text-sm font-medium transition-colors"
                            >
                                {"Profile"}
                            </Link<Route>>
                            if let Some(username) = &auth_state.username {
                                <span class="text-sm text-gray-600 dark:text-gray-400">
                                    {"Welcome, "}{username}
                                </span>
                            }
                            <button
                                onclick={on_logout}
                                class="text-sm text-gray-600 hover:text-gray-900 dark:text-gray-400 dark:hover:text-white"
                            >
                                {"Logout"}
                            </button>
                        } else {
                            <Link<Route> to={Route::Login} classes="text-sm text-gray-600 hover:text-gray-900 dark:text-gray-400 dark:hover:text-white">
                                {"Login"}
                            </Link<Route>>
                            <Link<Route> to={Route::Register} classes="bg-blue-600 hover:bg-blue-700 text-white px-3 py-2 rounded-md text-sm font-medium">
                                {"Sign Up"}
                            </Link<Route>>
                        }
                        <ThemeToggle />
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
    #[at("/login")]
    Login,
    #[at("/register")]
    Register,
    #[at("/verify-email")]
    VerifyEmail,
    #[at("/forgot-password")]
    ForgotPassword,
    #[at("/reset-password")]
    ResetPassword,
    #[at("/communities")]
    Communities,
    #[at("/communities/create")]
    CreateCommunity,
    #[at("/communities/invites")]
    CommunityInvites,
    #[at("/communities/:id/manage")]
    CommunityManage { id: String },
    #[at("/profile")]
    Profile,
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
                    Ok(_) => {
                        health_status.set("✅ Connected to backend".to_string())
                    }
                    Err(e) => {
                        health_status.set(format!("❌ Backend error: {}", e))
                    }
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
                    <VerificationNotice />
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
        Route::Login => html! { <Login /> },
        Route::Register => html! { <Register /> },
        Route::VerifyEmail => html! { <VerifyEmailPrompt /> },
        Route::ForgotPassword => html! { <ForgotPassword /> },
        Route::ResetPassword => html! { <ResetPassword /> },
        Route::Communities => html! { <Communities /> },
        Route::CreateCommunity => html! { <CreateCommunityComponent /> },
        Route::CommunityInvites => html! { <CommunityInvites /> },
        Route::CommunityManage { id } => html! { <CommunityManage community_id={id.clone()} /> },
        Route::Profile => html! { <profile::Profile /> },
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

#[function_component]
fn VerificationNotice() -> Html {
    let (auth_state, _) = use_auth();
    let show_notice = use_state(|| false);
    let user_email = use_state(|| None::<String>);
    let is_resending = use_state(|| false);
    let resend_message = use_state(|| None::<String>);

    // Check user profile for email verification status
    {
        let show_notice = show_notice.clone();
        let user_email = user_email.clone();
        let auth_state = auth_state.clone();
        
        use_effect_with(auth_state.clone(), move |auth_state| {
            if auth_state.is_authenticated && !auth_state.is_loading {
                let show_notice = show_notice.clone();
                let user_email = user_email.clone();
                
                yew::platform::spawn_local(async move {
                    let client = get_api_client();
                    match client.user_profile().await {
                        Ok(profile) => {
                            // Show notice only if email is not verified
                            if !profile.email_verified {
                                show_notice.set(true);
                                user_email.set(Some(profile.email));
                            } else {
                                // Email is verified, clear any leftover session flags
                                let window = web_sys::window().unwrap();
                                if let Ok(Some(session_storage)) = window.session_storage() {
                                    let _ = session_storage.remove_item("show_verification_notice");
                                    let _ = session_storage.remove_item("verification_email");
                                }
                                show_notice.set(false);
                            }
                        }
                        Err(_) => {
                            // Failed to fetch profile, don't show notice
                            show_notice.set(false);
                        }
                    }
                });
            } else {
                // User not authenticated, don't show notice
                show_notice.set(false);
            }
            || ()
        });
    }

    let on_dismiss = {
        let show_notice = show_notice.clone();
        Callback::from(move |_: MouseEvent| {
            // Clear the notice from session storage and hide
            let window = web_sys::window().unwrap();
            if let Ok(Some(session_storage)) = window.session_storage() {
                let _ = session_storage.remove_item("show_verification_notice");
                let _ = session_storage.remove_item("verification_email");
            }
            show_notice.set(false);
        })
    };

    let on_resend = {
        let user_email = user_email.clone();
        let is_resending = is_resending.clone();
        let resend_message = resend_message.clone();

        Callback::from(move |_: MouseEvent| {
            if let Some(email_addr) = (*user_email).clone() {
                let is_resending = is_resending.clone();
                let resend_message = resend_message.clone();

                yew::platform::spawn_local(async move {
                    is_resending.set(true);
                    resend_message.set(None);

                    let client = get_api_client();
                    let request = payloads::requests::ResendVerificationEmail {
                        email: email_addr,
                    };

                    match client.resend_verification_email(&request).await {
                        Ok(_) => {
                            resend_message.set(Some("Verification email sent!".to_string()));
                        }
                        Err(e) => {
                            resend_message.set(Some(format!("Error: {}", e)));
                        }
                    }

                    is_resending.set(false);
                });
            }
        })
    };

    if *show_notice {
        html! {
            <div class="bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-md p-4">
                <div class="flex">
                    <div class="flex-shrink-0">
                        <svg class="h-5 w-5 text-yellow-400" viewBox="0 0 20 20" fill="currentColor">
                            <path fill-rule="evenodd" d="M2.94 6.412A2 2 0 002 8.108V16a2 2 0 002 2h12a2 2 0 002-2V8.108a2 2 0 00-.94-1.696l-6-3.75a2 2 0 00-2.12 0l-6 3.75zm2.615 2.423a1 1 0 10-1.11 1.664l5 3.333a1 1 0 001.11 0l5-3.333a1 1 0 00-1.11-1.664L10 12.027l-4.445-2.962z" clip-rule="evenodd"></path>
                        </svg>
                    </div>
                    <div class="ml-3 flex-1">
                        <h3 class="text-sm font-medium text-yellow-800 dark:text-yellow-200">
                            {"Welcome! Please verify your email"}
                        </h3>
                        <div class="mt-2 text-sm text-yellow-700 dark:text-yellow-300">
                            <p>
                                {"We've sent a verification link to "}
                                if let Some(email_addr) = (*user_email).clone() {
                                    <strong>{email_addr}</strong>
                                } else {
                                    {"your email address"}
                                }
                                {". Please check your inbox and click the link to verify your email."}
                            </p>
                        </div>
                        if let Some(message) = (*resend_message).clone() {
                            <div class="mt-2 text-sm font-medium text-yellow-800 dark:text-yellow-200">
                                {message}
                            </div>
                        }
                        <div class="mt-4 flex space-x-3">
                            <button
                                onclick={on_resend}
                                disabled={*is_resending}
                                class="text-sm bg-yellow-100 text-yellow-800 hover:bg-yellow-200 dark:bg-yellow-800 dark:text-yellow-100 dark:hover:bg-yellow-700 px-3 py-2 rounded-md font-medium disabled:opacity-50 disabled:cursor-not-allowed"
                            >
                                if *is_resending {
                                    {"Sending..."}
                                } else {
                                    {"Resend verification email"}
                                }
                            </button>
                        </div>
                    </div>
                    <div class="ml-auto pl-3">
                        <div class="-mx-1.5 -my-1.5">
                            <button
                                onclick={on_dismiss}
                                class="inline-flex bg-yellow-50 dark:bg-yellow-900/20 rounded-md p-1.5 text-yellow-500 hover:bg-yellow-100 dark:hover:bg-yellow-900/40 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-offset-yellow-50 focus:ring-yellow-600"
                            >
                                <span class="sr-only">{"Dismiss"}</span>
                                <svg class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
                                    <path fill-rule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clip-rule="evenodd"></path>
                                </svg>
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        }
    } else {
        html! {}
    }
}
