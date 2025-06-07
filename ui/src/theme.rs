use web_sys::window;
use yew::prelude::*;
use yewdux::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
    System,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::System
    }
}

impl Theme {
    pub fn to_string(&self) -> String {
        match self {
            Theme::Light => "light".to_string(),
            Theme::Dark => "dark".to_string(),
            Theme::System => "system".to_string(),
        }
    }

    pub fn from_string(s: &str) -> Self {
        match s {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::System,
        }
    }
}

#[derive(Default, Clone, PartialEq, Store)]
pub struct ThemeState {
    pub theme: Theme,
    pub effective_theme: Theme, // The actual theme being used (resolves System to Light/Dark)
}

// Theme management functions
pub fn get_system_theme() -> Theme {
    let window = window().unwrap();
    if let Ok(Some(media_query)) =
        window.match_media("(prefers-color-scheme: dark)")
    {
        if media_query.matches() {
            return Theme::Dark;
        }
    }
    Theme::Light
}

pub fn get_stored_theme() -> Theme {
    if let Ok(Some(storage)) = window().unwrap().local_storage() {
        if let Ok(Some(theme_str)) = storage.get_item("theme") {
            return Theme::from_string(&theme_str);
        }
    }
    Theme::System
}

pub fn store_theme(theme: &Theme) {
    if let Ok(Some(storage)) = window().unwrap().local_storage() {
        let _ = storage.set_item("theme", &theme.to_string());
    }
}

pub fn apply_theme_to_document(theme: &Theme) {
    let document = window().unwrap().document().unwrap();
    let html = document.document_element().unwrap();

    // Remove existing theme classes
    let _ = html.class_list().remove_1("dark");

    // Apply new theme
    match theme {
        Theme::Dark => {
            let _ = html.class_list().add_1("dark");
        }
        Theme::Light => {
            // Light mode is default, no class needed
        }
        Theme::System => {
            // This should not happen in effective_theme
            unreachable!("System theme should be resolved to Light or Dark");
        }
    }
}

pub fn resolve_effective_theme(theme: &Theme) -> Theme {
    match theme {
        Theme::System => get_system_theme(),
        theme => *theme,
    }
}

#[hook]
pub fn use_theme() -> (Theme, Theme, Callback<Theme>) {
    let (state, dispatch) = use_store::<ThemeState>();

    let setter = use_callback(
        dispatch.clone(),
        move |new_theme: Theme, dispatch: &Dispatch<ThemeState>| {
            let effective = resolve_effective_theme(&new_theme);

            // Store preference (unless it's system default)
            if new_theme != Theme::System {
                store_theme(&new_theme);
            } else if let Ok(Some(storage)) = window().unwrap().local_storage()
            {
                let _ = storage.remove_item("theme");
            }

            // Apply to document
            apply_theme_to_document(&effective);

            // Update state
            dispatch.reduce_mut(|state| {
                state.theme = new_theme;
                state.effective_theme = effective;
            });
        },
    );

    (state.theme, state.effective_theme, setter)
}

// Theme toggle component
#[function_component]
pub fn ThemeToggle() -> Html {
    let (_, effective_theme, set_theme) = use_theme();

    // Initialize theme on first render
    {
        let set_theme_init = set_theme.clone();
        use_effect_with((), move |_| {
            let stored_theme = get_stored_theme();
            let effective = resolve_effective_theme(&stored_theme);
            apply_theme_to_document(&effective);
            set_theme_init.emit(stored_theme);
            || ()
        });
    }

    let toggle_theme = use_callback(
        (effective_theme, set_theme.clone()),
        move |_: MouseEvent, (effective_theme, set_theme)| {
            let new_theme = match *effective_theme {
                Theme::Light => Theme::Dark,
                Theme::Dark => Theme::Light,
                Theme::System => Theme::Dark, // Shouldn't happen but fallback to dark
            };
            set_theme.emit(new_theme);
        },
    );

    let icon = match effective_theme {
        Theme::Light => "🌙",  // Moon for dark mode toggle
        Theme::Dark => "☀️",   // Sun for light mode toggle
        Theme::System => "🌙", // Fallback
    };

    let title = match effective_theme {
        Theme::Light => "Switch to dark mode",
        Theme::Dark => "Switch to light mode",
        Theme::System => "Toggle theme",
    };

    html! {
        <button
            class="p-2 rounded-lg hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
            onclick={toggle_theme}
            title={title}
            aria-label={title}
        >
            <span class="text-xl">{icon}</span>
        </button>
    }
}
