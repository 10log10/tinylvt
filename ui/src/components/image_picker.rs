//! Unified image picker component for selecting or uploading community images.

use payloads::{CommunityId, SiteImageId};
use yew::prelude::*;

use crate::{get_api_client, hooks::use_community_images};

use super::CommunityImageUpload;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    /// Called when an image is selected (either existing or newly uploaded).
    /// Passes (image_id, image_name).
    pub on_select: Callback<(SiteImageId, String)>,
    /// Called when the picker is cancelled/closed.
    pub on_cancel: Callback<()>,
    #[prop_or_default]
    pub disabled: bool,
    /// Use fewer grid columns for narrow layouts (e.g., modals, cards).
    #[prop_or_default]
    pub compact: bool,
}

#[function_component]
pub fn ImagePicker(props: &Props) -> Html {
    let images_hook = use_community_images(props.community_id);

    // Handle clicking an existing image
    let on_image_click = {
        let on_select = props.on_select.clone();
        let images_data = images_hook.data.clone();

        Callback::from(move |image_id: SiteImageId| {
            if let Some(images) = images_data.as_ref()
                && let Some(image) = images.iter().find(|i| i.id == image_id)
            {
                on_select.emit((image_id, image.name.clone()));
            }
        })
    };

    // Handle upload completing
    let on_upload_complete = {
        let on_select = props.on_select.clone();
        let refetch_images = images_hook.refetch.clone();

        Callback::from(move |(image_id, image_name): (SiteImageId, String)| {
            refetch_images.emit(());
            on_select.emit((image_id, image_name));
        })
    };

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 rounded-lg
                    p-4 bg-neutral-50 dark:bg-neutral-800/50">
            <div class="flex items-center justify-between mb-3">
                <h4 class="text-sm font-medium text-neutral-900 dark:text-neutral-100">
                    {"Select an image"}
                </h4>
                <button
                    type="button"
                    onclick={props.on_cancel.reform(|_| ())}
                    class="text-sm px-3 py-2 rounded
                           text-neutral-500 dark:text-neutral-400
                           hover:text-neutral-700 dark:hover:text-neutral-200
                           hover:bg-neutral-100 dark:hover:bg-neutral-700"
                >
                    {"Cancel"}
                </button>
            </div>

            <ImageGrid
                community_id={props.community_id}
                images={images_hook.data.as_ref().cloned()}
                is_loading={images_hook.is_loading}
                on_image_click={on_image_click}
                on_upload={on_upload_complete}
                disabled={props.disabled}
                compact={props.compact}
            />
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct ImageGridProps {
    community_id: CommunityId,
    images: Option<Vec<payloads::responses::SiteImageInfo>>,
    is_loading: bool,
    on_image_click: Callback<SiteImageId>,
    on_upload: Callback<(SiteImageId, String)>,
    disabled: bool,
    compact: bool,
}

#[function_component]
fn ImageGrid(props: &ImageGridProps) -> Html {
    if props.is_loading && props.images.is_none() {
        return html! {
            <p class="text-sm text-neutral-500 dark:text-neutral-400 py-4
                      text-center">
                {"Loading images..."}
            </p>
        };
    }

    let images = props.images.as_ref();

    let grid_class = if props.compact {
        "grid grid-cols-2 sm:grid-cols-3 gap-2 max-h-64 overflow-y-auto"
    } else {
        "grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-2 max-h-64 overflow-y-auto"
    };

    html! {
        <div class={grid_class}>
            // Upload tile (always first) - uses CommunityImageUpload in tile mode
            <CommunityImageUpload
                community_id={props.community_id}
                on_upload={props.on_upload.clone()}
                disabled={props.disabled}
                tile=true
            />

            // Existing images
            {if let Some(images) = images {
                images.iter().map(|image| {
                    let image_id = image.id;
                    let on_click = props.on_image_click.clone();
                    let src = get_api_client().site_image_url(&image.id);

                    html! {
                        <button
                            key={image.id.0.to_string()}
                            type="button"
                            onclick={Callback::from(move |_| {
                                on_click.emit(image_id);
                            })}
                            title={image.name.clone()}
                            disabled={props.disabled}
                            class="aspect-video rounded overflow-hidden
                                   border-2 border-transparent
                                   hover:border-neutral-400
                                   dark:hover:border-neutral-500
                                   focus:border-neutral-500
                                   dark:focus:border-neutral-400
                                   focus:outline-none transition-colors
                                   disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                            <img
                                {src}
                                alt={image.name.clone()}
                                class="w-full h-full object-cover"
                            />
                        </button>
                    }
                }).collect::<Html>()
            } else {
                html! {}
            }}
        </div>
    }
}
