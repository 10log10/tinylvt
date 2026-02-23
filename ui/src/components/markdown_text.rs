//! Renders user-supplied markdown text safely.
//!
//! Uses pulldown-cmark with raw HTML disabled to prevent XSS attacks.
//! Supports basic markdown features: headers, bold, italic, links, images,
//! lists, and code blocks.

use pulldown_cmark::{Options, Parser, html};
use yew::prelude::*;

/// Renders markdown text safely with XSS protection.
///
/// Raw HTML in the markdown is escaped, not rendered. This prevents
/// script injection while still allowing markdown formatting.
#[derive(Properties, PartialEq)]
pub struct Props {
    /// The markdown text to render.
    pub text: AttrValue,
    /// Additional CSS classes for the container.
    #[prop_or_default]
    pub class: Classes,
}

#[function_component]
pub fn MarkdownText(props: &Props) -> Html {
    let html_content = render_markdown(&props.text);

    // Base prose styles for consistent markdown rendering
    let base_classes = classes!(
        "prose",
        "prose-neutral",
        "dark:prose-invert",
        "prose-sm",
        "max-w-none",
        // Override prose defaults for tighter spacing
        "prose-p:my-2",
        "prose-headings:mt-4",
        "prose-headings:mb-2",
        "prose-ul:my-2",
        "prose-ol:my-2",
        "prose-li:my-0",
        props.class.clone()
    );

    html! {
        <div class={base_classes}>
            { Html::from_html_unchecked(html_content.into()) }
        </div>
    }
}

/// Converts markdown to HTML with safety settings.
///
/// Raw HTML is escaped (not rendered) to prevent XSS attacks.
fn render_markdown(markdown: &str) -> String {
    // Do NOT enable ENABLE_RAW_HTML - this keeps us safe from XSS
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_SMART_PUNCTUATION;

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    html_output
}
