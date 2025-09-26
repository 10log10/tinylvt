use payloads::responses;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;
use crate::components::LoginForm;

#[function_component]
pub fn LoginPage() -> Html {
    let navigator = use_navigator().unwrap();

    let on_login_success = {
        let navigator = navigator.clone();

        Callback::from(move |_profile: responses::UserProfile| {
            navigator.push(&Route::Home);
        })
    };

    html! {
        <div class="flex items-center justify-center min-h-[60vh]">
            <LoginForm
                title="Sign in to TinyLVT"
                description="Enter your credentials to continue"
                submit_text="Sign in"
                on_success={on_login_success}
                show_dev_credentials={true}
            />
        </div>
    }
}
