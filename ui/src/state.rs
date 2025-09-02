use yewdux::prelude::*;

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
}

impl State {
    pub fn is_dark_mode(&self) -> bool {
        match self.theme_mode {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::System => self.system_prefers_dark,
        }
    }
}
