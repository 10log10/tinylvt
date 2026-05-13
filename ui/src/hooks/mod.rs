//! # Hook State Management Pattern
//!
//! All hooks in this module follow a consistent three-field pattern for
//! state management:
//!
//! ## FetchData Type
//!
//! The `FetchData<T>` enum explicitly separates network fetch state from
//! data nullability:
//!
//! - `FetchData::NotFetched` - No fetch attempt has been made yet
//! - `FetchData::Fetched(T)` - Data has been fetched (T may be Option<V>)
//!
//! This makes it clear when `None` means "not fetched yet" vs "fetched but
//! the API returned None". For example, `FetchData<Option<f64>>` can be:
//! - `NotFetched` - Haven't called the API yet
//! - `Fetched(None)` - API returned None (e.g., no eligibility for round 0)
//! - `Fetched(Some(0.5))` - API returned Some(0.5)
//!
//! ## Fields
//!
//! - `data: FetchData<T>` - The fetched/managed data
//! - `errors: Vec<String>` - Errors from most recent operation. Single-hook
//!   code emits 0 or 1 elements; plurality emerges through `Fetch::zip` (a
//!   derived fetch carries one error per zipped input).
//! - `is_loading: bool` - Whether any operation is in progress
//!
//! ## Rendering helpers
//!
//! Use the helpers in `use_fetch` to handle the standard "loading / error /
//! has data" pattern uniformly. They centralize the state-machine logic —
//! most importantly, "data is shown unconditionally once fetched, even
//! during refetch."
//!
//! - `inner.render(on_value, on_loading, on_error)` — method on `Fetch`. The
//!   bare primitive that drives the state machine; the only place that touches
//!   the internal fields. Wrapper helpers below are built on top.
//! - `render_section(&inner, "context", on_value)` — free function;
//!   page/section UI (centered loading text, error banner cards).
//! - `render_cell(&inner, on_value)` — free function; per-cell UI (skeleton
//!   block, compact error glyph with tooltip).
//!
//! ## State Combinations
//!
//! ### `data: NotFetched, errors: empty, is_loading: true`
//! **Initial loading state.** `render_*` shows the loading placeholder.
//!
//! ### `data: Fetched(_), errors: empty, is_loading: false`
//! **Successfully loaded.** `render_*` shows the value normally.
//!
//! ### `data: Fetched(_), errors: empty, is_loading: true`
//! **Refetching/updating with existing data.** The data stays visible;
//! `on_value` receives `is_loading = true` so it can render a subtle
//! refresh indicator.
//!
//! ### `data: Fetched(_), errors: non-empty, is_loading: false`
//! **Operation failed but have stale data.** The data stays visible;
//! `on_value` receives the errors so it can render an inline banner
//! ("Failed to refresh") alongside.
//!
//! ### `data: NotFetched, errors: non-empty, is_loading: false`
//! **Initial fetch failed completely.** `render_*` invokes `on_error`.
//!
//! ### `data: NotFetched, errors: empty, is_loading: false`
//! **Should not occur in practice.** `render_*` falls back to the loading
//! branch if encountered.

/// Represents the fetch state of data, separating network state from data
/// nullability
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FetchData<T> {
    /// No fetch attempt has been made yet
    #[default]
    NotFetched,
    /// Data has been fetched (T may be Option<V> for nullable data)
    Fetched(T),
}

impl<T> FetchData<T> {
    /// Returns true if data has been fetched (regardless of the data's value)
    pub fn is_fetched(&self) -> bool {
        matches!(self, FetchData::Fetched(_))
    }

    /// Returns a reference to the fetched data, or None if not fetched
    pub fn as_ref(&self) -> Option<&T> {
        match self {
            FetchData::Fetched(data) => Some(data),
            FetchData::NotFetched => None,
        }
    }

