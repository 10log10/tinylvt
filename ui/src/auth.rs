use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

use crate::{Route, get_api_client};
use payloads::requests::CreateAccount;

// Authentication state management
#[derive(Default, Clone, PartialEq, Store)]
pub struct AuthState {
    pub is_authenticated: bool,
    pub username: Option<String>,
    pub is_loading: bool,
}

// Form states for different auth pages
#[derive(Default, Clone, PartialEq)]
struct LoginForm {
    username: String,
    password: String,
    is_loading: bool,
    error: Option<String>,
}

#[derive(Default, Clone, PartialEq)]
struct RegisterForm {
    email: String,
    username: String,
    password: String,
    confirm_password: String,
    is_loading: bool,
    error: Option<String>,
}

#[derive(Default, Clone, PartialEq)]
struct ForgotForm {
    email: String,
    is_loading: bool,
    message: Option<String>,
    error: Option<String>,
}

// Authentication hook
#[hook]
pub fn use_auth() -> (AuthState, Callback<()>) {
    let (state, dispatch) = use_store::<AuthState>();

    let check_auth = use_callback(
        dispatch.clone(),
        move |_: (), dispatch: &Dispatch<AuthState>| {
            let dispatch = dispatch.clone();
            yew::platform::spawn_local(async move {
                dispatch.reduce_mut(|state| state.is_loading = true);

                let client = get_api_client();
                match client.login_check().await {
                    Ok(is_authenticated) => {
                        if is_authenticated {
                            // User is authenticated, now fetch their profile to get username
                            match client.user_profile().await {
                                Ok(profile) => {
                                    dispatch.reduce_mut(|state| {
                                        state.is_authenticated = true;
                                        state.username = Some(profile.username);
                                        state.is_loading = false;
                                    });
                                }
                                Err(_) => {
                                    // Profile fetch failed, but user is authenticated
                                    dispatch.reduce_mut(|state| {
                                        state.is_authenticated = true;
                                        state.username = None;
                                        state.is_loading = false;
                                    });
                                }
                            }
                        } else {
                            dispatch.reduce_mut(|state| {
                                state.is_authenticated = false;
                                state.username = None;
                                state.is_loading = false;
                            });
                        }
                    }
                    Err(_) => {
                        dispatch.reduce_mut(|state| {
                            state.is_authenticated = false;
                            state.username = None;
                            state.is_loading = false;
                        });
                    }
                }
            });
        },
    );

    ((*state).clone(), check_auth)
}

