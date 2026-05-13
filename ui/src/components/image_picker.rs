//! Unified image picker component for selecting or uploading community images.

use payloads::{CommunityId, SiteImageId};
use yew::prelude::*;

use crate::{
    get_api_client,
    hooks::{Fetch, use_community_images},
};

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

    // Upload completing produces the same (id, name) tuple shape that
    // clicking an existing image will produce, so both flows share a
    // refetch + on_select handler.
    let on_image_chosen = {
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
                images={images_hook.inner.clone()}
                on_image_chosen={on_image_chosen}
                disabled={props.disabled}
                compact={props.compact}
            />
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct ImageGridProps {
    community_id: CommunityId,
    images: Fetch<Vec<payloads::responses::SiteImageInfo>>,
    /// Fires when the user picks any image — either an existing one (looked
    /// up at the per-image button site, where the name is already known) or
    /// a freshly uploaded one. The grid never has to extract data from the
    /// `images` Fetch outside of `render`.
    on_image_chosen: Callback<(SiteImageId, String)>,
    disabled: bool,
    compact: bool,
}

#[function_component]
fn ImageGrid(props: &ImageGridProps) -> Html {
    let grid_class = if props.compact {
        "grid grid-cols-2 sm:grid-cols-3 gap-2 max-h-64 overflow-y-auto"
    } else {
        "grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-2 max-h-64 overflow-y-auto"
    };

    let render_grid_with_images =
        |images: &[payloads::responses::SiteImageInfo]| {
            html! {
                <div class={grid_class}>
                    // Upload tile (always first)
                    <CommunityImageUpload
                        community_id={props.community_id}
                        on_upload={props.on_image_chosen.clone()}
                        disabled={props.disabled}
                        tile=true
                    />

                    {images.iter().map(|image| {
                        let image_id = image.id;
                        let image_name = image.name.clone();
                        let on_chosen = props.on_image_chosen.clone();
                        let src = get_api_client().site_image_url(&image.id);

                        html! {
                            <button
                                key={image.id.0.to_string()}
                                type="button"
                                onclick={Callback::from(move |_| {
                                    on_chosen.emit((image_id, image_name.clone()));
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
                    }).collect::<Html>()}
                </div>
            }
        };

    props.images.render(
        |images, _is_loading, _errors| render_grid_with_images(images),
        || {
            html! {
                <p class="text-sm text-neutral-500 dark:text-neutral-400 py-4
                          text-center">
                    {"Loading images..."}
                </p>
            }
        },
        |errors: &[String]| {
            html! {
                <div class="space-y-2">
                    {for errors.iter().map(|err| html! {
                        <p class="text-sm text-red-600 dark:text-red-400 py-4
                                  text-center">
                            {err}
                        </p>
                    })}
                </div>
            }
        },
    )
}
