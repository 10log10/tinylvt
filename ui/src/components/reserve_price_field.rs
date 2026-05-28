use payloads::{CurrencySettings, ReservePrice};
use rust_decimal::Decimal;
use std::str::FromStr;
use yew::prelude::*;

use super::InlineEdit;

/// Inline-editable reserve price input.
///
/// Raw input accepts any decimal precision; on commit the parent receives
/// the parsed `ReservePrice`. The display shows the currency-formatted
/// value. Empty input is parsed as zero so the user can clear and
/// re-enter. The backend enforces the negative-reserve gate, so this
/// component does not pre-validate sign.
#[derive(Properties, PartialEq)]
pub struct Props {
    pub value: ReservePrice,
    pub currency: CurrencySettings,
    pub on_change: Callback<ReservePrice>,
}

#[function_component]
pub fn ReservePriceField(props: &Props) -> Html {
    let on_change = {
        let on_change = props.on_change.clone();
        Callback::from(move |raw: String| {
            let parsed = if raw.trim().is_empty() {
                Some(Decimal::ZERO)
            } else {
                Decimal::from_str(raw.trim()).ok()
            };
            if let Some(v) = parsed {
                on_change.emit(ReservePrice(v));
            }
            // Invalid input is silently ignored; InlineEdit reverts to
            // the prior display value on blur.
        })
    };

    // Match the chrome of standard form inputs (border, padding, focus
    // ring) so the field reads as a normal text input in both display
    // and editing modes. `replace_classes` is required so InlineEdit's
    // tighter built-in padding doesn't conflict with this styling.
    // `w-full` on the input is needed to override the `size="1"`
    // attribute, which otherwise collapses the editing input to ~1
    // character wide.
    let chrome: yew::Classes = yew::classes!(
        "w-full",
        "block",
        "px-3",
        "py-2",
        "border",
        "border-neutral-300",
        "dark:border-neutral-600",
        "rounded-md",
        "shadow-sm",
        "bg-white",
        "dark:bg-neutral-700",
        "text-neutral-900",
        "dark:text-neutral-100",
        "text-sm",
        "focus:outline-none",
        "focus:ring-2",
        "focus:ring-neutral-500",
        "focus:border-neutral-500",
    );
    let mut display_chrome = chrome.clone();
    display_chrome.push("cursor-pointer");

    html! {
        <>
            <InlineEdit
                value={props.value.0.normalize().to_string()}
                display_value={AttrValue::from(
                    props.currency.format_amount(props.value.0)
                )}
                on_change={on_change}
                inputmode={AttrValue::Static("decimal")}
                display_class={display_chrome}
                input_class={chrome}
                replace_classes={true}
            />
            <p class="mt-1 text-xs text-neutral-500 dark:text-neutral-400">
                {"Starting price the first time anyone bids. Use a \
                  negative number for chores where the winner is \
                  compensated."}
            </p>
        </>
    }
}
