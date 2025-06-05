use payloads::APIClient;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

#[derive(Default, Clone, PartialEq, Store)]
struct State {
    pub error_message: Option<String>,
}

#[function_component]
pub fn App() -> Html {
    // login::relogin();
    html! {
        <BrowserRouter>
            // <Header />
            // <ErrorMessage />
            <Switch<Route> render={switch} />
        </BrowserRouter>
    }
}

#[function_component]
fn ErrorMessage() -> Html {
    let err = use_selector(|s: &State| s.error_message.clone());
    if err.is_some() {
        yew::platform::spawn_local(async {
            yew::platform::time::sleep(std::time::Duration::from_secs(5)).await;
            let dispatch = Dispatch::<State>::global();
            dispatch.reduce_mut(|s| s.error_message = None);
        })
    }
    html! {
        <>
            if let Some(msg) = &*err {
                <div class="bg-red-800 text-center">
                    {msg}
                </div>
            }
        </>
    }
}
fn set_error_message(err: impl Into<anyhow::Error>) {
    let dispatch = Dispatch::<State>::global();
    dispatch.reduce_mut(move |s| {
        s.error_message = Some(format!("{:#}", err.into()))
    });
}

#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[at("/")]
    Home,
    // #[at("/about")]
    // About,
    // #[at("/login")]
    // Login,
    // #[at("/profile")]
    // Profile,
    // #[at("/bids")]
    // Bids,
    #[not_found]
    #[at("/404")]
    NotFound,
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <p> {"Hello world"} </p> },
        // Route::About => html! { <about::About /> },
        // Route::Login => html! { <login::Login /> },
        // Route::Profile => html! { <profile::Profile /> },
        // Route::Bids => html! { <bids::Bids /> },
        Route::NotFound => html! { <h1>{ "404" }</h1> },
    }
}
