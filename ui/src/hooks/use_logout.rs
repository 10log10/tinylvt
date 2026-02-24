use crate::hooks::use_push_route;
use crate::{Route, State};
use yew::prelude::*;
use yewdux::prelude::*;

#[hook]
pub fn use_logout() -> Callback<()> {
    let (_, dispatch) = use_store::<State>();
    let push_route = use_push_route();

    Callback::from(move |()| {
        let dispatch = dispatch.clone();
        let push_route = push_route.clone();

        yew::platform::spawn_local(async move {
            let api_client = crate::get_api_client();
            let _ = api_client.logout().await;

            dispatch.reduce_mut(|state| {
                state.logout();
            });

            push_route.emit(Route::Landing);
        });
    })
}
