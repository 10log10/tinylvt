use crate::{AuthState, Route, State};
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

#[hook]
pub fn use_logout() -> Callback<MouseEvent> {
    let (_, dispatch) = use_store::<State>();
    let navigator = use_navigator().unwrap();

    Callback::from(move |_| {
        let dispatch = dispatch.clone();
        let navigator = navigator.clone();

        yew::platform::spawn_local(async move {
            let api_client = crate::get_api_client();
            let _ = api_client.logout().await;

            dispatch.reduce_mut(|state| {
                state.auth_state = AuthState::LoggedOut;
            });

            navigator.push(&Route::Login);
        });
    })
}
