use wasm_bindgen::JsCast;
use yew::prelude::*;

/// A reusable modal component that supports backdrop clicks to close.
///
/// # Example
///
/// ```rust,ignore
/// use crate::components::Modal;
/// use yew::prelude::*;
///
/// #[function_component]
/// fn MyComponent() -> Html {
///     let show_modal = use_state(|| false);
///
///     let open_modal = {
///         let show_modal = show_modal.clone();
///         Callback::from(move |_| show_modal.set(true))
///     };
///
///     let close_modal = {
///         let show_modal = show_modal.clone();
///         Callback::from(move |_| show_modal.set(false))
///     };
///
///     html! {
///         <>
///             <button onclick={open_modal}>{"Open Modal"}</button>
///
///             if *show_modal {
///                 <Modal on_close={close_modal}>
///                     <h3>{"Modal Title"}</h3>
///                     <p>{"Modal content goes here"}</p>
///                     <button onclick={close_modal.reform(|_| ())}>
///                         {"Close"}
///                     </button>
///                 </Modal>
///             }
///         </>
///     }
/// }
/// ```
#[derive(Properties, PartialEq)]
pub struct ModalProps {
    /// Modal content (passed as children)
    pub children: Html,
    /// Called when user clicks backdrop or closes the modal
    pub on_close: Callback<()>,
    /// Maximum width class (default: "max-w-md")
    #[prop_or_else(|| AttrValue::from("max-w-md"))]
    pub max_width: AttrValue,
    /// Whether to allow closing by clicking backdrop (default: true)
    #[prop_or(true)]
    pub close_on_backdrop: bool,
}

#[function_component]
pub fn Modal(props: &ModalProps) -> Html {
    let backdrop_ref = use_node_ref();

    let on_backdrop_click = {
        let on_close = props.on_close.clone();
        let backdrop_ref = backdrop_ref.clone();
        let close_on_backdrop = props.close_on_backdrop;

        Callback::from(move |e: MouseEvent| {
            if !close_on_backdrop {
                return;
            }

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

    let max_width = props.max_width.to_string();

    html! {
        <div
            ref={backdrop_ref.clone()}
            onclick={on_backdrop_click}
            class="fixed inset-0 bg-black bg-opacity-50 z-50 flex
                   items-center justify-center p-4"
        >
            <div
                class={format!(
                    "bg-white dark:bg-neutral-800 rounded-lg shadow-xl \
                     w-full p-6 {}",
                    max_width
                )}
            >
                {props.children.clone()}
            </div>
        </div>
    }
}
