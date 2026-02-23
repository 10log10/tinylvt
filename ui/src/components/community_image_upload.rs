use base64::{Engine as _, engine::general_purpose};
use payloads::{CommunityId, MAX_IMAGE_SIZE, SiteImageId, requests};
use wasm_bindgen::prelude::*;
use web_sys::{Event, FileReader, HtmlInputElement};
use yew::prelude::*;

use crate::get_api_client;

#[derive(Clone, PartialEq)]
struct PendingUpload {
    data: Vec<u8>,
    preview_url: String,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    /// Called when an image is successfully uploaded. Passes the image ID and
    /// name.
    pub on_upload: Callback<(SiteImageId, String)>,
    #[prop_or_default]
    pub disabled: bool,
    /// Optional custom label for the upload area.
    #[prop_or_default]
    pub label: Option<String>,
    /// Whether to show a compact version (smaller preview).
    #[prop_or_default]
    pub compact: bool,
    /// Whether to render as a small tile (for image picker grids).
    #[prop_or_default]
    pub tile: bool,
}

/// A reusable component for uploading images to a community.
/// Handles file selection, validation, preview, naming, and API upload.
#[function_component]
pub fn CommunityImageUpload(props: &Props) -> Html {
    let file_input_ref = use_node_ref();
    let pending_upload = use_state(|| None::<PendingUpload>);
    let name_value = use_state(String::new);
    let is_uploading = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    // Handle file selection
    let on_file_select = {
        let pending_upload = pending_upload.clone();
        let name_value = name_value.clone();
        let error_message = error_message.clone();

        Callback::from(move |e: Event| {
            let pending_upload = pending_upload.clone();
            let name_value = name_value.clone();
            let error_message = error_message.clone();

            let input: HtmlInputElement = e.target_unchecked_into();
            let files = match input.files() {
                Some(f) => f,
                None => return,
            };

            let file = match files.get(0) {
                Some(f) => f,
                None => return,
            };

            // Validate file size
            let file_size = file.size() as usize;
            if file_size > MAX_IMAGE_SIZE {
                error_message.set(Some(format!(
                    "File is too large ({:.1}MB). Maximum size is 1MB.",
                    file_size as f64 / 1_048_576.0
                )));
                return;
            }

            // Get filename for default name
            let filename = file.name();
            let default_name = filename
                .rsplit_once('.')
                .map(|(name, _)| name.to_string())
                .unwrap_or(filename.clone());

            // Read file as array buffer
            let reader = FileReader::new().unwrap();
            let reader_clone = reader.clone();

            let onload = Closure::wrap(Box::new(move |_: Event| {
                let result = reader_clone.result().unwrap();
                let array = js_sys::Uint8Array::new(&result);
                let data: Vec<u8> = array.to_vec();

                let base64_data = general_purpose::STANDARD.encode(&data);
                let preview_url =
                    format!("data:image/jpeg;base64,{}", base64_data);

                pending_upload.set(Some(PendingUpload { data, preview_url }));
                name_value.set(default_name.clone());
                error_message.set(None);
            }) as Box<dyn FnMut(_)>);

            reader.set_onload(Some(onload.as_ref().unchecked_ref()));
            reader.read_as_array_buffer(&file).unwrap();
            onload.forget();
        })
    };

    // Handle name input change
    let on_name_change = {
        let name_value = name_value.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            name_value.set(input.value());
        })
    };

    // Handle upload
    let on_upload = {
        let pending_upload = pending_upload.clone();
        let name_value = name_value.clone();
        let is_uploading = is_uploading.clone();
        let error_message = error_message.clone();
        let on_upload_callback = props.on_upload.clone();
        let community_id = props.community_id;
        let file_input_ref = file_input_ref.clone();

        Callback::from(move |_| {
            let upload = match (*pending_upload).clone() {
                Some(u) => u,
                None => return,
            };

            let name = (*name_value).trim().to_string();
            if name.is_empty() {
                error_message.set(Some(
                    "Please enter a name for the image.".to_string(),
                ));
                return;
            }

            let pending_upload = pending_upload.clone();
            let is_uploading = is_uploading.clone();
            let error_message = error_message.clone();
            let on_upload_callback = on_upload_callback.clone();
            let file_input_ref = file_input_ref.clone();
            let uploaded_name = name.clone();

            wasm_bindgen_futures::spawn_local(async move {
                is_uploading.set(true);
                error_message.set(None);

                let api_client = get_api_client();
                let request = requests::CreateSiteImage {
                    community_id,
                    name,
                    image_data: upload.data,
                };

                match api_client.create_site_image(&request).await {
                    Ok(image_id) => {
                        pending_upload.set(None);
                        // Clear file input
                        if let Some(input) =
                            file_input_ref.cast::<HtmlInputElement>()
                        {
                            input.set_value("");
                        }
                        on_upload_callback.emit((image_id, uploaded_name));
                    }
                    Err(e) => {
                        error_message.set(Some(e.to_string()));
                    }
                }

                is_uploading.set(false);
            });
        })
    };

    // Handle cancel
    let on_cancel = {
        let pending_upload = pending_upload.clone();
        let error_message = error_message.clone();
        let file_input_ref = file_input_ref.clone();

        Callback::from(move |_| {
            pending_upload.set(None);
            error_message.set(None);
            if let Some(input) = file_input_ref.cast::<HtmlInputElement>() {
                input.set_value("");
            }
        })
    };

    // Trigger file input
    let on_select_file = {
        let file_input_ref = file_input_ref.clone();
        Callback::from(move |_| {
            if let Some(input) = file_input_ref.cast::<HtmlInputElement>() {
                input.click();
            }
        })
    };

    let disabled = props.disabled || *is_uploading;
    let preview_size = if props.compact || props.tile {
        "w-24 h-16"
    } else {
        "w-32 h-20"
    };

    // Tile mode: render as a compact grid tile
    if props.tile {
        return html! {
            <>
                // Hidden file input
                <input
                    ref={file_input_ref}
                    type="file"
                    accept="image/*"
                    onchange={on_file_select}
                    class="hidden"
                    disabled={disabled}
                />

                {if let Some(upload) = &*pending_upload {
                    // Show pending upload in tile - preview with name input below
                    html! {
                        <div class="space-y-2">
                            <div class="aspect-video rounded overflow-hidden
                                        bg-neutral-100 dark:bg-neutral-700">
                                <img
                                    src={upload.preview_url.clone()}
                                    alt="Preview"
                                    class="w-full h-full object-cover"
                                />
                            </div>
                            <input
                                type="text"
                                value={(*name_value).clone()}
                                onchange={on_name_change}
                                disabled={disabled}
                                placeholder="Name"
                                class="w-full px-2 py-1 text-xs border
                                       border-neutral-300 dark:border-neutral-600
                                       rounded bg-white dark:bg-neutral-700
                                       text-neutral-900 dark:text-neutral-100
                                       focus:outline-none focus:ring-1
                                       focus:ring-neutral-500"
                            />
                            // Error message
                            {if let Some(error) = &*error_message {
                                html! {
                                    <p class="text-xs text-red-600
                                              dark:text-red-400 truncate">
                                        {error}
                                    </p>
                                }
                            } else {
                                html! {}
                            }}
                            <div class="flex gap-1">
                                <button
                                    type="button"
                                    onclick={on_upload}
                                    disabled={disabled}
                                    class="flex-1 px-2 py-1 text-xs font-medium
                                           text-white bg-neutral-900
                                           hover:bg-neutral-800
                                           dark:bg-neutral-100
                                           dark:text-neutral-900
                                           dark:hover:bg-neutral-200 rounded
                                           disabled:opacity-50"
                                >
                                    {if *is_uploading { "..." } else { "Upload" }}
                                </button>
                                <button
                                    type="button"
                                    onclick={on_cancel}
                                    disabled={*is_uploading}
                                    class="px-2 py-1 text-xs text-neutral-500
                                           hover:text-neutral-700
                                           dark:hover:text-neutral-300
                                           hover:bg-neutral-100
                                           dark:hover:bg-neutral-700 rounded"
                                >
                                    {"X"}
                                </button>
                            </div>
                        </div>
                    }
                } else {
                    // Show upload tile button
                    html! {
                        <button
                            type="button"
                            onclick={on_select_file}
                            disabled={disabled}
                            class="aspect-video rounded border-2 border-dashed
                                   border-neutral-300 dark:border-neutral-600
                                   hover:border-neutral-400 dark:hover:border-neutral-500
                                   focus:border-neutral-500 dark:focus:border-neutral-400
                                   focus:outline-none transition-colors
                                   flex items-center justify-center
                                   disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                            <div class="text-center">
                                <span class="text-2xl text-neutral-400
                                             dark:text-neutral-500">
                                    {"+"}
                                </span>
                                <p class="text-xs text-neutral-500
                                          dark:text-neutral-400 mt-1">
                                    {"Upload"}
                                </p>
                            </div>
                        </button>
                    }
                }}
            </>
        };
    }

    // Standard mode
    html! {
        <div class="space-y-3">
            // Hidden file input
            <input
                ref={file_input_ref}
                type="file"
                accept="image/*"
                onchange={on_file_select}
                class="hidden"
                disabled={disabled}
            />

            // Error message
            {if let Some(error) = &*error_message {
                html! {
                    <div class="p-3 rounded-md bg-red-50 dark:bg-red-900/20 border
                                border-red-200 dark:border-red-800">
                        <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
                    </div>
                }
            } else {
                html! {}
            }}

            {if let Some(upload) = &*pending_upload {
                // Show pending upload preview
                html! {
                    <div class="flex items-start gap-4">
                        <div class={classes!(
                            preview_size,
                            "rounded-md", "overflow-hidden",
                            "bg-neutral-100", "dark:bg-neutral-700",
                            "flex-shrink-0"
                        )}>
                            <img
                                src={upload.preview_url.clone()}
                                alt="Preview"
                                class="w-full h-full object-cover"
                            />
                        </div>
                        <div class="flex-1 min-w-0 space-y-2">
                            <input
                                type="text"
                                value={(*name_value).clone()}
                                onchange={on_name_change}
                                disabled={disabled}
                                placeholder="Image name"
                                class="w-full px-3 py-2 border border-neutral-300
                                       dark:border-neutral-600 rounded-md shadow-sm
                                       bg-white dark:bg-neutral-700 text-neutral-900
                                       dark:text-neutral-100 text-sm
                                       focus:outline-none focus:ring-2
                                       focus:ring-neutral-500 focus:border-neutral-500
                                       dark:focus:ring-neutral-400
                                       disabled:opacity-50"
                            />
                            <div class="flex gap-2">
                                <button
                                    type="button"
                                    onclick={on_upload}
                                    disabled={disabled}
                                    class="px-3 py-1.5 text-sm font-medium text-white
                                           bg-neutral-900 hover:bg-neutral-800
                                           dark:bg-neutral-100 dark:text-neutral-900
                                           dark:hover:bg-neutral-200 rounded-md
                                           disabled:opacity-50"
                                >
                                    {if *is_uploading {
                                        "Uploading..."
                                    } else {
                                        "Upload"
                                    }}
                                </button>
                                <button
                                    type="button"
                                    onclick={on_cancel}
                                    disabled={*is_uploading}
                                    class="px-3 py-2 text-sm font-medium rounded
                                           text-neutral-600 dark:text-neutral-400
                                           hover:text-neutral-800
                                           dark:hover:text-neutral-200
                                           hover:bg-neutral-100
                                           dark:hover:bg-neutral-700"
                                >
                                    {"Cancel"}
                                </button>
                            </div>
                        </div>
                    </div>
                }
            } else {
                // Show upload button
                html! {
                    <button
                        type="button"
                        onclick={on_select_file}
                        disabled={disabled}
                        class="w-full px-4 py-4 border-2 border-dashed
                               border-neutral-300 dark:border-neutral-600
                               rounded-lg text-center hover:border-neutral-400
                               dark:hover:border-neutral-500 transition-colors
                               cursor-pointer disabled:opacity-50"
                    >
                        <p class="text-sm text-neutral-600 dark:text-neutral-400">
                            {props.label.as_deref()
                                .unwrap_or("Click to select an image")}
                        </p>
                        <p class="text-xs text-neutral-500 mt-1">
                            {"Maximum file size: 1MB"}
                        </p>
                    </button>
                }
            }}
        </div>
    }
}
