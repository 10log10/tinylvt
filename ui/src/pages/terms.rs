use crate::hooks::use_title;
use markdown_html::markdown_html;
use yew::prelude::*;

const TERMS_HTML: &str = markdown_html!(file: "docs/terms.md");

#[function_component]
pub fn TermsPage() -> Html {
    use_title("Terms of Service - TinyLVT");
    html! {
        <div class="max-w-3xl mx-auto px-4 py-8">
            <div class="prose dark:prose-invert max-w-none">
                {Html::from_html_unchecked(AttrValue::from(TERMS_HTML))}
            </div>
        </div>
    }
}