    /// Maps a FetchData<T> to FetchData<U> by applying a function to the
    /// fetched data
    pub fn map<U, F>(self, f: F) -> FetchData<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            FetchData::Fetched(data) => FetchData::Fetched(f(data)),
            FetchData::NotFetched => FetchData::NotFetched,
        }
    }

    /// Like `map`, but borrows. Useful for projecting a FetchData into a
    /// derived form without consuming the original.
    pub fn map_ref<U, F>(&self, f: F) -> FetchData<U>
    where
        F: FnOnce(&T) -> U,
    {
        match self {
            FetchData::Fetched(data) => FetchData::Fetched(f(data)),
            FetchData::NotFetched => FetchData::NotFetched,
        }
    }

    /// Combine two FetchDatas into one. Both inputs must be Fetched for
    /// the result to be Fetched; otherwise NotFetched. Data combinator
    /// only — for combining the full `Fetch<T>` bundle (which carries
    /// is_loading + errors), use `Fetch::zip` instead.
    #[allow(dead_code)]
    pub fn zip<U>(self, other: FetchData<U>) -> FetchData<(T, U)> {
        match (self, other) {
            (FetchData::Fetched(a), FetchData::Fetched(b)) => {
                FetchData::Fetched((a, b))
            }
            _ => FetchData::NotFetched,
        }
    }

    /// Like `zip`, but borrows both inputs.
    #[allow(dead_code)]
    pub fn zip_ref<'a, U>(
        &'a self,
        other: &'a FetchData<U>,
    ) -> FetchData<(&'a T, &'a U)> {
        match (self, other) {
            (FetchData::Fetched(a), FetchData::Fetched(b)) => {
                FetchData::Fetched((a, b))
            }
            _ => FetchData::NotFetched,
        }
    }
}

pub mod auction_subscription;
pub mod use_auction_detail;
pub mod use_auction_round_results;
pub mod use_auction_rounds;
pub mod use_auction_user_bids;
pub mod use_auctions;
pub mod use_authentication;
pub mod use_communities;
pub mod use_community_images;
pub mod use_fetch;
pub mod use_issued_invites;
pub mod use_last_round;
pub mod use_logout;
pub mod use_member_credit_limit_override;
pub mod use_member_currency_info;
pub mod use_member_transactions;
pub mod use_members;
pub mod use_orphaned_accounts;
pub mod use_platform_stats;
pub mod use_proxy_bidding_settings;
pub mod use_push_route;
pub mod use_require_auth;
pub mod use_round_prices;
pub mod use_site;
pub mod use_sites;
pub mod use_spaces;
pub mod use_storage_usage;
pub mod use_subscription_info;
pub mod use_system_theme;
pub mod use_title;
pub mod use_treasury_account;
pub mod use_treasury_transactions;
pub mod use_user_bids;
pub mod use_user_eligibility;
pub mod use_user_space_values;

pub use auction_subscription::{ConnectionStatus, SubscribedEvent};
pub use use_auction_detail::use_auction_detail;
pub use use_auction_round_results::use_auction_round_results;
pub use use_auction_rounds::use_auction_rounds;
pub use use_auction_user_bids::use_auction_user_bids;
pub use use_auctions::use_auctions;
pub use use_authentication::use_authentication;
pub use use_communities::use_communities;
pub use use_community_images::use_community_images;
#[allow(unused_imports)]
pub use use_fetch::render_cell;
pub use use_fetch::{
    Fetch, FetchHookReturn, SubscribedFetchHookReturn, render_section,
    stale_data_banner, use_fetch, use_fetch_with_cache, use_subscribed_fetch,
};
pub use use_issued_invites::use_issued_invites;
pub use use_last_round::use_last_round;
pub use use_logout::use_logout;
pub use use_member_credit_limit_override::use_member_credit_limit_override;
pub use use_member_currency_info::use_member_currency_info;
pub use use_member_transactions::use_member_transactions;
pub use use_members::use_members;
pub use use_orphaned_accounts::use_orphaned_accounts;
pub use use_platform_stats::use_platform_stats;
pub use use_proxy_bidding_settings::{
    ProxyBiddingSettingsHookReturn, use_proxy_bidding_settings,
};
pub use use_push_route::use_push_route;
pub use use_require_auth::{login_form, use_require_auth};
pub use use_round_prices::use_round_prices;
pub use use_site::use_site;
pub use use_sites::use_sites;
pub use use_spaces::use_spaces;
pub use use_storage_usage::use_storage_usage;
pub use use_subscription_info::use_subscription_info;
pub use use_system_theme::use_system_theme;
pub use use_title::use_title;
pub use use_treasury_account::use_treasury_account;
pub use use_treasury_transactions::use_treasury_transactions;
pub use use_user_bids::use_user_bids;
pub use use_user_eligibility::use_user_eligibility;
pub use use_user_space_values::{
    UserSpaceValuesHookReturn, use_user_space_values,
};
