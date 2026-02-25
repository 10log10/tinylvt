use yew::prelude::*;

/// Sets the document title. No cleanup on unmount since each page sets its own
/// title, and unmount/mount ordering isn't guaranteed during route transitions.
#[hook]
pub fn use_title(title: &str) {
    let title = title.to_string();
    use_effect_with(title, |title| {
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            doc.set_title(title);
        }
    });
}
