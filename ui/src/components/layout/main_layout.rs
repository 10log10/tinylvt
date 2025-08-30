use crate::{State, components::layout::Header};
use yew::prelude::*;
use yewdux::prelude::*;

// Detect system color scheme preference
fn get_system_theme_preference() -> bool {
    web_sys::window()
        .and_then(|w| w.match_media("(prefers-color-scheme: dark)").ok())
        .and_then(|mql| mql)
        .map(|mql| mql.matches())
        .unwrap_or(false)
}

#[derive(Properties, PartialEq)]
pub struct MainLayoutProps {
    pub children: Children,
}

#[function_component]
pub fn MainLayout(props: &MainLayoutProps) -> Html {
    let (state, dispatch) = use_store::<State>();

    // Initialize system preference and listen for changes
    use_effect_with((), {
        let dispatch = dispatch.clone();
        move |_| {
            use wasm_bindgen::prelude::*;
            use web_sys::{MediaQueryList, MediaQueryListEvent};

            // Set initial system preference
            let system_prefers_dark = get_system_theme_preference();
            dispatch.reduce_mut(move |state| {
                state.system_prefers_dark = system_prefers_dark;
            });

            // Listen for system preference changes
            let window = web_sys::window().unwrap();
            let media_query: MediaQueryList = window
                .match_media("(prefers-color-scheme: dark)")
                .unwrap()
                .unwrap();

            let dispatch_clone = dispatch.clone();
            let closure =
                Closure::wrap(Box::new(move |event: MediaQueryListEvent| {
                    let prefers_dark = event.matches();
                    dispatch_clone.reduce_mut(move |state| {
                        state.system_prefers_dark = prefers_dark;
                    });
                })
                    as Box<dyn FnMut(MediaQueryListEvent)>);

            // Use addEventListener instead of the deprecated addListener
            media_query
                .add_event_listener_with_callback(
                    "change",
                    closure.as_ref().unchecked_ref(),
                )
                .unwrap();

            // Return cleanup function
            // Note: Rust retains ownership of the closure, JS only has a pointer to it
            move || {
                let _ = media_query.remove_event_listener_with_callback(
                    "change",
                    closure.as_ref().unchecked_ref(),
                );
                drop(closure);
            }
        }
    });

    let dark_class = if state.is_dark_mode() { "dark" } else { "" };

    html! {
        <div class={classes!(dark_class)}>
            <div class="min-h-screen bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 transition-colors">
                <Header />
                <main class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
                    {for props.children.iter()}
                </main>
            </div>
        </div>
    }
}
