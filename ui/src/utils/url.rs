//! Small helpers for reading and rewriting the current page URL's query
//! string, shared by the Checkout return flows (billing and credit
//! purchases) so both parse and clean the same way.

/// The value of query parameter `name` in the current URL, if present.
pub fn query_param(name: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    param_value(&search, name)
}

/// Remove query parameter `name` from the current URL via
/// `history.replaceState`, preserving every other parameter and the
/// fragment. A reload then won't repeat a one-shot notice, without
/// discarding unrelated navigation state.
pub fn remove_query_param(name: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let location = window.location();
    let search = location.search().unwrap_or_default();
    let hash = location.hash().unwrap_or_default();
    let path = location.pathname().unwrap_or_default();
    let new_search = strip_param(&search, name);
    let url = format!("{path}{new_search}{hash}");
    if let Ok(history) = window.history() {
        let _ = history.replace_state_with_url(
            &wasm_bindgen::JsValue::NULL,
            "",
            Some(&url),
        );
    }
}

/// Value of `name` in a raw `?a=1&b=2` search string (leading `?`
/// optional).
fn param_value(search: &str, name: &str) -> Option<String> {
    search
        .trim_start_matches('?')
        .split('&')
        .filter(|pair| !pair.is_empty())
        .find_map(|pair| {
            let (key, value) = match pair.split_once('=') {
                Some((k, v)) => (k, v),
                None => (pair, ""),
            };
            (key == name).then(|| value.to_string())
        })
}

/// Return a search string (with leading `?`, or empty) equal to
/// `search` with every `name=...` pair removed.
fn strip_param(search: &str, name: &str) -> String {
    let kept: Vec<&str> = search
        .trim_start_matches('?')
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter(|pair| {
            let key = pair.split_once('=').map(|(k, _)| k).unwrap_or(pair);
            key != name
        })
        .collect();
    if kept.is_empty() {
        String::new()
    } else {
        format!("?{}", kept.join("&"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_value_reads_named_param() {
        assert_eq!(
            param_value("?a=1&purchase=success&b=2", "purchase"),
            Some("success".to_string())
        );
        assert_eq!(param_value("?a=1", "purchase"), None);
    }

    #[test]
    fn strip_param_keeps_others_and_drops_target() {
        assert_eq!(
            strip_param("?a=1&purchase=success&b=2", "purchase"),
            "?a=1&b=2"
        );
        assert_eq!(strip_param("?purchase=success", "purchase"), "");
        assert_eq!(strip_param("", "purchase"), "");
    }
}
