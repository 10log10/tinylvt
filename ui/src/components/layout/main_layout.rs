use crate::{
    State,
    components::layout::{Footer, Header},
    hooks::use_system_theme,
};
use yew::prelude::*;
use yewdux::prelude::*;

#[derive(Properties, PartialEq)]
pub struct MainLayoutProps {
    pub children: Children,
}

#[function_component]
pub fn MainLayout(props: &MainLayoutProps) -> Html {
    let (state, _dispatch) = use_store::<State>();

    // Track system theme preference
    use_system_theme();

    let dark_class = if state.is_dark_mode() { "dark" } else { "" };

    html! {
        <div class={classes!(dark_class)}>
            <div class="min-h-screen bg-white dark:bg-neutral-900 text-neutral-900 dark:text-neutral-100 transition-colors flex flex-col">
                <Header />
                <main class="w-full max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 flex-grow">
                    {for props.children.iter()}
                </main>
                <Footer />
            </div>
        </div>
    }
}
