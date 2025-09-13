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
    pub communities: Option<Vec<responses::CommunityWithRole>>,
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

    pub fn has_communities_loaded(&self) -> bool {
        self.communities.is_some()
    }

    pub fn get_communities(
        &self,
    ) -> &Option<Vec<responses::CommunityWithRole>> {
        &self.communities
    }

    pub fn clear_communities(&mut self) {
        self.communities = None;
    }

    pub fn set_communities(
        &mut self,
        communities: Vec<responses::CommunityWithRole>,
    ) {
        self.communities = Some(communities);
    }

    pub fn logout(&mut self) {
        self.auth_state = AuthState::LoggedOut;
        self.clear_communities();
        // Future: clear other user-specific state here
    }
}
