//! Markdown editor with preview and image insertion support.

use payloads::{CommunityId, MAX_SITE_DESCRIPTION_LENGTH, SiteImageId};
use web_sys::HtmlTextAreaElement;
use yew::prelude::*;

use crate::get_api_client;

use super::{ImagePicker, MarkdownText};

#[derive(Properties, PartialEq)]
pub struct Props {
    /// The current markdown text (controlled by parent).
    pub text: AttrValue,
    /// Called when the text changes.
    pub on_change: Callback<String>,
    /// Community ID for image selection.
    pub community_id: CommunityId,
    /// Whether the editor is disabled.
    #[prop_or_default]
    pub disabled: bool,
}

#[function_component]
pub fn MarkdownEditor(props: &Props) -> Html {
    let textarea_ref = use_node_ref();
    let show_preview = use_state(|| true);
    let show_image_picker = use_state(|| false);

    // Handle text changes
    let on_input = {
        let on_change = props.on_change.clone();
        Callback::from(move |e: InputEvent| {
            let textarea: HtmlTextAreaElement = e.target_unchecked_into();
            on_change.emit(textarea.value());
        })
    };

    // Toggle preview
    let on_toggle_preview = {
        let show_preview = show_preview.clone();
        Callback::from(move |_| {
            show_preview.set(!*show_preview);
        })
    };

    // Show image picker
    let on_show_image_picker = {
        let show_image_picker = show_image_picker.clone();
        Callback::from(move |_| {
            show_image_picker.set(true);
        })
    };

    // Hide image picker
    let on_hide_image_picker = {
        let show_image_picker = show_image_picker.clone();
        Callback::from(move |_| {
            show_image_picker.set(false);
        })
    };

    // Handle image selection
    let on_select_image = {
        let text = props.text.clone();
        let on_change = props.on_change.clone();
        let textarea_ref = textarea_ref.clone();
        let show_image_picker = show_image_picker.clone();

        Callback::from(move |(image_id, image_name): (SiteImageId, String)| {
            let api_client = get_api_client();
            let image_url = api_client.site_image_url(&image_id);
            let markdown = format!("![{}]({})", image_name, image_url);

            // Get cursor position and insert
            if let Some(textarea) = textarea_ref.cast::<HtmlTextAreaElement>() {
                let start =
                    textarea.selection_start().ok().flatten().unwrap_or(0);
                let end = textarea.selection_end().ok().flatten().unwrap_or(0);
                let current = text.to_string();

                let new_text = format!(
                    "{}{}{}",
                    &current[..start as usize],
                    markdown,
                    &current[end as usize..]
                );

                on_change.emit(new_text.clone());

                // Update textarea value and move cursor after inserted text
                textarea.set_value(&new_text);
                let new_cursor = start + markdown.len() as u32;
                let _ = textarea.set_selection_start(Some(new_cursor));
                let _ = textarea.set_selection_end(Some(new_cursor));
                let _ = textarea.focus();
            }

            show_image_picker.set(false);
        })
    };

    let disabled = props.disabled;

    // Character count for limit warning
    let char_count = props.text.len();
    let warning_threshold = MAX_SITE_DESCRIPTION_LENGTH - 1000;
    let show_char_count = char_count > warning_threshold;
    let is_over_limit = char_count > MAX_SITE_DESCRIPTION_LENGTH;
    let char_count_class = if is_over_limit {
        "text-xs tabular-nums text-red-600 dark:text-red-400"
    } else {
        "text-xs tabular-nums text-neutral-500 dark:text-neutral-400"
    };

    html! {
        <div class="space-y-4">
            // Toolbar
            <div class="flex items-center gap-2 flex-wrap">
                <button
                    type="button"
                    onclick={on_toggle_preview}
                    class={classes!(
                        "px-3", "py-1.5", "text-sm", "font-medium", "rounded-md",
                        "border", "transition-colors",
                        if *show_preview {
                            "bg-neutral-100 dark:bg-neutral-700 border-neutral-300 \
                             dark:border-neutral-600 text-neutral-900 \
                             dark:text-neutral-100"
                        } else {
                            "bg-white dark:bg-neutral-800 border-neutral-300 \
                             dark:border-neutral-600 text-neutral-600 \
                             dark:text-neutral-400 hover:bg-neutral-50 \
                             dark:hover:bg-neutral-700"
                        }
                    )}
                >
                    {if *show_preview { "Hide Preview" } else { "Show Preview" }}
                </button>

                <button
                    type="button"
                    onclick={on_show_image_picker}
                    {disabled}
                    class="px-3 py-1.5 text-sm font-medium rounded-md border
                           bg-white dark:bg-neutral-800 border-neutral-300
                           dark:border-neutral-600 text-neutral-600
                           dark:text-neutral-400 hover:bg-neutral-50
                           dark:hover:bg-neutral-700 transition-colors
                           disabled:opacity-50 disabled:cursor-not-allowed"
                >
                    {"Insert Image"}
                </button>
            </div>

            // Image picker (when visible)
            {if *show_image_picker {
                html! {
                    <ImagePicker
                        community_id={props.community_id}
                        on_select={on_select_image}
                        on_cancel={on_hide_image_picker}
                        {disabled}
                    />
                }
            } else {
                html! {}
            }}

            // Editor area
            <div class={classes!(
                "grid", "gap-4",
                if *show_preview { "md:grid-cols-2" } else { "grid-cols-1" }
            )}>
                // Textarea
                <div class="flex flex-col">
                    <textarea
                        ref={textarea_ref}
                        value={props.text.clone()}
                        oninput={on_input}
                        {disabled}
                        class="w-full h-48 md:h-96 px-3 py-2 border border-neutral-300
                               dark:border-neutral-600 rounded-md shadow-sm
                               bg-white dark:bg-neutral-700 text-neutral-900
                               dark:text-neutral-100 font-mono text-sm
                               focus:outline-none focus:ring-2 focus:ring-neutral-500
                               focus:border-neutral-500 dark:focus:ring-neutral-400
                               dark:focus:border-neutral-400 disabled:opacity-50
                               disabled:cursor-not-allowed resize-y"
                        placeholder="Enter description (supports Markdown)"
                    />
                    <div class="mt-1 flex justify-between items-center">
                        <p class="text-xs text-neutral-500 dark:text-neutral-400">
                            {"Supports Markdown: **bold**, *italic*, [links](url), \
                              ![images](url), lists, and more."}
                        </p>
                        // Show character count when approaching/exceeding limit
                        {if show_char_count {
                            html! {
                                <p class={char_count_class}>
                                    {format!(
                                        "{} / {}",
                                        char_count,
                                        MAX_SITE_DESCRIPTION_LENGTH
                                    )}
                                </p>
                            }
                        } else {
                            html! {}
                        }}
                    </div>
                </div>

                // Preview pane
                {if *show_preview {
                    html! {
                        <div class="border border-neutral-300 dark:border-neutral-600
                                    rounded-md p-4 bg-white dark:bg-neutral-800
                                    overflow-auto h-48 md:h-96">
                            {if props.text.is_empty() {
                                html! {
                                    <p class="text-sm text-neutral-400
                                              dark:text-neutral-500 italic">
                                        {"Preview will appear here..."}
                                    </p>
                                }
                            } else {
                                html! {
                                    <MarkdownText text={props.text.to_string()} />
                                }
                            }}
                        </div>
                    }
                } else {
                    html! {}
                }}
            </div>
        </div>
    }
}
