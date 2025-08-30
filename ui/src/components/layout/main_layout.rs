use yew::prelude::*;
use yewdux::prelude::*;
use crate::{State, components::layout::Header};

#[derive(Properties, PartialEq)]
pub struct MainLayoutProps {
    pub children: Children,
}

#[function_component]
pub fn MainLayout(props: &MainLayoutProps) -> Html {
    let (state, _) = use_store::<State>();
    let dark_class = if state.dark_mode { "dark" } else { "" };
    
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