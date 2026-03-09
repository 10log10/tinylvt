//! Text input component with length validation and character count feedback.

use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    /// The current value (controlled by parent).
    pub value: AttrValue,
    /// Called when the value changes.
    pub on_change: Callback<String>,
    /// Maximum allowed length.
    pub max_length: usize,
    /// Optional label text.
    #[prop_or_default]
    pub label: Option<AttrValue>,
    /// Optional placeholder text.
    #[prop_or_default]
    pub placeholder: Option<AttrValue>,
    /// Whether the field is required.
    #[prop_or_default]
    pub required: bool,
    /// Whether the input is disabled.
    #[prop_or_default]
    pub disabled: bool,
    /// Optional id for the input element.
    #[prop_or_default]
    pub id: Option<AttrValue>,
}

#[function_component]
pub fn TextInput(props: &Props) -> Html {
    let on_input = {
        let on_change = props.on_change.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            on_change.emit(input.value());
        })
    };

    // Character count logic - show when 80% of limit reached (20% remaining)
    let char_count = props.value.len();
    let warning_threshold = (props.max_length * 4) / 5; // 80% of max
    let show_char_count = char_count > warning_threshold;
    let is_over_limit = char_count > props.max_length;

    let char_count_class = if is_over_limit {
        "text-xs tabular-nums text-red-600 dark:text-red-400"
    } else {
        "text-xs tabular-nums text-neutral-500 dark:text-neutral-400"
    };

    let input_class = classes!(
        "w-full",
        "px-3",
        "py-2",
        "border",
        "rounded-md",
        "shadow-sm",
        "bg-white",
        "dark:bg-neutral-700",
        "text-neutral-900",
        "dark:text-neutral-100",
        "focus:outline-none",
        "focus:ring-2",
        "focus:ring-neutral-500",
        "focus:border-neutral-500",
        "dark:focus:ring-neutral-400",
        "dark:focus:border-neutral-400",
        "disabled:opacity-50",
        "disabled:cursor-not-allowed",
        if is_over_limit {
            "border-red-500 dark:border-red-400"
        } else {
            "border-neutral-300 dark:border-neutral-600"
        }
    );

    html! {
        <div>
            {if let Some(label) = &props.label {
                html! {
                    <label
                        for={props.id.clone()}
                        class="block text-sm font-medium text-neutral-700 \
                               dark:text-neutral-300 mb-2"
                    >
                        {label}
                        {if props.required { " *" } else { "" }}
                    </label>
                }
            } else {
                html! {}
            }}
            <input
                type="text"
                id={props.id.clone()}
                value={props.value.clone()}
                oninput={on_input}
                placeholder={props.placeholder.clone()}
                required={props.required}
                disabled={props.disabled}
                class={input_class}
            />
            {if show_char_count {
                html! {
                    <p class={char_count_class}>
                        {format!("{} / {}", char_count, props.max_length)}
                    </p>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
