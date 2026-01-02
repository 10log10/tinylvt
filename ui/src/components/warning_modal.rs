use wasm_bindgen::JsCast;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct WarningModalProps {
    /// Modal title (e.g., "Auction In Progress")
    pub title: AttrValue,
    /// Warning message explaining the issue
    pub message: AttrValue,
    /// Proceed button text (e.g., "Proceed Anyway")
    pub proceed_text: AttrValue,
    /// Called when user proceeds
    pub on_proceed: Callback<()>,
    /// Called when user cancels or clicks backdrop
    pub on_cancel: Callback<()>,
}

#[function_component]
pub fn WarningModal(props: &WarningModalProps) -> Html {
    let backdrop_ref = use_node_ref();

    let on_backdrop_click = {
        let on_cancel = props.on_cancel.clone();
        let backdrop_ref = backdrop_ref.clone();
        Callback::from(move |e: MouseEvent| {
            if let Some(backdrop_element) =
                backdrop_ref.cast::<web_sys::Element>()
                && let Some(target) = e.target()
                && target.dyn_ref::<web_sys::Element>()
                    == Some(&backdrop_element)
            {
                on_cancel.emit(());
            }
        })
    };

    let on_proceed_click = {
        let on_proceed = props.on_proceed.clone();
        Callback::from(move |_: MouseEvent| {
            on_proceed.emit(());
        })
    };

    let on_cancel_click = {
        let on_cancel = props.on_cancel.clone();
        Callback::from(move |_: MouseEvent| {
            on_cancel.emit(());
        })
    };

    html! {
        <div
            ref={backdrop_ref.clone()}
            onclick={on_backdrop_click}
            class="fixed inset-0 bg-neutral-900 bg-opacity-50 z-50 \
                   flex items-center justify-center p-4"
        >
            <div class="bg-white dark:bg-neutral-800 rounded-lg shadow-xl \
                        max-w-md w-full p-6">
                <h3 class="text-lg font-semibold text-neutral-900 \
                           dark:text-neutral-100 mb-4">
                    {&props.title}
                </h3>

                <div class="space-y-4">
                    <p class="text-sm text-neutral-600 dark:text-neutral-400">
                        {&props.message}
                    </p>
                </div>

                <div class="flex justify-end gap-3 mt-6">
                    <button
                        onclick={on_cancel_click}
                        class="px-4 py-2 text-sm font-medium \
                               text-neutral-700 dark:text-neutral-300 \
                               bg-white dark:bg-neutral-700 \
                               border border-neutral-300 dark:border-neutral-600 \
                               rounded-md hover:bg-neutral-50 \
                               dark:hover:bg-neutral-600 \
                               transition-colors"
                    >
                        {"Cancel"}
                    </button>
                    <button
                        onclick={on_proceed_click}
                        class="px-4 py-2 text-sm font-medium text-white \
                               bg-neutral-900 hover:bg-neutral-800 \
                               dark:bg-neutral-100 dark:text-neutral-900 \
                               dark:hover:bg-neutral-200 \
                               rounded-md transition-colors"
                    >
                        {&props.proceed_text}
                    </button>
                </div>
            </div>
        </div>
    }
}
