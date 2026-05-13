use std::cell::Cell;
use std::cmp::Ordering;
use std::future::Future;
use std::ops::Deref;
use std::rc::Rc;
use std::time::Duration;

use gloo_timers::callback::Timeout;
use yew::prelude::*;

use super::FetchData;
use payloads::AuctionId;

use super::auction_subscription::{
    ConnectionStatus, SubscribedEvent, registry,
};

/// Speculative-fetch deadline. If SSE hasn't attached by this time after
/// mount, fire a fetch so the page paints quickly. Status stays `Connecting`
/// until the longer deadline below.
const SPECULATIVE_FETCH_AFTER: Duration = Duration::from_secs(1);

/// Failure deadline. If SSE still hasn't attached by this time, the status
/// becomes `Failed` (indicator visible). The speculative fetch at 1s has
/// already populated the page, so no fetch is needed here.
const FAILURE_AFTER: Duration = Duration::from_secs(5);

/// The render-relevant fields of a fetch hook return.
///
/// Render-only components should take `&Fetch<T>` rather than
/// the full `FetchHookReturn<T>` / `SubscribedFetchHookReturn<T>`. That way
/// they don't rerender when only the `refetch` callback or
/// `connection_status` changes — which can happen without the inner state
/// having changed.
///
/// Fields are private. The only ways to read a `Fetch<T>` are:
/// - Pass it to a render helper (`render`, `render_section`, `render_cell`).
/// - Derive a new `Fetch` via `map`, `map_ref`, `map_err`, `zip`, `zip_ref`.
/// - Compare for sort via `cmp_by`.
///
/// This makes the state-machine logic inviolate: there is no way to observe
/// the inner data without going through a helper that handles the
/// loading/error/fetched cases.
///
/// `errors` is a `Vec<String>` rather than `Option<String>` because
/// `Fetch::zip` can produce a derived fetch that carries multiple
/// independent error messages (one per zipped input). Single-hook code
/// emits 0 or 1 elements; plurality only emerges through `zip`.
#[derive(Clone, Debug, PartialEq)]
pub struct Fetch<T: Clone + PartialEq> {
    data: FetchData<T>,
    is_loading: bool,
    errors: Vec<String>,
}

impl<T: Clone + PartialEq> Default for Fetch<T> {
    /// Default is `NotFetched`, not loading, no errors. Useful as a
    /// `#[prop_or_default]` for components that take a `Fetch` whose value
    /// is not always relevant (e.g., a space list rendered in a
    /// before-auction-starts context where there's no activity to compute).
    fn default() -> Self {
        Self {
            data: FetchData::NotFetched,
            is_loading: false,
            errors: Vec::new(),
        }
    }
}

/// Generic fetch hook return type. Derefs to `Fetch<T>` so
/// `data`, `is_loading`, and `error` remain reachable as field access.
#[derive(Clone, PartialEq)]
pub struct FetchHookReturn<T: Clone + PartialEq> {
    pub inner: Fetch<T>,
    pub refetch: Callback<()>,
}

impl<T: Clone + PartialEq> Deref for FetchHookReturn<T> {
    type Target = Fetch<T>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Return type of subscribed fetch hooks (`use_subscribed_fetch`).
///
/// Derefs to `Fetch<T>` so `data`, `is_loading`, and `error` remain reachable
/// as field access. `connection_status` is the live SSE connection state for
/// UI freshness indicators. Refetches are driven by SSE events, not the
/// caller, so there's no manual `refetch` callback.
#[derive(Clone, PartialEq)]
pub struct SubscribedFetchHookReturn<T: Clone + PartialEq> {
    pub inner: Fetch<T>,
    pub connection_status: ConnectionStatus,
}

impl<T: Clone + PartialEq> Deref for SubscribedFetchHookReturn<T> {
    type Target = Fetch<T>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[allow(dead_code)]
impl<T: Clone + PartialEq> Fetch<T> {
    /// Map the fetched value to a new type. `is_loading` and `errors` are
    /// preserved.
    pub fn map<U, F>(self, f: F) -> Fetch<U>
    where
        U: Clone + PartialEq,
        F: FnOnce(T) -> U,
    {
        Fetch {
            data: self.data.map(f),
            is_loading: self.is_loading,
            errors: self.errors,
        }
    }

