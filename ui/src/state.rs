use payloads::{CommunityId, SiteId, SpaceId, responses};
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
    // === Core App State (managed by various components) ===
    pub error_message: Option<String>, // Global error handling
    pub theme_mode: ThemeMode,         // Theme components
    pub system_prefers_dark: bool,     // System theme detection

    // === Authentication (managed by use_authentication) ===
    pub auth_state: AuthState,

    // === Communities (managed by use_communities) ===
    pub communities: Option<Vec<responses::CommunityWithRole>>,

    // === Sites (canonical store - managed by use_sites + use_site) ===
    pub individual_sites: HashMap<SiteId, responses::Site>, // Single source of truth
    pub sites_by_community: HashMap<CommunityId, Vec<SiteId>>, // Community index

    // === Spaces (canonical store - managed by use_spaces) ===
    pub individual_spaces: HashMap<SpaceId, responses::Space>, // Single source of truth
    pub spaces_by_site: HashMap<SiteId, Vec<SpaceId>>,         // Site index

    // === Members (managed by use_members) ===
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

    pub fn get_community_by_id(
        &self,
        community_id: CommunityId,
    ) -> Option<&responses::CommunityWithRole> {
        self.communities
            .as_ref()?
            .iter()
            .find(|c| c.id == community_id)
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
        self.sites_by_community.contains_key(&community_id)
    }

    pub fn get_sites_for_community(
        &self,
        community_id: CommunityId,
    ) -> Option<Vec<&responses::Site>> {
        self.sites_by_community.get(&community_id).map(|site_ids| {
            site_ids
                .iter()
                .filter_map(|site_id| self.individual_sites.get(site_id))
                .collect()
        })
    }

    pub fn set_sites_for_community(
        &mut self,
        community_id: CommunityId,
        sites: Vec<responses::Site>,
    ) {
        // Extract site IDs for the community index
        let site_ids: Vec<SiteId> =
            sites.iter().map(|site| site.site_id).collect();

        // Store individual sites in the canonical store
        for site in sites {
            self.individual_sites.insert(site.site_id, site);
        }

        // Update the community index
        self.sites_by_community.insert(community_id, site_ids);
    }

    pub fn clear_sites_for_community(&mut self) {
        self.sites_by_community.clear();
        // Note: We don't clear individual_sites here as they might be used by use_site
        // Individual sites will be cleared on logout
    }

    pub fn has_site_loaded(&self, site_id: SiteId) -> bool {
        self.individual_sites.contains_key(&site_id)
    }

    pub fn get_site(&self, site_id: SiteId) -> Option<&responses::Site> {
        self.individual_sites.get(&site_id)
    }

    pub fn set_site(&mut self, site_id: SiteId, site: responses::Site) {
        self.individual_sites.insert(site_id, site);
    }

    pub fn clear_individual_sites(&mut self) {
        self.individual_sites.clear();
    }

    pub fn has_spaces_loaded_for_site(&self, site_id: SiteId) -> bool {
        self.spaces_by_site.contains_key(&site_id)
    }

    pub fn get_spaces_for_site(
        &self,
        site_id: SiteId,
    ) -> Option<Vec<&responses::Space>> {
        self.spaces_by_site.get(&site_id).map(|space_ids| {
            space_ids
                .iter()
                .filter_map(|space_id| self.individual_spaces.get(space_id))
                .collect()
        })
    }

    pub fn set_spaces_for_site(
        &mut self,
        site_id: SiteId,
        spaces: Vec<responses::Space>,
    ) {
        // Extract space IDs for the site index
        let space_ids: Vec<SpaceId> =
            spaces.iter().map(|space| space.space_id).collect();

        // Store individual spaces in the canonical store
        for space in spaces {
            self.individual_spaces.insert(space.space_id, space);
        }

        // Update the site index
        self.spaces_by_site.insert(site_id, space_ids);
    }

    pub fn clear_spaces_for_site(&mut self) {
        self.spaces_by_site.clear();
        // Note: We don't clear individual_spaces here as they might be used by other components
        // Individual spaces will be cleared on logout
    }

    pub fn has_space_loaded(&self, space_id: SpaceId) -> bool {
        self.individual_spaces.contains_key(&space_id)
    }

    pub fn get_space(&self, space_id: SpaceId) -> Option<&responses::Space> {
        self.individual_spaces.get(&space_id)
    }

    pub fn set_space(&mut self, space_id: SpaceId, space: responses::Space) {
        self.individual_spaces.insert(space_id, space);
    }

    pub fn clear_individual_spaces(&mut self) {
        self.individual_spaces.clear();
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
        self.clear_sites_for_community();
        self.clear_individual_sites();
        self.clear_spaces_for_site();
        self.clear_individual_spaces();
        self.clear_members();
        // Future: clear other user-specific state here
    }
}
