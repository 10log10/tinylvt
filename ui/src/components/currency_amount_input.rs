use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    /// The raw input text; owned by the parent, which parses and
    /// validates it.
    pub value: UseStateHandle<String>,
    /// Currency symbol for the placeholder.
    pub symbol: String,
    pub disabled: bool,
}

/// A decimal amount input in the community's currency: standard width,
/// styling, and placeholder, shared by the payment forms.
#[function_component]
pub fn CurrencyAmountInput(props: &Props) -> Html {
    let oninput = {
        let value = props.value.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            value.set(input.value());
        })
    };
    html! {
        <input
            type="text"
            inputmode="decimal"
            value={(*props.value).clone()}
            oninput={oninput}
            placeholder={format!("Amount ({})", props.symbol)}
            disabled={props.disabled}
            class="w-40 px-3 py-2 border border-neutral-300 \
                   dark:border-neutral-600 rounded-md text-sm bg-white \
                   dark:bg-neutral-800 text-neutral-900 \
                   dark:text-neutral-100 focus:outline-none focus:ring-2 \
                   focus:ring-neutral-500"
        />
    }
}
