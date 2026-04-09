use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;

/// An inline-editable cell that toggles between a static display
/// and a text input. Clicking the display enters edit mode,
/// which auto-focuses and selects the content. Blurring or
/// pressing Enter commits the change; Escape cancels.
///
/// The `on_change` callback receives the new string value on
/// commit. The parent is responsible for validation and
/// reverting if the value is invalid.
///
/// When `on_remove` is provided, a ✕ button appears in edit
/// mode.
#[derive(Properties, PartialEq)]
pub struct Props {
    /// Current display value
    pub value: AttrValue,
    /// Text shown when value is empty
    #[prop_or(AttrValue::Static("\u{2014}"))]
    pub placeholder: AttrValue,
    /// Called with the new text value on commit
    pub on_change: Callback<String>,
    /// If set, shows a ✕ remove button in edit mode
    #[prop_or_default]
    pub on_remove: Option<Callback<()>>,
    /// Extra CSS classes for the outer container
    #[prop_or_default]
    pub class: Classes,
    /// Extra CSS classes applied to both display and input
    #[prop_or_default]
    pub inner_class: Classes,
    /// HTML input type (default "text")
    #[prop_or(AttrValue::Static("text"))]
    pub input_type: AttrValue,
    /// HTML inputmode attribute
    #[prop_or_default]
    pub inputmode: Option<AttrValue>,
    /// Called after a value is committed via Enter (not blur).
    /// Useful for advancing focus to the next cell.
    #[prop_or_default]
    pub on_enter: Option<Callback<()>>,
    /// Ref to the outer container, so the parent can
    /// programmatically `.click()` to enter edit mode.
    #[prop_or_default]
    pub container_ref: NodeRef,
}

const DISPLAY_CLASSES: &str = "\
    cursor-pointer px-2 py-1 rounded text-sm \
    border border-transparent \
    hover:border-neutral-300 dark:hover:border-neutral-600 \
    transition-colors";

const INPUT_CLASSES: &str = "\
    px-2 py-1 text-sm border \
    border-neutral-300 dark:border-neutral-600 \
    rounded bg-white dark:bg-neutral-800 \
    text-neutral-900 dark:text-neutral-100 \
    focus:outline-none focus:ring-1 \
    focus:ring-neutral-400";

const REMOVE_BTN: &str = "\
    text-neutral-400 dark:text-neutral-600 \
    hover:text-neutral-700 dark:hover:text-neutral-400 \
    text-sm shrink-0 p-1.5 rounded \
    hover:bg-neutral-100 dark:hover:bg-neutral-700 \
    transition-colors";

#[function_component]
pub fn InlineEdit(props: &Props) -> Html {
    let is_editing = use_state(|| false);
    let input_ref = use_node_ref();

    // Enter edit mode: focus and select
    let on_display_click = {
        let is_editing = is_editing.clone();
        let input_ref = input_ref.clone();
        Callback::from(move |_: MouseEvent| {
            is_editing.set(true);
            // Focus + select after the input renders
            let input_ref = input_ref.clone();
            yew::platform::spawn_local(async move {
                // Yield to let the DOM update
                gloo_timers::future::sleep(std::time::Duration::from_millis(0))
                    .await;
                if let Some(input) = input_ref.cast::<HtmlInputElement>() {
                    let _ = input.focus();
                    input.select();
                }
            });
        })
    };

    // Commit on blur
    let on_blur = {
        let is_editing = is_editing.clone();
        let on_change = props.on_change.clone();
        let input_ref = input_ref.clone();
        Callback::from(move |_: FocusEvent| {
            if let Some(input) = input_ref.cast::<HtmlInputElement>() {
                on_change.emit(input.value());
            }
            is_editing.set(false);
        })
    };

    // Enter/Tab commits and advances, Escape cancels
    let on_keydown = {
        let is_editing = is_editing.clone();
        let on_change = props.on_change.clone();
        let on_enter = props.on_enter.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" || e.key() == "Tab" {
                e.prevent_default();
                if let Some(input) = e
                    .target()
                    .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
                {
                    on_change.emit(input.value());
                }
                is_editing.set(false);
                if let Some(cb) = &on_enter {
                    cb.emit(());
                }
            } else if e.key() == "Escape" {
                e.prevent_default();
                is_editing.set(false);
            }
        })
    };

    let on_remove_click = {
        let on_remove = props.on_remove.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(cb) = &on_remove {
                cb.emit(());
            }
        })
    };

    let display_text = if props.value.is_empty() {
        props.placeholder.to_string()
    } else {
        props.value.to_string()
    };

    let empty_style = if props.value.is_empty() {
        "text-neutral-400 dark:text-neutral-600"
    } else {
        "text-neutral-900 dark:text-neutral-100"
    };

    html! {
        <div class={classes!(
            "flex", "items-center", "gap-1",
            props.class.clone()
        )}>
            {if *is_editing {
                html! {
                    <>
                        <input
                            ref={input_ref}
                            type={props.input_type.clone()}
                            inputmode={props.inputmode.clone()}
                            value={props.value.clone()}
                            onblur={on_blur}
                            onkeydown={on_keydown}
                            class={classes!(
                                INPUT_CLASSES,
                                "w-full",
                                props.inner_class.clone()
                            )}
                        />
                        {if props.on_remove.is_some() {
                            html! {
                                <button
                                    onmousedown={on_remove_click}
                                    class={REMOVE_BTN}
                                    title="Remove"
                                >
                                    {"\u{2715}"}
                                </button>
                            }
                        } else {
                            html! {}
                        }}
                    </>
                }
            } else {
                html! {
                    <div
                        ref={props.container_ref.clone()}
                        onclick={on_display_click}
                        class={classes!(
                            DISPLAY_CLASSES,
                            empty_style,
                            "w-full",
                            props.inner_class.clone()
                        )}
                    >
                        {display_text}
                    </div>
                }
            }}
        </div>
    }
}
