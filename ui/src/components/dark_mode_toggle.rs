use crate::{State, ThemeMode};
use yew::prelude::*;
use yewdux::prelude::*;

#[function_component]
pub fn DarkModeToggle() -> Html {
    let (state, dispatch) = use_store::<State>();

    let cycle_theme = {
        let dispatch = dispatch.clone();
        Callback::from(move |_| {
            dispatch.reduce_mut(|state| {
                state.theme_mode = match state.theme_mode {
                    ThemeMode::System => ThemeMode::Dark,
                    ThemeMode::Dark => ThemeMode::Light,
                    ThemeMode::Light => ThemeMode::System,
                };
            });
        })
    };

    html! {
        <button
            onclick={cycle_theme}
            class="p-2 rounded-md hover:bg-neutral-100 dark:hover:bg-neutral-800 transition-colors"
            title={match state.theme_mode {
                ThemeMode::System => "Theme: System (click for Dark)",
                ThemeMode::Dark => "Theme: Dark (click for Light)",
                ThemeMode::Light => "Theme: Light (click for System)",
            }}
        >
            {match state.theme_mode {
                ThemeMode::System => html! {
                    // Computer/monitor icon for system preference
                    <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
                        <path fill_rule="evenodd" d="M3 4a1 1 0 011-1h12a1 1 0 011 1v8a1 1 0 01-1 1h-5v1h3a1 1 0 110 2H6a1 1 0 110-2h3v-1H4a1 1 0 01-1-1V4zm1 7V5h12v6H4z" clip_rule="evenodd" />
                    </svg>
                },
                ThemeMode::Light => html! {
                    // Sun icon - shown for light mode
                    <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
                        <path fill_rule="evenodd" d="M10 2a1 1 0 011 1v1a1 1 0 11-2 0V3a1 1 0 011-1zm4 8a4 4 0 11-8 0 4 4 0 018 0zm-.464 4.95l.707.707a1 1 0 001.414-1.414l-.707-.707a1 1 0 00-1.414 1.414zm2.12-10.607a1 1 0 010 1.414l-.706.707a1 1 0 11-1.414-1.414l.707-.707a1 1 0 011.414 0zM17 11a1 1 0 100-2h-1a1 1 0 100 2h1zm-7 4a1 1 0 011 1v1a1 1 0 11-2 0v-1a1 1 0 011-1zM5.05 6.464A1 1 0 106.465 5.05l-.708-.707a1 1 0 00-1.414 1.414l.707.707zm1.414 8.486l-.707.707a1 1 0 01-1.414-1.414l.707-.707a1 1 0 011.414 1.414zM4 11a1 1 0 100-2H3a1 1 0 000 2h1z" clip_rule="evenodd" />
                    </svg>
                },
                ThemeMode::Dark => html! {
                    // Moon icon - shown for dark mode
                    <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
                        <path d="M17.293 13.293A8 8 0 016.707 2.707a8.001 8.001 0 1010.586 10.586z" />
                    </svg>
                },
            }}
        </button>
    }
}