    /// Like `map`, but borrows the input. The errors vec is cloned (cheap —
    /// usually empty); the data is borrowed and remapped via the closure.
    pub fn map_ref<U, F>(&self, f: F) -> Fetch<U>
    where
        U: Clone + PartialEq,
        F: FnOnce(&T) -> U,
    {
        Fetch {
            data: self.data.map_ref(f),
            is_loading: self.is_loading,
            errors: self.errors.clone(),
        }
    }

    /// Transform the errors vector. Useful for flattening, deduping, or
    /// prefixing errors before passing into a render helper. Data and
    /// `is_loading` pass through unchanged.
    pub fn map_err<F>(mut self, f: F) -> Self
    where
        F: FnOnce(Vec<String>) -> Vec<String>,
    {
        self.errors = f(self.errors);
        self
    }

    /// Combine two `Fetch` bundles into a single `Fetch<(T, U)>`. Data:
    /// both inputs Fetched → `Fetched((t, u))`; otherwise `NotFetched`.
    /// `is_loading`: any input loading → result loading. `errors`: errors
    /// from both inputs are concatenated, preserving order.
    pub fn zip<U>(self, other: Fetch<U>) -> Fetch<(T, U)>
    where
        U: Clone + PartialEq,
    {
        let mut errors = self.errors;
        errors.extend(other.errors);
        Fetch {
            data: self.data.zip(other.data),
            is_loading: self.is_loading || other.is_loading,
            errors,
        }
    }

    /// Like `zip`, but borrows both inputs. Data is borrowed and combined
    /// into `Fetched((&t, &u))`; errors and `is_loading` are aggregated as
    /// in `zip` (errors cloned from each input).
    pub fn zip_ref<'a, U>(
        &'a self,
        other: &'a Fetch<U>,
    ) -> Fetch<(&'a T, &'a U)>
    where
        U: Clone + PartialEq,
    {
        let mut errors = self.errors.clone();
        errors.extend(other.errors.iter().cloned());
        Fetch {
            data: self.data.zip_ref(&other.data),
            is_loading: self.is_loading || other.is_loading,
            errors,
        }
    }

    /// Compare two `Fetch` values for sorting. Fetched values come before
    /// `NotFetched` ones; ties within `Fetched` are broken by `cmp_value`,
    /// which receives the inner data of both. Note that `is_loading` and
    /// `errors` are deliberately ignored — sort order is a property of the
    /// data dimension.
    ///
    /// For `Fetch<Option<T>>`, the closure handles the `Option`'s ordering
    /// (e.g., `Some` before `None`); reverse with `.reverse()` inside the
    /// closure to flip just the data dimension.
    pub fn cmp_by<F>(&self, other: &Self, cmp_value: F) -> Ordering
    where
        F: FnOnce(&T, &T) -> Ordering,
    {
        match (&self.data, &other.data) {
            (FetchData::Fetched(a), FetchData::Fetched(b)) => cmp_value(a, b),
            (FetchData::Fetched(_), FetchData::NotFetched) => Ordering::Less,
            (FetchData::NotFetched, FetchData::Fetched(_)) => Ordering::Greater,
            (FetchData::NotFetched, FetchData::NotFetched) => Ordering::Equal,
        }
    }

