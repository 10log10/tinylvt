//! # Hook State Management Pattern
//!
//! All hooks in this module follow a consistent three-field pattern for
//! state management:
//!
//! ## FetchState Type
//!
//! The `FetchState<T>` enum explicitly separates network fetch state from
//! data nullability:
//!
//! - `FetchState::NotFetched` - No fetch attempt has been made yet
//! - `FetchState::Fetched(T)` - Data has been fetched (T may be Option<V>)
//!
//! This makes it clear when `None` means "not fetched yet" vs "fetched but
//! the API returned None". For example, `FetchState<Option<f64>>` can be:
//! - `NotFetched` - Haven't called the API yet
//! - `Fetched(None)` - API returned None (e.g., no eligibility for round 0)
//! - `Fetched(Some(0.5))` - API returned Some(0.5)
//!
//! ## Fields
//!
//! - `data: Option<T>` or `FetchState<T>` - The fetched/managed data
//! - `error: Option<String>` - Error from most recent operation
//! - `is_loading: bool` - Whether any operation is in progress
//!
//! ## Helper Methods
//!
//! All hook return types provide this helper method:
//!
//! - `is_initial_loading() -> bool` - Returns true when loading with no data
//!   or error (initial page load that should block the UI)
//!
//! ## State Combinations
//!
//! ### `data: None, error: None, is_loading: true`
//! **Initial loading state** - `is_initial_loading() == true`
//! - Show: Full-page loading spinner or skeleton
//! - Action: Wait for data or error
//! - Example: `if hook.is_initial_loading() { return <LoadingSpinner /> }`
//!
//! ### `data: Some(T), error: None, is_loading: false`
//! **Successfully loaded**
//! - Show: Data normally
//! - Action: None
//!
//! ### `data: Some(T), error: None, is_loading: true`
//! **Refetching/updating with existing data**
//! - Show: Data with subtle loading indicator (e.g., spinner in corner)
//! - Action: Keep UI interactive but may want to disable mutation buttons
//! - Example: `{hook.is_loading && hook.data.is_some() && <InlineSpinner />}`
//!
//! ### `data: Some(T), error: Some(e), is_loading: false`
//! **Operation failed but have stale data.**
//! - Show: Data + error banner (e.g., "Failed to refresh", "Failed to
//!   update")
//! - Action: Allow user to retry or dismiss error
//!
//! ### `data: None, error: Some(e), is_loading: false`
//! **Initial fetch failed completely.**
//! - Show: Error message, no data available
//! - Action: Show retry button or link to go back
//!
//! ### `data: None, error: None, is_loading: false`
//! **Should not occur in practice.**
//! - This state should be unreachable if hooks are implemented correctly
//! - If encountered, treat as loading or error state

/// Represents the fetch state of data, separating network state from data
/// nullability
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FetchState<T> {
    /// No fetch attempt has been made yet
    NotFetched,
    /// Data has been fetched (T may be Option<V> for nullable data)
    Fetched(T),
}

impl<T> FetchState<T> {
    /// Returns true if data has been fetched (regardless of the data's value)
    pub fn is_fetched(&self) -> bool {
        matches!(self, FetchState::Fetched(_))
    }

    /// Returns a reference to the fetched data, or None if not fetched
    pub fn as_ref(&self) -> Option<&T> {
        match self {
            FetchState::Fetched(data) => Some(data),
            FetchState::NotFetched => None,
        }
    }

    /// Maps a FetchState<T> to FetchState<U> by applying a function to the
    /// fetched data
    pub fn map<U, F>(self, f: F) -> FetchState<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            FetchState::Fetched(data) => FetchState::Fetched(f(data)),
            FetchState::NotFetched => FetchState::NotFetched,
        }
    }
}

impl<T> Default for FetchState<T> {
    fn default() -> Self {
        FetchState::NotFetched
    }
}

pub mod use_auction_detail;
pub mod use_auction_round_results;
pub mod use_auction_rounds;
pub mod use_auction_user_bids;
pub mod use_auctions;
pub mod use_authentication;
pub mod use_communities;
pub mod use_current_round;
pub mod use_exponential_refetch;
pub mod use_issued_invites;
pub mod use_logout;
pub mod use_members;
pub mod use_proxy_bidding_settings;
pub mod use_round_prices;
pub mod use_site;
pub mod use_sites;
pub mod use_spaces;
pub mod use_system_theme;
pub mod use_user_bids;
pub mod use_user_eligibility;
pub mod use_user_space_values;

pub use use_auction_detail::use_auction_detail;
pub use use_auction_round_results::use_auction_round_results;
pub use use_auction_rounds::use_auction_rounds;
pub use use_auction_user_bids::use_auction_user_bids;
pub use use_auctions::use_auctions;
pub use use_authentication::use_authentication;
pub use use_communities::use_communities;
pub use use_current_round::use_current_round;
pub use use_exponential_refetch::use_exponential_refetch;
pub use use_issued_invites::use_issued_invites;
pub use use_logout::use_logout;
pub use use_members::use_members;
pub use use_proxy_bidding_settings::use_proxy_bidding_settings;
pub use use_round_prices::use_round_prices;
pub use use_site::use_site;
pub use use_sites::use_sites;
pub use use_spaces::use_spaces;
pub use use_system_theme::use_system_theme;
pub use use_user_bids::use_user_bids;
pub use use_user_eligibility::use_user_eligibility;
pub use use_user_space_values::use_user_space_values;
