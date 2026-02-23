use payloads::{CommunityId, SiteImageId};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::use_community_images;

use super::ImagePicker;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    pub current_image_id: Option<SiteImageId>,
    pub on_change: Callback<Option<SiteImageId>>,
    #[prop_or_default]
    pub disabled: bool,
    /// Use fewer grid columns for narrow layouts (e.g., modals, cards).
    #[prop_or_default]
    pub compact: bool,
}

/// Component for selecting or changing a site's associated image.
/// Allows selecting from existing community images or uploading a new one.
#[function_component]
pub fn SiteImageSelector(props: &Props) -> Html {
    let show_picker = use_state(|| false);
    let community_images_hook = use_community_images(props.community_id);

    // Look up current image name from the community images list
    let current_image_name: Option<String> =
        props.current_image_id.and_then(|id| {
            community_images_hook.data.as_ref().and_then(|images| {
                images
                    .iter()
                    .find(|img| img.id == id)
                    .map(|img| img.name.clone())
            })
        });

    // Show image picker
    let on_show_picker = {
        let show_picker = show_picker.clone();
        Callback::from(move |_| {
            show_picker.set(true);
        })
    };

    // Hide image picker
    let on_hide_picker = {
        let show_picker = show_picker.clone();
        Callback::from(move |_| {
            show_picker.set(false);
        })
    };

    // Handle image selection from picker
    let on_select_image = {
        let on_change = props.on_change.clone();
        let show_picker = show_picker.clone();
        let refetch = community_images_hook.refetch.clone();

        Callback::from(move |(image_id, _name): (SiteImageId, String)| {
            on_change.emit(Some(image_id));
            show_picker.set(false);
            refetch.emit(());
        })
    };

    // Handle remove current image
    let on_remove = {
        let on_change = props.on_change.clone();
        Callback::from(move |_| {
            on_change.emit(None);
        })
    };

    let disabled = props.disabled;

    html! {
        <div class="space-y-4">
            {if *show_picker {
                // Show the unified image picker
                html! {
                    <ImagePicker
                        community_id={props.community_id}
                        on_select={on_select_image}
                        on_cancel={on_hide_picker}
                        disabled={disabled}
                        compact={props.compact}
                    />
                }
            } else if let Some(image_id) = props.current_image_id {
                // Show current image with Change/Remove buttons
                let api_client = get_api_client();
                let src = api_client.site_image_url(&image_id);

                html! {
                    <div class="space-y-3">
                        <div class="flex items-start gap-4">
                            <div class="w-32 h-20 rounded-md overflow-hidden
                                        bg-neutral-100 dark:bg-neutral-700
                                        flex-shrink-0">
                                <img
                                    src={src}
                                    alt={current_image_name.clone().unwrap_or_default()}
                                    class="w-full h-full object-cover"
                                />
                            </div>
                            <div class="flex-1 min-w-0">
                                {if let Some(name) = &current_image_name {
                                    html! {
                                        <p class="text-sm font-medium text-neutral-900
                                                  dark:text-neutral-100 truncate">
                                            {name}
                                        </p>
                                    }
                                } else {
                                    html! {}
                                }}
                                <div class={if current_image_name.is_some() { "mt-2 flex flex-wrap gap-1" } else { "flex flex-wrap gap-1" }}>
                                    <button
                                        type="button"
                                        onclick={on_show_picker.clone()}
                                        disabled={disabled}
                                        class="text-sm px-3 py-2 rounded
                                               text-neutral-600 hover:text-neutral-800
                                               hover:bg-neutral-100
                                               dark:text-neutral-400
                                               dark:hover:text-neutral-200
                                               dark:hover:bg-neutral-700
                                               disabled:opacity-50
                                               disabled:cursor-not-allowed"
                                    >
                                        {"Change"}
                                    </button>
                                    <button
                                        type="button"
                                        onclick={on_remove}
                                        disabled={disabled}
                                        class="text-sm px-3 py-2 rounded
                                               text-red-600 hover:text-red-800
                                               hover:bg-red-50
                                               dark:text-red-400 dark:hover:text-red-300
                                               dark:hover:bg-red-900/20
                                               disabled:opacity-50
                                               disabled:cursor-not-allowed"
                                    >
                                        {"Remove"}
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                }
            } else {
                // No image - show button to open picker
                html! {
                    <button
                        type="button"
                        onclick={on_show_picker}
                        disabled={disabled}
                        class="w-full px-4 py-3 border-2 border-dashed
                               border-neutral-300 dark:border-neutral-600
                               rounded-lg text-center hover:border-neutral-400
                               dark:hover:border-neutral-500 transition-colors
                               cursor-pointer disabled:opacity-50
                               disabled:cursor-not-allowed"
                    >
                        <p class="text-sm text-neutral-600 dark:text-neutral-400">
                            {"Select or upload an image"}
                        </p>
                        <p class="text-xs text-neutral-500 dark:text-neutral-500 mt-1">
                            {"Max 1MB"}
                        </p>
                    </button>
                }
            }}
        </div>
    }
}
