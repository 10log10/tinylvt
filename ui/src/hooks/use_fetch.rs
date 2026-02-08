use std::future::Future;
use std::rc::Rc;
use yew::prelude::*;

use super::FetchState;

/// Generic fetch hook return type
pub struct FetchHookReturn<T> {
    pub data: FetchState<T>,
    pub is_loading: bool,
    pub error: Option<String>,
    pub refetch: Callback<()>,
}

impl<T: Clone> FetchHookReturn<T> {
    /// Returns true if this is the initial load (data not yet fetched,
    /// currently loading, and no error).
    pub fn is_initial_loading(&self) -> bool {
        self.is_loading && !self.data.is_fetched() && self.error.is_none()
    }

    /// Render based on fetch state with contextual loading/error messages.
    ///
    /// This handles the common pattern of:
    /// - No data + loading: Show spinner with "Loading {context}..."
    /// - No data + error: Show error with "Error loading {context}: ..."
    /// - Has data: Call render function with (data, is_loading, error)
    ///
    /// The render function receives:
    /// - `data`: The fetched data
    /// - `is_loading`: True if a refetch is in progress
    /// - `error`: Error from a failed refetch (data from previous fetch
    ///   still shown)
    ///
    /// # Arguments
    ///
    /// * `context` - Contextual string like "auction" or "user profile"
    /// * `render_fn` - Function to render when data is available
    ///
    /// # Example
    ///
    /// ```rust
    /// auction_hook.render(
    ///     "auction",
    ///     |auction, is_loading, error| html! {
    ///         <div>
    ///             {if is_loading {
    ///                 html! { <span>{"Refreshing..."}</span> }
    ///             } else {
    ///                 html! {}
    ///             }}
    ///             {if let Some(err) = error {
    ///                 html! { <div class="error">{err}</div> }
    ///             } else {
    ///                 html! {}
    ///             }}
    ///             <AuctionDetails auction={auction.clone()} />
    ///         </div>
    ///     }
    /// )
    /// ```
    pub fn render<F>(&self, context: &str, render_fn: F) -> Html
    where
        F: Fn(&T, bool, Option<&String>) -> Html,
    {
        match self.data.as_ref() {
            None => {
                // No data case
                if self.is_loading {
                    html! {
                        <div class="text-center py-12">
                            <p class="text-neutral-600 dark:text-neutral-400">
                                {format!("Loading {}...", context)}
                            </p>
                        </div>
                    }
                } else if let Some(error) = &self.error {
                    html! {
                        <div class="p-4 rounded-md bg-red-50 \
                                   dark:bg-red-900/20 border \
                                   border-red-200 dark:border-red-800">
                            <p class="text-sm text-red-700 \
                                      dark:text-red-400">
                                {format!("Error loading {}: {}", context, error)}
                            </p>
                        </div>
                    }
                } else {
                    // Shouldn't happen: no data, not loading, no error
                    html! {
                        <div class="text-center py-12">
                            <p class="text-neutral-600 dark:text-neutral-400">
                                {format!("No {} found", context)}
                            </p>
                        </div>
                    }
                }
            }
            Some(data) => {
                // Has data - render with loading/error state for refetches
                render_fn(data, self.is_loading, self.error.as_ref())
            }
        }
    }
}

