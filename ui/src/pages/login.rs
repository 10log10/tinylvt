use payloads::responses;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

use crate::Route;
use crate::components::{AuthForm, login_form::AuthMode};
use crate::state::State;

#[function_component]
pub fn LoginPage() -> Html {
    let navigator = use_navigator().unwrap();
    let (state, _) = use_store::<State>();

    // Check for signup query parameter synchronously
    let initial_mode = {
        let window = web_sys::window().unwrap();
        let location = window.location();
        let search = location.search().unwrap_or_default();

        if search.contains("signup") {
            AuthMode::CreateAccount
        } else {
            AuthMode::Login
        }
    };

    // Redirect to home if already logged in
    {
        let navigator = navigator.clone();
        let is_authenticated = state.is_authenticated();

        use_effect_with(is_authenticated, move |is_auth| {
            if *is_auth {
                navigator.push(&Route::Home);
            }
        });
    }

    let on_auth_success = {
        let navigator = navigator.clone();

        Callback::from(move |_profile: responses::UserProfile| {
            // Both login and account creation now navigate to home
            // since we auto-login users after successful account creation
            navigator.push(&Route::Home);
        })
    };

    html! {
        <div class="flex items-center justify-center min-h-[60vh]">
            <AuthForm
                initial_mode={initial_mode}
                on_success={on_auth_success}
                login_title="Sign in to TinyLVT"
                login_description="Enter your credentials to continue"
                signup_title="Create your account"
                signup_description="Join TinyLVT to get started"
            />
        </div>
    }
}
