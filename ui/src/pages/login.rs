use payloads::responses;
use yew::prelude::*;
use yewdux::prelude::*;

use crate::Route;
use crate::components::{AuthForm, login_form::AuthMode};
use crate::hooks::{use_push_route, use_title};
use crate::state::State;

#[function_component]
pub fn LoginPage() -> Html {
    use_title("Log In - TinyLVT");
    let push_route = use_push_route();
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
        let push_route = push_route.clone();
        let is_authenticated = state.is_authenticated();

        use_effect_with(is_authenticated, move |is_auth| {
            if *is_auth {
                push_route.emit(Route::Home);
            }
        });
    }

    let on_auth_success = {
        let push_route = push_route.clone();

        Callback::from(move |_profile: responses::UserProfile| {
            // Both login and account creation now navigate to home
            // since we auto-login users after successful account creation
            push_route.emit(Route::Home);
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
