use crate::Route;
use yew::prelude::*;
use yew_router::prelude::*;

/// Returns a callback that navigates to a route and scrolls to top.
/// Use this instead of `navigator.push()` for most navigation.
#[hook]
pub fn use_push_route() -> Callback<Route> {
    let navigator = use_navigator().unwrap();
    Callback::from(move |route: Route| {
        navigator.push(&route);
        if let Some(window) = web_sys::window() {
            window.scroll_to_with_x_and_y(0.0, 0.0);
        }
    })
}
