use payloads::{CommunityId, responses};
use std::collections::HashMap;
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
    pub sites: HashMap<CommunityId, Vec<responses::Site>>,
    pub members: HashMap<CommunityId, Vec<responses::CommunityMember>>,
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

    pub fn has_sites_loaded_for_community(
        &self,
        community_id: CommunityId,
    ) -> bool {
        self.sites.contains_key(&community_id)
    }

    pub fn get_sites_for_community(
        &self,
        community_id: CommunityId,
    ) -> Option<&Vec<responses::Site>> {
        self.sites.get(&community_id)
    }

    pub fn set_sites_for_community(
        &mut self,
        community_id: CommunityId,
        sites: Vec<responses::Site>,
    ) {
        self.sites.insert(community_id, sites);
    }

    pub fn clear_sites(&mut self) {
        self.sites.clear();
    }

    pub fn has_members_loaded_for_community(
        &self,
        community_id: CommunityId,
    ) -> bool {
        self.members.contains_key(&community_id)
    }

    pub fn get_members_for_community(
        &self,
        community_id: CommunityId,
    ) -> Option<&Vec<responses::CommunityMember>> {
        self.members.get(&community_id)
    }

    pub fn set_members_for_community(
        &mut self,
        community_id: CommunityId,
        members: Vec<responses::CommunityMember>,
    ) {
        self.members.insert(community_id, members);
    }

    pub fn clear_members(&mut self) {
        self.members.clear();
    }

    pub fn logout(&mut self) {
        self.auth_state = AuthState::LoggedOut;
        self.clear_communities();
        self.clear_sites();
        self.clear_members();
        // Future: clear other user-specific state here
    }
}
