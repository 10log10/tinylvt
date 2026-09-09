use payloads::{SpaceCategoryId, responses};
use wasm_bindgen::JsCast;
use web_sys::HtmlSelectElement;
use yew::prelude::*;

/// Display name for a category bucket: `None` is the uncategorized
/// bucket, and an id whose category is missing from the list (deleted
/// after rows referenced it) reads "Unknown category".
pub fn category_name(
    category_id: Option<SpaceCategoryId>,
    categories: &[responses::SpaceCategory],
) -> String {
    match category_id {
        None => "Uncategorized".to_string(),
        Some(id) => categories
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "Unknown category".to_string()),
    }
}

/// Format eligibility points without a trailing ".0" for whole numbers,
/// since with 1-point spaces point totals read as item counts.
pub fn format_points(points: f64) -> String {
    if points.fract() == 0.0 {
        format!("{}", points as i64)
    } else {
        format!("{}", points)
    }
}

/// Dropdown for choosing a space category, with an explicit
/// "Uncategorized" option mapping to None. Callers fetch the community's
/// categories with `use_space_categories` and pass them in.
///
/// TODO: stale categories accumulate over time, since categories are
/// community-scoped and one referenced by auction history (soft-deleted
/// spaces or a concluded capped auction's cap rows) can never be deleted.
/// If dropdown clutter becomes a problem, filter options per surface — the
/// cap editor could show only categories on the auction site's undeleted
/// spaces — but the space forms can't use that predicate: a fresh category
/// has no spaces yet, and a category live only on another site is
/// deliberately assignable here, so those may need an archived flag
/// instead.
#[derive(Properties, PartialEq)]
pub struct Props {
    pub categories: Vec<responses::SpaceCategory>,
    pub value: Option<SpaceCategoryId>,
    pub on_change: Callback<Option<SpaceCategoryId>>,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub id: Option<AttrValue>,
    /// Sizing/placement classes appended to the base styling. The default
    /// full width suits form layouts; inline forms pass their own (e.g.
    /// "text-sm" for an intrinsic-width, small-text select).
    #[prop_or(AttrValue::Static("w-full"))]
    pub class: AttrValue,
}

#[function_component]
pub fn CategorySelect(props: &Props) -> Html {
    let on_change = {
        let on_change = props.on_change.clone();
        Callback::from(move |e: Event| {
            let select =
                e.target().unwrap().dyn_into::<HtmlSelectElement>().unwrap();
            let value = select.value();
            let category_id = if value.is_empty() {
                None
            } else {
                value.parse().ok().map(SpaceCategoryId)
            };
            on_change.emit(category_id);
        })
    };

    let selected = props.value.map(|id| id.to_string()).unwrap_or_default();

    html! {
        <select
            id={props.id.clone()}
            disabled={props.disabled}
            onchange={on_change}
            class={format!(
                "px-3 py-2 border border-neutral-300 \
                 dark:border-neutral-600 rounded-md shadow-sm bg-white \
                 dark:bg-neutral-700 text-neutral-900 \
                 dark:text-neutral-100 focus:outline-none focus:ring-2 \
                 focus:ring-neutral-500 focus:border-neutral-500 \
                 disabled:opacity-50 disabled:cursor-not-allowed {}",
                props.class,
            )}
        >
            <option value="" selected={selected.is_empty()}>
                {"Uncategorized"}
            </option>
            {for props.categories.iter().map(|category| {
                let id_str = category.id.to_string();
                html! {
                    <option
                        value={id_str.clone()}
                        selected={id_str == selected}
                    >
                        {&category.name}
                    </option>
                }
            })}
        </select>
    }
}