    /// Render based on fetch state. Centralizes the state-machine logic for
    /// "data is shown unconditionally once fetched, even during refetch":
    /// once data lands, it stays visible — `is_loading` / `errors` are
    /// passed to the `on_value` closure so the consumer can render a subtle
    /// refresh indicator or stale-data warning without hiding the data.
    ///
    /// State transitions:
    /// - `NotFetched` + no errors → `on_loading()`
    /// - `NotFetched` + errors → `on_error(errors)` (initial fetch failed)
    /// - `Fetched(v)` → `on_value(v, is_loading, errors)` regardless
    ///
    /// `render` is the bare primitive — the only place that needs to peek
    /// at the internal state-machine fields. Most call sites should use
    /// `render_section` (page/section-level) or `render_cell` (per-cell)
    /// instead; both are free functions built on top of `render` and treat
    /// `Fetch` as a black box. Reach for `render` only when neither named
    /// idiom fits.
    pub fn render<OnValue, OnLoading, OnError>(
        &self,
        on_value: OnValue,
        on_loading: OnLoading,
        on_error: OnError,
    ) -> Html
    where
        OnValue: Fn(&T, bool, &[String]) -> Html,
        OnLoading: Fn() -> Html,
        OnError: Fn(&[String]) -> Html,
    {
        match &self.data {
            FetchData::NotFetched => {
                if self.errors.is_empty() {
                    on_loading()
                } else {
                    on_error(&self.errors)
                }
            }
            FetchData::Fetched(data) => {
                on_value(data, self.is_loading, &self.errors)
            }
        }
    }
}

/// Page/section-level render helper. Loading shows "Loading {context}..."
/// centered; errors show as a list of banner cards. The `on_value` closure
/// receives `(data, is_loading, errors)` so it can render a subtle
/// "Refreshing..." indicator and an inline error banner alongside the data
/// during refetches.
pub fn render_section<T, F>(
    inner: &Fetch<T>,
    context: &str,
    on_value: F,
) -> Html
where
    T: Clone + PartialEq,
    F: Fn(&T, bool, &[String]) -> Html,
{
    let context_for_loading = context.to_string();
    let context_for_error = context.to_string();
    inner.render(
        on_value,
        move || {
            html! {
                <div class="text-center py-12">
                    <p class="text-neutral-600 dark:text-neutral-400">
                        {format!("Loading {}...", context_for_loading)}
                    </p>
                </div>
            }
        },
        move |errors: &[String]| {
            html! {
                <div class="space-y-2">
                    {for errors.iter().map(|err| html! {
                        <div class="p-4 rounded-md bg-red-50 \
                                   dark:bg-red-900/20 border \
                                   border-red-200 dark:border-red-800">
                            <p class="text-sm text-red-700 \
                                      dark:text-red-400">
                                {format!(
                                    "Error loading {}: {}",
                                    context_for_error, err,
                                )}
                            </p>
                        </div>
                    })}
                </div>
            }
        },
    )
}

/// Per-cell render helper. Loading shows a small skeleton block; errors show
/// a compact glyph whose tooltip joins the errors with a separator. Suitable
/// for table cells and other tight spaces where the section-level loading
/// indicator already covers the broader context.
///
/// The `on_value` closure does not receive `is_loading` / `errors` — a
/// refreshing cell should not show its own inline indicator; the section
/// level handles that.
#[allow(dead_code)]
pub fn render_cell<T, F>(inner: &Fetch<T>, on_value: F) -> Html
where
    T: Clone + PartialEq,
    F: Fn(&T) -> Html,
{
    inner.render(
        |value, _is_loading, _errors| on_value(value),
        || {
            html! {
                <span
                    class="inline-block h-4 w-16 rounded \
                           bg-neutral-200 dark:bg-neutral-700 animate-pulse"
                    aria-label="Loading"
                />
            }
        },
        |errors: &[String]| {
            let tooltip = errors.join("\n\n");
            html! {
                <span
                    class="inline-flex items-center justify-center h-4 w-4 \
                           rounded-full bg-red-100 dark:bg-red-900/40 \
                           text-red-700 dark:text-red-400 text-xs font-bold \
                           cursor-help"
                    title={tooltip}
                    aria-label="Error"
                >
                    {"!"}
                </span>
            }
        },
    )
}

/// Render a "stale data" banner for refetch errors that occurred while
/// data was already fetched. Use this inside a `render_section` `on_value`
/// closure to surface refetch failures to the user — without it, a
/// background refetch error is silently dropped and the user keeps seeing
/// stale data with no indication.
///
/// Returns an empty `html!` when `errors` is empty, so it's safe to call
/// unconditionally at the top of an `on_value` closure.
pub fn stale_data_banner(errors: &[String]) -> Html {
    if errors.is_empty() {
        return html! {};
    }
    html! {
        <div class="mb-4 p-3 rounded-md bg-amber-50 dark:bg-amber-900/20 \
                    border border-amber-200 dark:border-amber-800">
            <p class="text-sm font-medium text-amber-800 \
                      dark:text-amber-200 mb-1">
                {"Some data couldn't be refreshed; what you see may be \
                  out of date. Refresh the page to retry."}
            </p>
            <ul class="text-xs text-amber-700 dark:text-amber-300 \
                       list-disc list-inside space-y-0.5">
                {for errors.iter().map(|err| html! { <li>{err}</li> })}
            </ul>
        </div>
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
/// Manages the data/error/is_loading state and the fetch callback, but does
/// NOT auto-trigger the fetch. Compose with `use_on_mount_lifecycle` (for
/// today's `use_fetch` behavior) or `use_subscription_lifecycle` (for SSE-
/// driven fetches).
///
/// The returned `refetch` callback is the `do_fetch` that lifecycle hooks
/// invoke. There is no `is_loading` guard — `error` and `is_loading` are
/// purely informational; whether a refetch should fire is the lifecycle's
/// concern.
#[hook]
pub fn use_fetch_state<T, D, F, Fut>(deps: D, fetch_fn: F) -> FetchHookReturn<T>
where
    T: Clone + PartialEq + 'static,
    D: PartialEq + Clone + 'static,
    F: Fn() -> Fut + 'static,
    Fut: Future<Output = Result<T, String>> + 'static,
{
    let data = use_state(|| FetchData::NotFetched);
    let errors = use_state(Vec::<String>::new);
    let is_loading = use_state(|| false);

    let refetch = {
        let data = data.clone();
        let errors = errors.clone();
        let is_loading = is_loading.clone();
        let fetch_fn = Rc::new(fetch_fn);

        use_callback(deps, move |_, _| {
            let data = data.clone();
            let errors = errors.clone();
            let is_loading = is_loading.clone();
            let fetch_fn = fetch_fn.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);

                match fetch_fn().await {
                    Ok(result) => {
                        data.set(FetchData::Fetched(result));
                        errors.set(vec![]);
                    }
                    Err(e) => {
                        errors.set(vec![e]);
                    }
                }

                is_loading.set(false);
            });
        })
    };

