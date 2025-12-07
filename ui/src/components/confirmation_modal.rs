use wasm_bindgen::JsCast;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ConfirmationModalProps {
    /// Modal title (e.g., "Delete Account")
    pub title: AttrValue,
    /// Warning message explaining consequences
    pub message: AttrValue,
    /// Confirm button text (e.g., "Delete Account")
    pub confirm_text: AttrValue,
    /// The value user must type to confirm (e.g., username or community name)
    pub confirmation_value: AttrValue,
    /// Label for what the user is typing (e.g., "your username")
    pub confirmation_label: AttrValue,
    /// Called when user confirms the action
    pub on_confirm: Callback<()>,
    /// Called when user cancels or clicks backdrop
    pub on_close: Callback<()>,
    /// Whether a delete/confirm operation is in progress
    #[prop_or_default]
    pub is_loading: bool,
    /// Error message to display
    #[prop_or_default]
    pub error_message: Option<AttrValue>,
}

#[function_component]
pub fn ConfirmationModal(props: &ConfirmationModalProps) -> Html {
    let confirmation_input = use_state(String::new);
    let backdrop_ref = use_node_ref();

    let can_confirm = *confirmation_input == props.confirmation_value.as_str();

    let on_backdrop_click = {
        let on_close = props.on_close.clone();
        let backdrop_ref = backdrop_ref.clone();
        Callback::from(move |e: MouseEvent| {
            if let Some(backdrop_element) =
                backdrop_ref.cast::<web_sys::Element>()
                && let Some(target) = e.target()
                && target.dyn_ref::<web_sys::Element>()
                    == Some(&backdrop_element)
            {
                on_close.emit(());
            }
        })
    };

    let on_input = {
        let confirmation_input = confirmation_input.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            confirmation_input.set(input.value());
        })
    };

    let on_confirm_click = {
        let on_confirm = props.on_confirm.clone();
        Callback::from(move |_: MouseEvent| {
            on_confirm.emit(());
        })
    };

    let on_cancel_click = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| {
            on_close.emit(());
        })
    };

    html! {
        <div
            ref={backdrop_ref.clone()}
            onclick={on_backdrop_click}
            class="fixed inset-0 bg-neutral-900 bg-opacity-50 z-50 flex items-center justify-center p-4"
        >
            <div class="bg-white dark:bg-neutral-800 rounded-lg shadow-xl max-w-md w-full p-6">
                <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                    {&props.title}
                </h3>

                <div class="space-y-4">
                    <p class="text-sm text-neutral-600 dark:text-neutral-400">
                        {"This action "}
                        <span class="font-semibold text-red-600 dark:text-red-400">
                            {"cannot be undone"}
                        </span>
                        {". "}{&props.message}
                    </p>

                    <p class="text-sm text-neutral-600 dark:text-neutral-400">
                        {"Please type "}
                        <span class="font-mono font-semibold text-neutral-900 dark:text-neutral-100">
                            {&props.confirmation_value}
                        </span>
                        {" to confirm."}
                    </p>

                    <input
                        type="text"
                        value={(*confirmation_input).clone()}
                        oninput={on_input}
                        placeholder={format!("Enter {}", props.confirmation_label)}
                        disabled={props.is_loading}
                        class="w-full px-3 py-2 text-sm border border-neutral-300 dark:border-neutral-600
                               rounded-md bg-white dark:bg-neutral-700
                               text-neutral-900 dark:text-neutral-100
                               placeholder-neutral-400 dark:placeholder-neutral-500
                               focus:outline-none focus:ring-2 focus:ring-red-500 focus:border-red-500
                               disabled:opacity-50 disabled:cursor-not-allowed"
                    />

                    if let Some(error) = &props.error_message {
                        <div class="text-sm text-red-600 dark:text-red-400">
                            {error}
                        </div>
                    }
                </div>

                <div class="flex justify-end gap-3 mt-6">
                    <button
                        onclick={on_cancel_click}
                        disabled={props.is_loading}
                        class="px-4 py-2 text-sm font-medium text-neutral-700 dark:text-neutral-300
                               bg-white dark:bg-neutral-700 border border-neutral-300 dark:border-neutral-600
                               rounded-md hover:bg-neutral-50 dark:hover:bg-neutral-600
                               disabled:opacity-50 disabled:cursor-not-allowed
                               transition-colors"
                    >
                        {"Cancel"}
                    </button>
                    <button
                        onclick={on_confirm_click}
                        disabled={!can_confirm || props.is_loading}
                        class="px-4 py-2 text-sm font-medium text-white
                               bg-red-600 hover:bg-red-700 dark:bg-red-700 dark:hover:bg-red-600
                               rounded-md disabled:opacity-50 disabled:cursor-not-allowed
                               transition-colors"
                    >
                        {if props.is_loading { "Processing..." } else { &props.confirm_text }}
                    </button>
                </div>
            </div>
        </div>
    }
}