// Login Component
#[function_component]
pub fn Login() -> Html {
    let navigator = use_navigator().unwrap();
    let form = use_state(LoginForm::default);
    let (_, auth_dispatch) = use_store::<AuthState>();

    let on_username_change = {
        let form = form.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.username = input.value();
            form.set(form_data);
        })
    };

    let on_password_change = {
        let form = form.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.password = input.value();
            form.set(form_data);
        })
    };

    let on_submit = {
        let form = form.clone();
        let navigator = navigator.clone();
        let auth_dispatch = auth_dispatch.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let form_data = (*form).clone();
            if form_data.username.is_empty() || form_data.password.is_empty() {
                let mut new_form = form_data;
                new_form.error = Some("Please fill in all fields".to_string());
                form.set(new_form);
                return;
            }

            let form = form.clone();
            let navigator = navigator.clone();
            let auth_dispatch = auth_dispatch.clone();
            let username_for_state = form_data.username.clone();
            let password_for_login = form_data.password.clone();

            yew::platform::spawn_local(async move {
                // Set loading state
                {
                    let mut new_form = form_data;
                    new_form.is_loading = true;
                    new_form.error = None;
                    form.set(new_form);
                }

                let client = get_api_client();

                // Note: The API client has a bug - login() takes CreateAccount instead of LoginCredentials
                // We'll use the create_account struct but call login
                let login_details = CreateAccount {
                    email: "".to_string(), // Not used for login
                    username: username_for_state.clone(),
                    password: password_for_login,
                };

                match client.login(&login_details).await {
                    Ok(()) => {
                        // Update auth state
                        auth_dispatch.reduce_mut(|state| {
                            state.is_authenticated = true;
                            state.username = Some(username_for_state);
                        });

                        // Navigate to home
                        navigator.push(&Route::Home);
                    }
                    Err(e) => {
                        let mut new_form = (*form).clone();
                        new_form.is_loading = false;
                        new_form.error = Some(format!("Login failed: {}", e));
                        form.set(new_form);
                    }
                }
            });
        })
    };

    html! {
        <main class="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900 py-12 px-4 sm:px-6 lg:px-8">
            <div class="max-w-md w-full space-y-8">
                <div>
                    <h2 class="mt-6 text-center text-3xl font-extrabold text-gray-900 dark:text-white">
                        {"Sign in to your account"}
                    </h2>
                    <p class="mt-2 text-center text-sm text-gray-600 dark:text-gray-400">
                        {"Or "}
                        <Link<Route> to={Route::Register} classes="font-medium text-blue-600 hover:text-blue-500 dark:text-blue-400">
                            {"create a new account"}
                        </Link<Route>>
                    </p>
                </div>
                <form class="mt-8 space-y-6" onsubmit={on_submit}>
                    <div class="rounded-md shadow-sm -space-y-px">
                        <div>
                            <label for="username" class="sr-only">{"Username"}</label>
                            <input
                                id="username"
                                name="username"
                                type="text"
                                required=true
                                class="appearance-none rounded-none relative block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 placeholder-gray-500 dark:placeholder-gray-400 text-gray-900 dark:text-white bg-white dark:bg-gray-700 rounded-t-md focus:outline-none focus:ring-blue-500 focus:border-blue-500 focus:z-10 sm:text-sm"
                                placeholder="Username"
                                value={form.username.clone()}
                                onchange={on_username_change}
                                disabled={form.is_loading}
                            />
                        </div>
                        <div>
                            <label for="password" class="sr-only">{"Password"}</label>
                            <input
                                id="password"
                                name="password"
                                type="password"
                                required=true
                                class="appearance-none rounded-none relative block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 placeholder-gray-500 dark:placeholder-gray-400 text-gray-900 dark:text-white bg-white dark:bg-gray-700 rounded-b-md focus:outline-none focus:ring-blue-500 focus:border-blue-500 focus:z-10 sm:text-sm"
                                placeholder="Password"
                                value={form.password.clone()}
                                onchange={on_password_change}
                                disabled={form.is_loading}
                            />
                        </div>
                    </div>

                    if let Some(error) = &form.error {
                        <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded">
                            {error}
                        </div>
                    }

                    <div class="flex items-center justify-between">
                        <Link<Route> to={Route::ForgotPassword} classes="text-sm text-blue-600 hover:text-blue-500 dark:text-blue-400">
                            {"Forgot your password?"}
                        </Link<Route>>
                    </div>

                    <div>
                        <button
                            type="submit"
                            class="group relative w-full flex justify-center py-2 px-4 border border-transparent text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
                            disabled={form.is_loading}
                        >
                            if form.is_loading {
                                <span class="flex items-center">
                                    <svg class="animate-spin -ml-1 mr-3 h-5 w-5 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                    </svg>
                                    {"Signing in..."}
                                </span>
                            } else {
                                {"Sign in"}
                            }
                        </button>
                    </div>
                </form>
            </div>
        </main>
    }
}

