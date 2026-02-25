use crate::Route;
use crate::hooks::use_push_route;
use yew::prelude::*;

/// A navigation item in the docs sidebar.
#[derive(Clone, PartialEq)]
pub struct DocNavItem {
    pub title: &'static str,
    pub route: Route,
}

/// Docs sidebar navigation items. Add new pages here.
pub const DOC_NAV_ITEMS: &[DocNavItem] = &[
    DocNavItem {
        title: "Getting Started",
        route: Route::Docs,
    },
    DocNavItem {
        title: "Currency Modes",
        route: Route::DocsCurrency,
    },
    DocNavItem {
        title: "Community Setup",
        route: Route::DocsSetup,
    },
    DocNavItem {
        title: "Auctions",
        route: Route::DocsAuctions,
    },
    DocNavItem {
        title: "Desk Allocation",
        route: Route::DocsDeskAllocation,
    },
    DocNavItem {
        title: "Rent Splitting",
        route: Route::DocsRentSplitting,
    },
];

#[derive(Properties, PartialEq)]
pub struct DocsSidebarProps {
    /// Current active route for highlighting.
    pub active_route: Route,
    /// Callback when a nav item is clicked (for closing mobile drawer).
    #[prop_or_default]
    pub on_navigate: Option<Callback<()>>,
}

const LINK_BASE_CLASSES: &str =
    "block px-4 py-2 text-sm transition-colors cursor-pointer";
const LINK_INACTIVE_CLASSES: &str = "text-neutral-600 dark:text-neutral-400 \
    hover:text-neutral-900 dark:hover:text-white \
    hover:bg-neutral-100 dark:hover:bg-neutral-800";
const LINK_ACTIVE_CLASSES: &str = "text-neutral-900 dark:text-white \
    bg-neutral-100 dark:bg-neutral-800 font-medium";

#[function_component]
pub fn DocsSidebar(props: &DocsSidebarProps) -> Html {
    let push_route = use_push_route();

    html! {
        <nav class="py-4">
            <div class="px-4 pb-2 text-xs font-semibold uppercase tracking-wider \
                        text-neutral-500 dark:text-neutral-400">
                {"Documentation"}
            </div>
            <ul>
                { for DOC_NAV_ITEMS.iter().map(|item| {
                    let is_active = props.active_route == item.route;

                    let link_classes = format!(
                        "{} {}",
                        LINK_BASE_CLASSES,
                        if is_active { LINK_ACTIVE_CLASSES } else { LINK_INACTIVE_CLASSES }
                    );

                    let on_click = {
                        let push_route = push_route.clone();
                        let route = item.route.clone();
                        let on_navigate = props.on_navigate.clone();
                        Callback::from(move |_: MouseEvent| {
                            push_route.emit(route.clone());
                            if let Some(ref cb) = on_navigate {
                                cb.emit(());
                            }
                        })
                    };

                    html! {
                        <li key={item.title}>
                            <div
                                class={link_classes}
                                onclick={on_click}
                            >
                                {item.title}
                            </div>
                        </li>
                    }
                })}
            </ul>
        </nav>
    }
}
