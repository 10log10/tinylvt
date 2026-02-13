use super::docs_sidebar::DocsSidebar;
use crate::Route;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_router::prelude::*;

#[derive(Properties, PartialEq)]
pub struct DocsLayoutProps {
    pub children: Children,
}

#[function_component]
pub fn DocsLayout(props: &DocsLayoutProps) -> Html {
    let route = use_route::<Route>().unwrap_or(Route::Docs);

    let drawer_open = use_state(|| false);

    let toggle_drawer = {
        let drawer_open = drawer_open.clone();
        Callback::from(move |_: MouseEvent| {
            drawer_open.set(!*drawer_open);
        })
    };

    let close_drawer = {
        let drawer_open = drawer_open.clone();
        Callback::from(move |_| {
            drawer_open.set(false);
        })
    };

    html! {
        <div class="flex min-h-0">
            // Desktop sidebar - hidden on mobile
            <aside class="hidden md:block w-64 flex-shrink-0 border-r \
                          border-neutral-200 dark:border-neutral-700 \
                          bg-white dark:bg-neutral-900">
                <DocsSidebar active_route={route.clone()} />
            </aside>

            // Main content area
            <div class="flex-1 min-w-0">
                {for props.children.iter()}
            </div>

            // Mobile floating menu button - shown only on mobile
            <button
                onclick={toggle_drawer.clone()}
                class="md:hidden fixed bottom-4 left-4 z-40 p-3 rounded-full \
                       bg-neutral-900 dark:bg-white text-white dark:text-neutral-900 \
                       shadow-lg hover:bg-neutral-700 dark:hover:bg-neutral-200 \
                       transition-colors"
                aria-label="Toggle documentation menu"
            >
                <MenuIcon />
            </button>

            // Mobile drawer overlay and sidebar
            if *drawer_open {
                <MobileDrawer
                    active_route={route.clone()}
                    on_close={close_drawer}
                />
            }
        </div>
    }
}

/// Hamburger menu icon for the floating button.
#[function_component]
fn MenuIcon() -> Html {
    html! {
        <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M4 6h16M4 12h16M4 18h16"
            />
        </svg>
    }
}

#[derive(Properties, PartialEq)]
struct MobileDrawerProps {
    active_route: Route,
    on_close: Callback<()>,
}

/// Mobile drawer component that slides in from the left.
#[function_component]
fn MobileDrawer(props: &MobileDrawerProps) -> Html {
    let backdrop_ref = use_node_ref();

    let on_backdrop_click = {
        let on_close = props.on_close.clone();
        let backdrop_ref = backdrop_ref.clone();

        Callback::from(move |e: MouseEvent| {
            // Only close if clicking directly on backdrop, not on drawer content
            if let Some(backdrop_element) =
                backdrop_ref.cast::<web_sys::Element>()
                && let Some(target) = e.target()
                && target.dyn_ref::<web_sys::Element>()
                    == Some(&backdrop_element)
            {
                on_close.emit(());
            }
        })
    };

    let on_navigate = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| {
            on_close.emit(());
        })
    };

    html! {
        <div
            ref={backdrop_ref}
            onclick={on_backdrop_click}
            class="md:hidden fixed inset-0 z-50 bg-black bg-opacity-50"
        >
            <aside class="absolute left-0 top-0 bottom-0 w-64 \
                          bg-white dark:bg-neutral-900 shadow-xl \
                          border-r border-neutral-200 dark:border-neutral-700">
                <div class="flex items-center justify-between p-4 \
                            border-b border-neutral-200 dark:border-neutral-700">
                    <span class="font-semibold text-neutral-900 dark:text-white">
                        {"Menu"}
                    </span>
                    <button
                        onclick={props.on_close.reform(|_: MouseEvent| ())}
                        class="p-1 text-neutral-500 hover:text-neutral-700 \
                               dark:text-neutral-400 dark:hover:text-neutral-200"
                        aria-label="Close menu"
                    >
                        <CloseIcon />
                    </button>
                </div>
                <DocsSidebar
                    active_route={props.active_route.clone()}
                    on_navigate={on_navigate}
                />
            </aside>
        </div>
    }
}

/// Close (X) icon for the drawer header.
#[function_component]
fn CloseIcon() -> Html {
    html! {
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M6 18L18 6M6 6l12 12"
            />
        </svg>
    }
}
