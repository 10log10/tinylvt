use super::ToastItem;
use crate::contexts::toast::ToastContext;
use yew::prelude::*;

#[function_component]
pub fn ToastContainer() -> Html {
    let toast_context = use_context::<ToastContext>();

    let toasts = match toast_context {
        Some(context) => {
            // Convert HashMap values to Vec and sort by creation time (using UUID for now)
            let mut toasts: Vec<_> = context.toasts.values().cloned().collect();
            toasts.sort_by_key(|toast| toast.id.to_string()); // Simple ordering by UUID string
            toasts
        }
        None => vec![],
    };

    if toasts.is_empty() {
        return html! {};
    }

    html! {
        <div class="fixed top-4 right-4 z-50 space-y-3 max-w-sm w-full">
            {for toasts.iter().map(|toast| {
                html! {
                    <ToastItem key={toast.id.to_string()} toast={toast.clone()} />
                }
            })}
        </div>
    }
}
