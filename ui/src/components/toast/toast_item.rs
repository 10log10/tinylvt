use crate::contexts::toast::{Toast, ToastType, use_toast};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ToastItemProps {
    pub toast: Toast,
}

#[function_component]
pub fn ToastItem(props: &ToastItemProps) -> Html {
    let toast_handle = use_toast();
    let toast = &props.toast;

    let (bg_class, border_class, text_class, icon) = match toast.toast_type {
        ToastType::Error => (
            "bg-red-50 dark:bg-red-900",
            "border-red-200 dark:border-red-800",
            "text-red-700 dark:text-red-400",
            "✕",
        ),
        ToastType::Success => (
            "bg-green-50 dark:bg-green-900",
            "border-green-200 dark:border-green-800",
            "text-green-700 dark:text-green-400",
            "✓",
        ),
        ToastType::Info => (
            "bg-neutral-50 dark:bg-neutral-800",
            "border-neutral-200 dark:border-neutral-700",
            "text-neutral-700 dark:text-neutral-300",
            "ℹ",
        ),
    };

    let on_close = {
        let toast_id = toast.id;
        let toast_handle = toast_handle.clone();
        Callback::from(move |_| {
            toast_handle.remove(toast_id);
        })
    };

    html! {
        <div class={format!(
            "relative p-4 rounded-lg border shadow-lg transform transition-all duration-300 ease-out {} {} {}",
            bg_class, border_class, text_class
        )}>
            <div class="flex items-start space-x-3">
                <div class="flex-shrink-0">
                    <span class="text-sm font-medium">{icon}</span>
                </div>
                <div class="flex-1 min-w-0">
                    <p class="text-sm font-medium leading-5">
                        {&toast.message}
                    </p>
                </div>
                <div class="flex-shrink-0">
                    <button
                        onclick={on_close}
                        class="inline-flex text-neutral-400 hover:text-neutral-600 dark:hover:text-neutral-200 focus:outline-none focus:text-neutral-600 dark:focus:text-neutral-200 transition-colors"
                        title="Dismiss"
                    >
                        <span class="text-lg leading-none">{"×"}</span>
                    </button>
                </div>
            </div>
        </div>
    }
}