// Register Component
#[function_component]
pub fn Register() -> Html {
    let navigator = use_navigator().unwrap();
    let form = use_state(RegisterForm::default);

    let on_email_change = {
        let form = form.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.email = input.value();
            form.set(form_data);
        })
    };

    let on_username_change = {
        let form = form.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.username = input.value();
            form.set(form_data);
        })
    };

    let on_password_change = {
        let form = form.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.password = input.value();
            form.set(form_data);
        })
    };

    let on_confirm_password_change = {
        let form = form.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.confirm_password = input.value();
            form.set(form_data);
        })
    };

    let on_submit = {
        let form = form.clone();
        let navigator = navigator.clone();
        let (_, auth_dispatch) = use_store::<AuthState>();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let form_data = (*form).clone();

            // Validation
            if form_data.email.is_empty()
                || form_data.username.is_empty()
                || form_data.password.is_empty()
                || form_data.confirm_password.is_empty()
            {
                let mut new_form = form_data;
                new_form.error = Some("Please fill in all fields".to_string());
                form.set(new_form);
                return;
            }

            if form_data.password != form_data.confirm_password {
                let mut new_form = form_data;
                new_form.error = Some("Passwords do not match".to_string());
                form.set(new_form);
                return;
            }

            if form_data.password.len() < 8 {
                let mut new_form = form_data;
                new_form.error =
                    Some("Password must be at least 8 characters".to_string());
                form.set(new_form);
                return;
            }

            let form = form.clone();
            let navigator = navigator.clone();
            let auth_dispatch = auth_dispatch.clone();
            let email = form_data.email.clone();
            let username = form_data.username.clone();
            let password = form_data.password.clone();

            yew::platform::spawn_local(async move {
                // Set loading state
                {
                    let mut new_form = (*form).clone();
                    new_form.is_loading = true;
                    new_form.error = None;
                    form.set(new_form);
                }

                let client = get_api_client();
                let account_details = CreateAccount {
                    email: email.clone(),
                    username: username.clone(),
                    password: password.clone(),
                };

                match client.create_account(&account_details).await {
                    Ok(()) => {
                        // Registration successful, now try to auto-login
                        let login_details = CreateAccount {
                            email: "".to_string(), // Not used for login
                            username: username.clone(),
                            password: password.clone(),
                        };

                        match client.login(&login_details).await {
                            Ok(()) => {
                                // Auto-login successful, update auth state and redirect to home
                                auth_dispatch.reduce_mut(|state| {
                                    state.is_authenticated = true;
                                    state.username = Some(username);
                                });

                                // Navigate to home - the verification notice will show automatically 
                                // based on the user's actual verification status
                                navigator.push(&Route::Home);
                            }
                            Err(_) => {
                                // Auto-login failed, fall back to verification prompt
                                let window = web_sys::window().unwrap();
                                let session_storage = window.session_storage().unwrap().unwrap();
                                let _ = session_storage.set_item("pending_verification_email", &email);
                                navigator.push(&Route::VerifyEmail);
                            }
                        }
                    }
                    Err(e) => {
                        let mut new_form = (*form).clone();
                        new_form.is_loading = false;
                        new_form.error =
                            Some(format!("Registration failed: {}", e));
                        form.set(new_form);
                    }
                }
            });
        })
    };

    html! {
        <main class="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900 py-12 px-4 sm:px-6 lg:px-8">
            <div class="max-w-md w-full space-y-8">
                <div>
                    <h2 class="mt-6 text-center text-3xl font-extrabold text-gray-900 dark:text-white">
                        {"Create your account"}
                    </h2>
                    <p class="mt-2 text-center text-sm text-gray-600 dark:text-gray-400">
                        {"Or "}
                        <Link<Route> to={Route::Login} classes="font-medium text-blue-600 hover:text-blue-500 dark:text-blue-400">
                            {"sign in to your existing account"}
                        </Link<Route>>
                    </p>
                </div>
                <form class="mt-8 space-y-6" onsubmit={on_submit}>
                    <div class="space-y-4">
                        <div>
                            <label for="email" class="block text-sm font-medium text-gray-700 dark:text-gray-300">{"Email address"}</label>
                            <input
                                id="email"
                                name="email"
                                type="email"
                                required=true
                                class="mt-1 appearance-none relative block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 placeholder-gray-500 dark:placeholder-gray-400 text-gray-900 dark:text-white bg-white dark:bg-gray-700 rounded-md focus:outline-none focus:ring-blue-500 focus:border-blue-500 sm:text-sm"
                                placeholder="Email address"
                                value={form.email.clone()}
                                onchange={on_email_change}
                                disabled={form.is_loading}
                            />
                        </div>
                        <div>
                            <label for="username" class="block text-sm font-medium text-gray-700 dark:text-gray-300">{"Username"}</label>
                            <input
                                id="username"
                                name="username"
                                type="text"
                                required=true
                                class="mt-1 appearance-none relative block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 placeholder-gray-500 dark:placeholder-gray-400 text-gray-900 dark:text-white bg-white dark:bg-gray-700 rounded-md focus:outline-none focus:ring-blue-500 focus:border-blue-500 sm:text-sm"
                                placeholder="Username"
                                value={form.username.clone()}
                                onchange={on_username_change}
                                disabled={form.is_loading}
                            />
                        </div>
                        <div>
                            <label for="password" class="block text-sm font-medium text-gray-700 dark:text-gray-300">{"Password"}</label>
                            <input
                                id="password"
                                name="password"
                                type="password"
                                required=true
                                class="mt-1 appearance-none relative block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 placeholder-gray-500 dark:placeholder-gray-400 text-gray-900 dark:text-white bg-white dark:bg-gray-700 rounded-md focus:outline-none focus:ring-blue-500 focus:border-blue-500 sm:text-sm"
                                placeholder="Password (min 8 characters)"
                                value={form.password.clone()}
                                onchange={on_password_change}
                                disabled={form.is_loading}
                            />
                        </div>
                        <div>
                            <label for="confirm-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300">{"Confirm Password"}</label>
                            <input
                                id="confirm-password"
                                name="confirm-password"
                                type="password"
                                required=true
                                class="mt-1 appearance-none relative block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 placeholder-gray-500 dark:placeholder-gray-400 text-gray-900 dark:text-white bg-white dark:bg-gray-700 rounded-md focus:outline-none focus:ring-blue-500 focus:border-blue-500 sm:text-sm"
                                placeholder="Confirm password"
                                value={form.confirm_password.clone()}
                                onchange={on_confirm_password_change}
                                disabled={form.is_loading}
                            />
                        </div>
                    </div>

                    if let Some(error) = &form.error {
                        <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded">
                            {error}
                        </div>
                    }

                    <div>
                        <button
                            type="submit"
                            class="group relative w-full flex justify-center py-2 px-4 border border-transparent text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
                            disabled={form.is_loading}
                        >
                            if form.is_loading {
                                <span class="flex items-center">
                                    <svg class="animate-spin -ml-1 mr-3 h-5 w-5 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                    </svg>
                                    {"Creating account..."}
                                </span>
                            } else {
                                {"Create account"}
                            }
                        </button>
                    </div>
                </form>
            </div>
        </main>
    }
}

