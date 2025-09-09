use payloads::responses;
use yewdux::prelude::*;

#[derive(Clone, PartialEq, Default)]
pub enum AuthState {
    #[default]
    Unknown,
    LoggedOut,
    LoggedIn(responses::UserProfile),
}

#[derive(Clone, PartialEq)]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

impl Default for ThemeMode {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Default, Clone, PartialEq, Store)]
pub struct State {
    pub error_message: Option<String>,
    pub theme_mode: ThemeMode,
    pub system_prefers_dark: bool,
    pub auth_state: AuthState,
}

impl State {
    pub fn is_dark_mode(&self) -> bool {
        match self.theme_mode {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::System => self.system_prefers_dark,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        matches!(self.auth_state, AuthState::LoggedIn(_))
    }
}