/// Generic fetch hook composer.
///
/// Automatically fetches on mount and provides refetch capability.
/// The fetch function captures dependencies from the closure, and the
/// deps parameter is used only for dependency tracking in use_callback
/// and use_effect_with.
///
/// # Example
///
/// ```rust
/// #[hook]
/// pub fn use_user_data(user_id: UserId) -> FetchHookReturn<UserData> {
///     use_fetch(
///         user_id,
///         || async move {
///             let api_client = get_api_client();
///             api_client
///                 .get_user_data(user_id)
///                 .await
///                 .map_err(|e| e.to_string())
///         },
///     )
/// }
/// ```
#[hook]
pub fn use_fetch<T, D, F, Fut>(deps: D, fetch_fn: F) -> FetchHookReturn<T>
where
    T: Clone + 'static,
    D: PartialEq + Clone + 'static,
    F: Fn() -> Fut + 'static,
    Fut: Future<Output = Result<T, String>> + 'static,
{
    let data = use_state(|| FetchState::NotFetched);
    let error = use_state(|| None::<String>);
    let is_loading = use_state(|| false);

    let refetch = {
        let data = data.clone();
        let error = error.clone();
        let is_loading = is_loading.clone();
        let fetch_fn = Rc::new(fetch_fn);

        use_callback(deps.clone(), move |_, _| {
            let data = data.clone();
            let error = error.clone();
            let is_loading = is_loading.clone();
            let fetch_fn = fetch_fn.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                match fetch_fn().await {
                    Ok(result) => {
                        data.set(FetchState::Fetched(result));
                        error.set(None);
                    }
                    Err(e) => {
                        error.set(Some(e));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    // Auto-fetch on mount and when deps change
    {
        let refetch = refetch.clone();
        let is_loading_clone = is_loading.clone();

        use_effect_with(deps, move |_| {
            if !*is_loading_clone {
                refetch.emit(());
            }
        });
    }

    FetchHookReturn {
        data: (*data).clone(),
        is_loading: *is_loading,
        error: (*error).clone(),
        refetch: Callback::from(move |_| refetch.emit(())),
    }
}

/// Generic fetch hook with global state caching support.
///
/// This hook is similar to `use_fetch` but designed for hooks that cache
/// data in Yewdux global state. It takes three closures:
///
/// 1. `get_cached`: Retrieves cached data from global state
/// 2. `should_fetch`: Determines if a fetch is needed (checks auth + cache
///    status)
/// 3. `fetch_and_cache`: Performs the API call and updates global state
///
/// The hook automatically fetches on mount if `should_fetch` returns true,
/// and returns cached data via FetchState to distinguish between "not
/// fetched" and "fetched but empty".
///
/// # Example
///
/// ```rust
/// #[hook]
/// pub fn use_site(site_id: SiteId) -> FetchHookReturn<responses::Site> {
///     let (state, dispatch) = use_store::<State>();
///
///     use_fetch_with_cache(
///         site_id,
///         move || state.get_site(site_id).cloned(),
///         move || !state.has_site_loaded(site_id),
///         move || async move {
///             let api_client = get_api_client();
///             let site = api_client.get_site(&site_id).await
///                 .map_err(|e| e.to_string())?;
///             dispatch.reduce_mut(|s| s.set_site(site_id, site.clone()));
///             Ok(site)
///         }
///     )
/// }
/// ```
#[hook]
pub fn use_fetch_with_cache<T, D, GetCached, ShouldFetch, FetchAndCache, Fut>(
    deps: D,
    get_cached: GetCached,
    should_fetch: ShouldFetch,
    fetch_and_cache: FetchAndCache,
) -> FetchHookReturn<T>
where
    T: Clone + 'static,
    D: PartialEq + Clone + 'static,
    GetCached: Fn() -> Option<T> + 'static,
    ShouldFetch: Fn() -> bool + 'static,
    FetchAndCache: Fn() -> Fut + 'static,
    Fut: Future<Output = Result<T, String>> + 'static,
{
    let error = use_state(|| None::<String>);
    let is_loading = use_state(|| false);

    let refetch = {
        let error = error.clone();
        let is_loading = is_loading.clone();
        let fetch_and_cache = Rc::new(fetch_and_cache);

        use_callback(deps.clone(), move |_, _| {
            let error = error.clone();
            let is_loading = is_loading.clone();
            let fetch_and_cache = fetch_and_cache.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                match fetch_and_cache().await {
                    Ok(_) => {
                        error.set(None);
                    }
                    Err(e) => {
                        error.set(Some(e));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    // Auto-fetch on mount if should_fetch returns true
    {
        let refetch = refetch.clone();
        let is_loading_clone = is_loading.clone();
        let should_fetch = Rc::new(should_fetch);

        use_effect_with(deps.clone(), move |_| {
            if should_fetch() && !*is_loading_clone {
                refetch.emit(());
            }
        });
    }

    // Get cached data and convert to FetchState
    let cached_data = get_cached();
    let data = if let Some(cached) = cached_data {
        FetchState::Fetched(cached)
    } else if *is_loading || error.is_some() {
        // If we're loading or have an error, we haven't successfully fetched
        FetchState::NotFetched
    } else {
        // Not loading, no error, but also no cached data
        // This happens when should_fetch returns false (not authenticated)
        FetchState::NotFetched
    };

    // Calculate effective is_loading: loading OR initial state with no data
    let effective_is_loading =
        *is_loading || (!data.is_fetched() && error.is_none());

    FetchHookReturn {
        data,
        is_loading: effective_is_loading,
        error: (*error).clone(),
        refetch: Callback::from(move |_| refetch.emit(())),
    }
}