    FetchHookReturn {
        inner: Fetch {
            data: (*data).clone(),
            is_loading: *is_loading,
            errors: (*errors).clone(),
        },
        refetch: Callback::from(move |_| refetch.emit(())),
    }
}

/// Fetch hook that auto-triggers on mount and on dep change.
///
/// For SSE-driven fetches, see `use_subscribed_fetch`.
#[hook]
pub fn use_fetch<T, D, F, Fut>(deps: D, fetch_fn: F) -> FetchHookReturn<T>
where
    T: Clone + PartialEq + 'static,
    D: PartialEq + Clone + 'static,
    F: Fn() -> Fut + 'static,
    Fut: Future<Output = Result<T, String>> + 'static,
{
    let hook = use_fetch_state(deps.clone(), fetch_fn);
    {
        let refetch = hook.refetch.clone();
        use_effect_with(deps, move |_| {
            refetch.emit(());
        });
    }
    hook
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
/// and returns cached data via FetchData to distinguish between "not
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
    T: Clone + PartialEq + 'static,
    D: PartialEq + Clone + 'static,
    GetCached: Fn() -> Option<T> + 'static,
    ShouldFetch: Fn() -> bool + 'static,
    FetchAndCache: Fn() -> Fut + 'static,
    Fut: Future<Output = Result<T, String>> + 'static,
{
    // Reuse use_fetch_state for is_loading / error / refetch. Its own
    // `data: FetchData<T>` is unused — for cached fetches the cache is the
    // source of truth, read separately below on every render so updates
    // from elsewhere (e.g., another hook writing to the same yewdux entry)
    // are reflected here.
    let inner = use_fetch_state(deps.clone(), fetch_and_cache);

    // Auto-fetch on mount when should_fetch says so. The closure passed to
    // use_effect_with is rebuilt each render and captures whatever
    // should_fetch closure (and its captured state) was constructed this
    // render. Yew runs the freshly-passed closure when deps change, so
    // should_fetch always reflects the latest captured state — we don't
    // need to (and can't) put the closure itself in deps.
    {
        let refetch = inner.refetch.clone();
        use_effect_with(deps, move |_| {
            if should_fetch() {
                refetch.emit(());
            }
        });
    }

    let data = match get_cached() {
        Some(cached) => FetchData::Fetched(cached),
        None => FetchData::NotFetched,
    };

    // Effective is_loading: loading OR initial state with no data, so a
    // caller can distinguish "not fetched yet" from "should_fetch() returned
    // false on mount."
    let effective_is_loading = inner.inner.is_loading
        || (!data.is_fetched() && inner.inner.errors.is_empty());

    FetchHookReturn {
        inner: Fetch {
            data,
            is_loading: effective_is_loading,
            errors: inner.inner.errors,
        },
        refetch: inner.refetch,
    }
}

/// Internal: run the subscribed-fetch lifecycle for a given `do_fetch`.
/// Owns the `connection_status` and `first_fetch_done` state, schedules the
/// speculative + failure timers, and registers with the subscription
/// registry. Returns the current `ConnectionStatus` snapshot.
///
/// Used by `use_subscribed_fetch` to share the lifecycle code with any
/// future subscribed-fetch composers.
#[hook]
fn use_subscription_lifecycle<D>(
    deps: D,
    auction_id: AuctionId,
    events: &'static [SubscribedEvent],
    do_fetch: Callback<()>,
) -> ConnectionStatus
where
    D: PartialEq + Clone + 'static,
{
    let status = use_state(|| ConnectionStatus::Connecting);

    {
        let do_fetch = do_fetch;
        let status = status.clone();

        use_effect_with((deps, auction_id), move |&(_, auction_id)| {
            // `first_fetch_done` and `current_status` are shared mutable
            // state read/written by both the timers and the registry
            // status callback. Reading them through `UseStateHandle` would
            // see only the value captured when the effect ran, since
            // handles cache the store value at creation time — a later
            // `set` call from one closure would not be visible to another.
            // `Rc<Cell<...>>` gives us a shared current value across all
            // closures driven by this effect.
            let first_fetch_done = Rc::new(Cell::new(false));
            let current_status =
                Rc::new(Cell::new(ConnectionStatus::Connecting));

            let speculative_timer = Timeout::new(
                SPECULATIVE_FETCH_AFTER.as_millis() as u32,
                {
                    let do_fetch = do_fetch.clone();
                    let first_fetch_done = first_fetch_done.clone();
                    move || {
                        if !first_fetch_done.get() {
                            tracing::info!(
                                ?auction_id,
                                "speculative fetch firing (SSE not yet attached)"
                            );
                            first_fetch_done.set(true);
                            do_fetch.emit(());
                        }
                    }
                },
            );

            let failure_timer =
                Timeout::new(FAILURE_AFTER.as_millis() as u32, {
                    let status_handle = status.clone();
                    let current_status = current_status.clone();
                    move || {
                        if matches!(
                            current_status.get(),
                            ConnectionStatus::Connecting
                        ) {
                            tracing::warn!(
                                ?auction_id,
                                "SSE failure deadline reached \
                                 (still Connecting), marking Failed"
                            );
                            current_status.set(ConnectionStatus::Failed);
                            status_handle.set(ConnectionStatus::Failed);
                        }
                    }
                });

            let token = registry::register(
                auction_id,
                SubscribedEvent::refetches_for(events, do_fetch.clone()),
                Callback::from({
                    let status_handle = status.clone();
                    let current_status = current_status.clone();
                    let do_fetch = do_fetch.clone();
                    let first_fetch_done = first_fetch_done.clone();
                    move |new_status: ConnectionStatus| {
                        current_status.set(new_status);
                        status_handle.set(new_status);
                        if matches!(new_status, ConnectionStatus::Connected) {
                            first_fetch_done.set(true);
                            // Fire on every Connected, including reconnects:
                            // events that fired during a disconnect window
                            // are not redelivered, so refetch to be safe.
                            do_fetch.emit(());
                        }
                    }
                }),
            );

            move || {
                drop(speculative_timer);
                drop(failure_timer);
                registry::unregister(auction_id, token);
            }
        });
    }

    *status
}

/// Subscription-aware fetch hook.
///
/// Like `use_fetch`, but the initial fetch is gated on the SSE connection for
/// `auction_id`, and any of the events in `events` will trigger a refetch on
/// receipt. The contract is "this hook always reflects current state" — at
/// the cost of a brief delay before the initial fetch while we wait for SSE
/// to confirm we'll be notified of changes.
///
/// Lifecycle:
/// - SSE attaches < 1s: fetch fires once on attach.
/// - 1s without SSE attach: speculative fetch fires so the page paints. If SSE
///   later attaches, an insurance fetch fires too.
/// - 5s without SSE attach: status → `Failed`. Indicator becomes visible.
/// - Reconnect: insurance fetch fires on each `Connected` transition.
/// - Deps change after first fetch: behaves like `use_fetch` (immediate fetch).
///   Deps change *before* first fetch: lifecycle restarts.
#[hook]
pub fn use_subscribed_fetch<T, D, F, Fut>(
    deps: D,
    auction_id: AuctionId,
    events: &'static [SubscribedEvent],
    fetch_fn: F,
) -> SubscribedFetchHookReturn<T>
where
    T: Clone + PartialEq + 'static,
    D: PartialEq + Clone + 'static,
    F: Fn() -> Fut + 'static,
    Fut: Future<Output = Result<T, String>> + 'static,
{
    let hook = use_fetch_state(deps.clone(), fetch_fn);
    let connection_status =
        use_subscription_lifecycle(deps, auction_id, events, hook.refetch);

    SubscribedFetchHookReturn {
        inner: hook.inner,
        connection_status,
    }
}
