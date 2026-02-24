use crate::Route;
use crate::components::layout::DocsLayout;
use markdown_html::markdown_html;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_router::prelude::*;

/// Wrapper that intercepts clicks on internal links and routes them
/// through yew_router instead of triggering full page loads.
#[derive(Properties, PartialEq)]
struct MarkdownContentProps {
    html: &'static str,
}

#[function_component]
fn MarkdownContent(props: &MarkdownContentProps) -> Html {
    let navigator = use_navigator().expect("Must be used within a Router");

    let onclick = {
        let navigator = navigator.clone();
        Callback::from(move |e: MouseEvent| {
            // Check if the click target is an anchor tag
            let target = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok());
            let anchor = target.and_then(|el| {
                // Check if clicked element or parent is an anchor
                if el.tag_name() == "A" {
                    Some(el)
                } else {
                    el.closest("a").ok().flatten()
                }
            });

            // Try to recognize as an internal route
            if let Some(anchor) = anchor
                && let Some(href) = anchor.get_attribute("href")
                && let Some(route) = Route::recognize(&href)
            {
                e.prevent_default();
                navigator.push(&route);
                // Scroll to top after navigation
                if let Some(window) = web_sys::window() {
                    window.scroll_to_with_x_and_y(0.0, 0.0);
                }
            }
        })
    };

    html! {
        <div {onclick} class="prose dark:prose-invert max-w-none">
            { Html::from_html_unchecked(props.html.into()) }
        </div>
    }
}

#[function_component]
pub fn DocsPage() -> Html {
    html! {
        <DocsLayout>
            <div class="max-w-4xl mx-auto px-4 py-8">
                <MarkdownContent html={markdown_html!(file: "docs/getting-started.md")} />
            </div>
        </DocsLayout>
    }
}

#[function_component]
pub fn CurrencyPage() -> Html {
    html! {
        <DocsLayout>
            <div class="max-w-4xl mx-auto px-4 py-8">
                <MarkdownContent html={markdown_html!(file: "docs/currency.md")} />
            </div>
        </DocsLayout>
    }
}

#[function_component]
pub fn AuctionsPage() -> Html {
    html! {
        <DocsLayout>
            <div class="max-w-4xl mx-auto px-4 py-8">
                <MarkdownContent html={markdown_html!(file: "docs/auctions.md")} />
            </div>
        </DocsLayout>
    }
}

#[function_component]
pub fn SetupPage() -> Html {
    html! {
        <DocsLayout>
            <div class="max-w-4xl mx-auto px-4 py-8">
                <MarkdownContent html={markdown_html!(file: "docs/setup.md")} />
            </div>
        </DocsLayout>
    }
}

#[function_component]
pub fn DeskAllocationPage() -> Html {
    html! {
        <DocsLayout>
            <div class="max-w-4xl mx-auto px-4 py-8">
                <MarkdownContent html={markdown_html!(file: "docs/desk-allocation.md")} />
            </div>
        </DocsLayout>
    }
}