// Forgot Password Component
#[function_component]
pub fn ForgotPassword() -> Html {
    let form = use_state(ForgotForm::default);

    let on_email_change = {
        let form = form.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.email = input.value();
            form.set(form_data);
        })
    };

    let on_submit = {
        let form = form.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let form_data = (*form).clone();

            if form_data.email.is_empty() {
                let mut new_form = form_data;
                new_form.error =
                    Some("Please enter your email address".to_string());
                form.set(new_form);
                return;
            }

            let form = form.clone();
            let email = form_data.email.clone();

            yew::platform::spawn_local(async move {
                // Set loading state
                {
                    let mut new_form = form_data;
                    new_form.is_loading = true;
                    new_form.error = None;
                    new_form.message = None;
                    form.set(new_form);
                }

                let client = get_api_client();
                let forgot_request = payloads::requests::ForgotPassword { email };

                match client.forgot_password(&forgot_request).await {
                    Ok(response) => {
                        let mut new_form = (*form).clone();
                        new_form.is_loading = false;
                        new_form.message = Some(response.message);
                        form.set(new_form);
                    }
                    Err(e) => {
                        let mut new_form = (*form).clone();
                        new_form.is_loading = false;
                        new_form.error = Some(format!("Error: {}", e));
                        form.set(new_form);
                    }
                }
            });
        })
    };

    html! {
        <main class="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900 py-12 px-4 sm:px-6 lg:px-8">
            <div class="max-w-md w-full space-y-8">
                <div>
                    <h2 class="mt-6 text-center text-3xl font-extrabold text-gray-900 dark:text-white">
                        {"Reset your password"}
                    </h2>
                    <p class="mt-2 text-center text-sm text-gray-600 dark:text-gray-400">
                        {"Remember your password? "}
                        <Link<Route> to={Route::Login} classes="font-medium text-blue-600 hover:text-blue-500 dark:text-blue-400">
                            {"Sign in"}
                        </Link<Route>>
                    </p>
                </div>
                <form class="mt-8 space-y-6" onsubmit={on_submit}>
                    <div>
                        <label for="email" class="block text-sm font-medium text-gray-700 dark:text-gray-300">{"Email address"}</label>
                        <input
                            id="email"
                            name="email"
                            type="email"
                            required=true
                            class="mt-1 appearance-none relative block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 placeholder-gray-500 dark:placeholder-gray-400 text-gray-900 dark:text-white bg-white dark:bg-gray-700 rounded-md focus:outline-none focus:ring-blue-500 focus:border-blue-500 sm:text-sm"
                            placeholder="Enter your email address"
                            value={form.email.clone()}
                            onchange={on_email_change}
                            disabled={form.is_loading}
                        />
                    </div>

                    if let Some(error) = &form.error {
                        <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded">
                            {error}
                        </div>
                    }

                    if let Some(message) = &form.message {
                        <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 text-green-600 dark:text-green-400 px-4 py-3 rounded">
                            {message}
                        </div>
                    }

                    <div>
                        <button
                            type="submit"
                            class="group relative w-full flex justify-center py-2 px-4 border border-transparent text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
                            disabled={form.is_loading}
                        >
                            if form.is_loading {
                                <span class="flex items-center">
                                    <svg class="animate-spin -ml-1 mr-3 h-5 w-5 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                    </svg>
                                    {"Sending..."}
                                </span>
                            } else {
                                {"Send reset link"}
                            }
                        </button>
                    </div>
                </form>
            </div>
        </main>
    }
}

