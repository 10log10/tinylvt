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
                    email,
                    username,
                    password,
                };

                match client.create_account(&account_details).await {
                    Ok(()) => {
                        // Registration successful, redirect to login
                        navigator.push(&Route::Login);
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

            yew::platform::spawn_local(async move {
                // Set loading state
                {
                    let mut new_form = form_data;
                    new_form.is_loading = true;
                    new_form.error = None;
                    new_form.message = None;
                    form.set(new_form);
                }

                // Simulate API call (not implemented in backend yet)
                yew::platform::time::sleep(std::time::Duration::from_secs(2))
                    .await;

                let mut new_form = (*form).clone();
                new_form.is_loading = false;
                new_form.message = Some("If an account with that email exists, we've sent you a password reset link.".to_string());
                form.set(new_form);
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
