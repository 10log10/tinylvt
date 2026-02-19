use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    /// Current offset (0-indexed)
    pub offset: i64,
    /// Items per page
    pub limit: i64,
    /// Number of items returned in current fetch (to detect last page)
    pub current_count: usize,
    /// Callback when offset changes
    pub on_offset_change: Callback<i64>,
    /// Whether currently loading (to disable buttons)
    #[prop_or(false)]
    pub is_loading: bool,
}

#[function_component]
pub fn PaginationControls(props: &Props) -> Html {
    let Props {
        offset,
        limit,
        current_count,
        is_loading,
        ..
    } = *props;

    // Don't show controls if no items at all on first page
    if offset == 0 && current_count == 0 {
        return html! {};
    }

    let is_first_page = offset == 0;
    let is_last_page = (current_count as i64) < limit;

    let on_previous = {
        let on_offset_change = props.on_offset_change.clone();
        Callback::from(move |_: MouseEvent| {
            let new_offset = (offset - limit).max(0);
            on_offset_change.emit(new_offset);
        })
    };

    let on_next = {
        let on_offset_change = props.on_offset_change.clone();
        Callback::from(move |_: MouseEvent| {
            on_offset_change.emit(offset + limit);
        })
    };

    // Calculate display range
    let range_start = offset + 1;
    let range_end = offset + (current_count as i64);

    let prev_disabled = is_first_page || is_loading;
    let next_disabled = is_last_page || is_loading;

    let button_class = |disabled: bool| {
        if disabled {
            "px-4 py-2 border border-neutral-300 dark:border-neutral-600 \
             rounded-md text-sm font-medium text-neutral-400 \
             dark:text-neutral-500 bg-neutral-100 dark:bg-neutral-800 \
             cursor-not-allowed"
        } else {
            "px-4 py-2 border border-neutral-300 dark:border-neutral-600 \
             rounded-md text-sm font-medium text-neutral-700 \
             dark:text-neutral-300 bg-white dark:bg-neutral-700 \
             hover:bg-neutral-50 dark:hover:bg-neutral-600 \
             transition-colors duration-200"
        }
    };

    html! {
        <div class="flex items-center justify-between mt-4 pt-4 \
                    border-t border-neutral-200 dark:border-neutral-700">
            <button
                onclick={on_previous}
                disabled={prev_disabled}
                class={button_class(prev_disabled)}
            >
                {"Previous"}
            </button>

            <span class="text-sm text-neutral-600 dark:text-neutral-400">
                {format!("Showing {}-{}", range_start, range_end)}
            </span>

            <button
                onclick={on_next}
                disabled={next_disabled}
                class={button_class(next_disabled)}
            >
                {"Next"}
            </button>
        </div>
    }
}