// Email Verification Prompt Component
#[function_component]
pub fn VerifyEmailPrompt() -> Html {
    let navigator = use_navigator().unwrap();
    let email = use_state(|| None::<String>);
    let is_resending = use_state(|| false);
    let resend_message = use_state(|| None::<String>);
    let token = use_state(|| None::<String>);

    // Check URL for token parameter and get email from session storage
    {
        let email = email.clone();
        let token = token.clone();
        use_effect_with((), move |_| {
            let window = web_sys::window().unwrap();
            
            // Check for token in URL
            let location = window.location();
            let mut found_token = false;
            if let Ok(search) = location.search() {
                if !search.is_empty() {
                    // Parse query string (starts with '?')
                    let query_string = &search[1..]; // Remove the '?' prefix
                    for param in query_string.split('&') {
                        if let Some((key, value)) = param.split_once('=') {
                            if key == "token" {
                                token.set(Some(value.to_string()));
                                found_token = true;
                                break;
                            }
                        }
                    }
                }
            }
            
            // If no token, get email from session storage for regular prompt
            if !found_token {
                if let Ok(Some(session_storage)) = window.session_storage() {
                    if let Ok(stored_email) = session_storage.get_item("pending_verification_email") {
                        email.set(stored_email);
                    }
                }
            }
            || ()
        });
    }

    // If we have a token, render the verification component
    if let Some(token_value) = (*token).clone() {
        return html! { <VerifyEmailWithToken token={token_value} /> };
    }

    let on_resend = {
        let email = email.clone();
        let is_resending = is_resending.clone();
        let resend_message = resend_message.clone();

        Callback::from(move |_: MouseEvent| {
            if let Some(email_addr) = (*email).clone() {
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
                            resend_message.set(Some("Verification email sent! Please check your inbox.".to_string()));
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

    let on_back_to_login = {
        let navigator = navigator.clone();
        Callback::from(move |_: MouseEvent| {
            // Clear session storage
            let window = web_sys::window().unwrap();
            if let Ok(Some(session_storage)) = window.session_storage() {
                let _ = session_storage.remove_item("pending_verification_email");
            }
            navigator.push(&Route::Login);
        })
    };

    html! {
        <main class="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900 py-12 px-4 sm:px-6 lg:px-8">
            <div class="max-w-md w-full space-y-8">
                <div class="text-center">
                    <div class="mx-auto h-12 w-12 text-yellow-400">
                        <svg fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
                            <path fill-rule="evenodd" d="M2.94 6.412A2 2 0 002 8.108V16a2 2 0 002 2h12a2 2 0 002-2V8.108a2 2 0 00-.94-1.696l-6-3.75a2 2 0 00-2.12 0l-6 3.75zm2.615 2.423a1 1 0 10-1.11 1.664l5 3.333a1 1 0 001.11 0l5-3.333a1 1 0 00-1.11-1.664L10 12.027l-4.445-2.962z" clip-rule="evenodd"></path>
                        </svg>
                    </div>
                    <h2 class="mt-6 text-center text-3xl font-extrabold text-gray-900 dark:text-white">
                        {"Check your email"}
                    </h2>
                    <p class="mt-2 text-center text-sm text-gray-600 dark:text-gray-400">
                        {"We've sent a verification link to your email address."}
                    </p>
                    if let Some(email_addr) = (*email).clone() {
                        <p class="mt-1 text-center text-sm font-medium text-gray-900 dark:text-white">
                            {email_addr}
                        </p>
                    }
                </div>

                <div class="mt-8 space-y-6">
                    <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-md p-4">
                        <div class="flex">
                            <div class="flex-shrink-0">
                                <svg class="h-5 w-5 text-blue-400" viewBox="0 0 20 20" fill="currentColor">
                                    <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd"></path>
                                </svg>
                            </div>
                            <div class="ml-3">
                                <h3 class="text-sm font-medium text-blue-800 dark:text-blue-200">
                                    {"Next steps:"}
                                </h3>
                                <div class="mt-2 text-sm text-blue-700 dark:text-blue-300">
                                    <ul class="list-disc list-inside space-y-1">
                                        <li>{"Check your email inbox"}</li>
                                        <li>{"Click the verification link in the email"}</li>
                                        <li>{"Return here to sign in"}</li>
                                    </ul>
                                </div>
                            </div>
                        </div>
                    </div>

                    if let Some(message) = (*resend_message).clone() {
                        <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 text-green-700 dark:text-green-300 px-4 py-3 rounded">
                            {message}
                        </div>
                    }

                    <div class="flex flex-col space-y-4">
                        <button
                            onclick={on_resend}
                            disabled={*is_resending}
                            class="w-full flex justify-center py-2 px-4 border border-transparent rounded-md shadow-sm text-sm font-medium text-blue-600 bg-blue-100 hover:bg-blue-200 dark:bg-blue-900 dark:text-blue-200 dark:hover:bg-blue-800 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                            if *is_resending {
                                <span class="flex items-center">
                                    <svg class="animate-spin -ml-1 mr-3 h-5 w-5" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                    </svg>
                                    {"Sending..."}
                                </span>
                            } else {
                                {"Resend verification email"}
                            }
                        </button>

                        <button
                            onclick={on_back_to_login}
                            class="w-full flex justify-center py-2 px-4 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 hover:bg-gray-50 dark:hover:bg-gray-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                        >
                            {"Back to login"}
                        </button>
                    </div>
                </div>
            </div>
        </main>
    }
}

// Email Verification with Token Component
#[derive(Properties, PartialEq)]
pub struct VerifyEmailWithTokenProps {
    pub token: String,
}

#[function_component]
pub fn VerifyEmailWithToken(props: &VerifyEmailWithTokenProps) -> Html {
    let navigator = use_navigator().unwrap();
    let (_, auth_dispatch) = use_store::<AuthState>();
    let verification_state = use_state(|| "verifying".to_string()); // "verifying", "success", "error"
    let error_message = use_state(|| None::<String>);

    // Verify email on component mount
    {
        let token = props.token.clone();
        let verification_state = verification_state.clone();
        let error_message = error_message.clone();
        let auth_dispatch = auth_dispatch.clone();

        use_effect_with(token.clone(), move |token| {
            let token = token.clone();
            let verification_state = verification_state.clone();
            let error_message = error_message.clone();
            let auth_dispatch = auth_dispatch.clone();

            yew::platform::spawn_local(async move {
                let client = get_api_client();
                let request = payloads::requests::VerifyEmail { token };

                match client.verify_email(&request).await {
                    Ok(_) => {
                        verification_state.set("success".to_string());
                        
                        // Clear any pending verification email from session
                        let window = web_sys::window().unwrap();
                        if let Ok(Some(session_storage)) = window.session_storage() {
                            let _ = session_storage.remove_item("pending_verification_email");
                            let _ = session_storage.remove_item("show_verification_notice");
                            let _ = session_storage.remove_item("verification_email");
                        }

                        // Trigger a re-fetch of user profile to update verification status
                        // This will cause the verification notice to disappear
                        if let Ok(profile) = client.user_profile().await {
                            auth_dispatch.reduce_mut(|state| {
                                // Update username if we have it
                                state.username = Some(profile.username);
                            });
                        }
                    }
                    Err(e) => {
                        verification_state.set("error".to_string());
                        error_message.set(Some(format!("{}", e)));
                    }
                }
            });

            || ()
        });
    }

    let on_go_to_login = {
        let navigator = navigator.clone();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::Login);
        })
    };

    match verification_state.as_str() {
        "verifying" => html! {
            <main class="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900 py-12 px-4 sm:px-6 lg:px-8">
                <div class="max-w-md w-full space-y-8 text-center">
                    <div>
                        <div class="mx-auto h-12 w-12">
                            <svg class="animate-spin h-12 w-12 text-blue-600" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                            </svg>
                        </div>
                        <h2 class="mt-6 text-center text-3xl font-extrabold text-gray-900 dark:text-white">
                            {"Verifying your email..."}
                        </h2>
                        <p class="mt-2 text-center text-sm text-gray-600 dark:text-gray-400">
                            {"Please wait while we verify your email address."}
                        </p>
                    </div>
                </div>
            </main>
        },
        "success" => html! {
            <main class="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900 py-12 px-4 sm:px-6 lg:px-8">
                <div class="max-w-md w-full space-y-8 text-center">
                    <div>
                        <div class="mx-auto h-12 w-12 text-green-400">
                            <svg class="h-12 w-12" fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
                                <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.857-9.809a.75.75 0 00-1.214-.882l-3.236 4.53L7.73 10.06a.75.75 0 00-1.06 1.061l2.5 2.5a.75.75 0 001.137-.089l4-5.5z" clip-rule="evenodd"></path>
                            </svg>
                        </div>
                        <h2 class="mt-6 text-center text-3xl font-extrabold text-gray-900 dark:text-white">
                            {"Email verified successfully!"}
                        </h2>
                        <p class="mt-2 text-center text-sm text-gray-600 dark:text-gray-400">
                            {"Your email has been verified. You can now sign in to your account."}
                        </p>
                    </div>

                    <div class="mt-8">
                        <button
                            onclick={on_go_to_login}
                            class="w-full flex justify-center py-2 px-4 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-green-600 hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500"
                        >
                            {"Continue to sign in"}
                        </button>
                    </div>
                </div>
            </main>
        },
        "error" => html! {
            <main class="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900 py-12 px-4 sm:px-6 lg:px-8">
                <div class="max-w-md w-full space-y-8 text-center">
                    <div>
                        <div class="mx-auto h-12 w-12 text-red-400">
                            <svg fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
                                <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clip-rule="evenodd"></path>
                            </svg>
                        </div>
                        <h2 class="mt-6 text-center text-3xl font-extrabold text-gray-900 dark:text-white">
                            {"Verification failed"}
                        </h2>
                        <p class="mt-2 text-center text-sm text-gray-600 dark:text-gray-400">
                            {"There was a problem verifying your email address."}
                        </p>
                        if let Some(error) = (*error_message).clone() {
                            <div class="mt-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded">
                                {error}
                            </div>
                        }
                    </div>

                    <div class="mt-8 flex flex-col space-y-4">
                        <button
                            onclick={on_go_to_login}
                            class="w-full flex justify-center py-2 px-4 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                        >
                            {"Go to sign in"}
                        </button>
                        <Link<Route> to={Route::VerifyEmail} classes="w-full flex justify-center py-2 px-4 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 hover:bg-gray-50 dark:hover:bg-gray-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500">
                            {"Try verification again"}
                        </Link<Route>>
                    </div>
                </div>
            </main>
        },
        _ => html! { <div>{"Unknown state"}</div> }
    }
}

// Reset Password Component
#[derive(Default, Clone, PartialEq)]
struct ResetPasswordForm {
    password: String,
    confirm_password: String,
    is_loading: bool,
    message: Option<String>,
    error: Option<String>,
}

#[function_component]
pub fn ResetPassword() -> Html {
    let navigator = use_navigator().unwrap();
    let form = use_state(ResetPasswordForm::default);
    let token = use_state(|| None::<String>);

    // Extract token from URL query parameters
    {
        let token = token.clone();
        use_effect_with((), move |_| {
            let window = web_sys::window().unwrap();
            let location = window.location();
            
            if let Ok(search) = location.search() {
                if !search.is_empty() {
                    // Parse query string (starts with '?')
                    let query_string = &search[1..]; // Remove the '?' prefix
                    for param in query_string.split('&') {
                        if let Some((key, value)) = param.split_once('=') {
                            if key == "token" {
                                token.set(Some(value.to_string()));
                                break;
                            }
                        }
                    }
                }
            }
            || ()
        });
    }

    let on_password_change = {
        let form = form.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.password = input.value();
            form.set(form_data);
        })
    };

    let on_confirm_password_change = {
        let form = form.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.confirm_password = input.value();
            form.set(form_data);
        })
    };

    let on_submit = {
        let form = form.clone();
        let token = token.clone();
        let navigator = navigator.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let form_data = (*form).clone();
            let token_value = (*token).clone();

            // Validation
            if token_value.is_none() {
                let mut new_form = form_data;
                new_form.error = Some("Invalid or missing reset token".to_string());
                form.set(new_form);
                return;
            }

            if form_data.password.is_empty() || form_data.confirm_password.is_empty() {
                let mut new_form = form_data;
                new_form.error = Some("Please fill in all fields".to_string());
                form.set(new_form);
                return;
            }

            if form_data.password != form_data.confirm_password {
                let mut new_form = form_data;
                new_form.error = Some("Passwords do not match".to_string());
                form.set(new_form);
                return;
            }

            if form_data.password.len() < 8 {
                let mut new_form = form_data;
                new_form.error = Some("Password must be at least 8 characters".to_string());
                form.set(new_form);
                return;
            }

            let form = form.clone();
            let navigator = navigator.clone();
            let token = token_value.unwrap();
            let password = form_data.password.clone();

            yew::platform::spawn_local(async move {
                // Set loading state
                {
                    let mut new_form = form_data;
                    new_form.is_loading = true;
                    new_form.error = None;
                    new_form.message = None;
                    form.set(new_form);
                }

                let client = get_api_client();
                let reset_request = payloads::requests::ResetPassword {
                    token,
                    password,
                };

                match client.reset_password(&reset_request).await {
                    Ok(response) => {
                        let mut new_form = (*form).clone();
                        new_form.is_loading = false;
                        new_form.message = Some(response.message);
                        form.set(new_form);

                        // Navigate to login after a short delay
                        yew::platform::spawn_local(async move {
                            yew::platform::time::sleep(std::time::Duration::from_secs(2)).await;
                            navigator.push(&Route::Login);
                        });
                    }
                    Err(e) => {
                        let mut new_form = (*form).clone();
                        new_form.is_loading = false;
                        new_form.error = Some(format!("Password reset failed: {}", e));
                        form.set(new_form);
                    }
                }
            });
        })
    };

    // Show loading state if no token yet
    if token.is_none() {
        return html! {
            <main class="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900 py-12 px-4 sm:px-6 lg:px-8">
                <div class="max-w-md w-full space-y-8 text-center">
                    <div>
                        <div class="mx-auto h-12 w-12">
                            <svg class="animate-spin h-12 w-12 text-blue-600" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                            </svg>
                        </div>
                        <h2 class="mt-6 text-center text-3xl font-extrabold text-gray-900 dark:text-white">
                            {"Loading..."}
                        </h2>
                    </div>
                </div>
            </main>
        };
    }

    html! {
        <main class="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900 py-12 px-4 sm:px-6 lg:px-8">
            <div class="max-w-md w-full space-y-8">
                <div>
                    <h2 class="mt-6 text-center text-3xl font-extrabold text-gray-900 dark:text-white">
                        {"Reset your password"}
                    </h2>
                    <p class="mt-2 text-center text-sm text-gray-600 dark:text-gray-400">
                        {"Enter your new password below"}
                    </p>
                </div>
                <form class="mt-8 space-y-6" onsubmit={on_submit}>
                    <div class="space-y-4">
                        <div>
                            <label for="password" class="block text-sm font-medium text-gray-700 dark:text-gray-300">{"New Password"}</label>
                            <input
                                id="password"
                                name="password"
                                type="password"
                                required=true
                                class="mt-1 appearance-none relative block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 placeholder-gray-500 dark:placeholder-gray-400 text-gray-900 dark:text-white bg-white dark:bg-gray-700 rounded-md focus:outline-none focus:ring-blue-500 focus:border-blue-500 sm:text-sm"
                                placeholder="Password (min 8 characters)"
                                value={form.password.clone()}
                                onchange={on_password_change}
                                disabled={form.is_loading}
                            />
                        </div>
                        <div>
                            <label for="confirm-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300">{"Confirm New Password"}</label>
                            <input
                                id="confirm-password"
                                name="confirm-password"
                                type="password"
                                required=true
                                class="mt-1 appearance-none relative block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 placeholder-gray-500 dark:placeholder-gray-400 text-gray-900 dark:text-white bg-white dark:bg-gray-700 rounded-md focus:outline-none focus:ring-blue-500 focus:border-blue-500 sm:text-sm"
                                placeholder="Confirm new password"
                                value={form.confirm_password.clone()}
                                onchange={on_confirm_password_change}
                                disabled={form.is_loading}
                            />
                        </div>
                    </div>

                    if let Some(error) = &form.error {
                        <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded">
                            {error}
                        </div>
                    }

                    if let Some(message) = &form.message {
                        <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 text-green-600 dark:text-green-400 px-4 py-3 rounded">
                            {message}
                        </div>
                    }

                    <div>
                        <button
                            type="submit"
                            class="group relative w-full flex justify-center py-2 px-4 border border-transparent text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
                            disabled={form.is_loading}
                        >
                            if form.is_loading {
                                <span class="flex items-center">
                                    <svg class="animate-spin -ml-1 mr-3 h-5 w-5 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                    </svg>
                                    {"Resetting password..."}
                                </span>
                            } else {
                                {"Reset password"}
                            }
                        </button>
                    </div>
                </form>

                <div class="text-center">
                    <Link<Route> to={Route::Login} classes="text-sm text-blue-600 hover:text-blue-500 dark:text-blue-400">
                        {"Back to sign in"}
                    </Link<Route>>
                </div>
            </div>
        </main>
    }
}
