use payloads::{CommunityId, SiteImageId, requests, responses};
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::{
    components::{
        CommunityImageUpload, CommunityPageWrapper, CommunityTabHeader,
        ConfirmationModal, community_tab_header::ActiveTab,
    },
    get_api_client,
    hooks::use_community_images,
};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
}

#[function_component]
pub fn CommunityImagesPage(props: &Props) -> Html {
    let render_content =
        Callback::from(|community: responses::CommunityWithRole| {
            html! {
                <div>
                    <CommunityTabHeader
                        community={community.clone()}
                        active_tab={ActiveTab::Images}
                    />
                    <div class="py-6">
                        <CommunityImagesContent community={community} />
                    </div>
                </div>
            }
        });

    html! {
        <CommunityPageWrapper
            community_id={props.community_id}
            children={render_content}
        />
    }
}

#[derive(Properties, PartialEq)]
struct ContentProps {
    community: responses::CommunityWithRole,
}

#[function_component]
fn CommunityImagesContent(props: &ContentProps) -> Html {
    let images_hook = use_community_images(props.community.id);
    let success_message = use_state(|| None::<String>);

    // Delete confirmation state
    let delete_target = use_state(|| None::<SiteImageId>);
    let is_deleting = use_state(|| false);
    let delete_error = use_state(|| None::<String>);

    // Handle upload complete
    let on_upload = {
        let refetch = images_hook.refetch.clone();
        let success_message = success_message.clone();

        Callback::from(move |(_image_id, _name): (SiteImageId, String)| {
            success_message
                .set(Some("Image uploaded successfully.".to_string()));
            refetch.emit(());
        })
    };

    // Handle delete click
    let on_delete_click = {
        let delete_target = delete_target.clone();
        Callback::from(move |id: SiteImageId| {
            delete_target.set(Some(id));
        })
    };

    // Handle delete confirm
    let on_delete_confirm = {
        let delete_target = delete_target.clone();
        let is_deleting = is_deleting.clone();
        let delete_error = delete_error.clone();
        let success_message = success_message.clone();
        let refetch = images_hook.refetch.clone();

        Callback::from(move |_| {
            let image_id = match *delete_target {
                Some(id) => id,
                None => return,
            };

            let delete_target = delete_target.clone();
            let is_deleting = is_deleting.clone();
            let delete_error = delete_error.clone();
            let success_message = success_message.clone();
            let refetch = refetch.clone();

            wasm_bindgen_futures::spawn_local(async move {
                is_deleting.set(true);
                delete_error.set(None);

                let api_client = get_api_client();
                match api_client.delete_site_image(&image_id).await {
                    Ok(_) => {
                        delete_target.set(None);
                        success_message.set(Some(
                            "Image deleted successfully.".to_string(),
                        ));
                        refetch.emit(());
                    }
                    Err(e) => {
                        delete_error.set(Some(e.to_string()));
                    }
                }

                is_deleting.set(false);
            });
        })
    };

    // Handle delete cancel
    let on_delete_cancel = {
        let delete_target = delete_target.clone();
        let delete_error = delete_error.clone();
        Callback::from(move |_| {
            delete_target.set(None);
            delete_error.set(None);
        })
    };

    // Handle rename
    let on_rename = {
        let refetch = images_hook.refetch.clone();
        let success_message = success_message.clone();

        Callback::from(move |(image_id, new_name): (SiteImageId, String)| {
            let refetch = refetch.clone();
            let success_message = success_message.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let api_client = get_api_client();
                let request = requests::UpdateSiteImage {
                    id: image_id,
                    name: Some(new_name),
                };

                match api_client.update_site_image(&request).await {
                    Ok(_) => {
                        success_message.set(Some(
                            "Image renamed successfully.".to_string(),
                        ));
                        refetch.emit(());
                    }
                    Err(e) => {
                        // TODO: Could show error in the card itself
                        web_sys::console::error_1(
                            &format!("Failed to rename image: {}", e).into(),
                        );
                    }
                }
            });
        })
    };

    html! {
        <div class="space-y-6">
            <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between
                        gap-4">
                <div>
                    <h2 class="text-xl font-semibold text-neutral-900
                               dark:text-neutral-100">
                        {"Community Images"}
                    </h2>
                    <p class="text-sm text-neutral-600 dark:text-neutral-400 mt-1">
                        {"Manage images that can be used across sites in this \
                         community. This page is only visible to coleaders, and
                         images are only visible to members if attached to a
                         site or space."}
                    </p>
                </div>
            </div>

            // Success message
            {if let Some(success) = &*success_message {
                html! {
                    <div class="p-3 rounded-md bg-green-50 dark:bg-green-900/20
                                border border-green-200 dark:border-green-800">
                        <p class="text-sm text-green-700 dark:text-green-400">
                            {success}
                        </p>
                    </div>
                }
            } else {
                html! {}
            }}

            // Upload section
            <div class="bg-white dark:bg-neutral-800 rounded-lg shadow-md border
                        border-neutral-200 dark:border-neutral-700 p-4">
                <h3 class="text-lg font-medium text-neutral-900 dark:text-neutral-100
                           mb-4">
                    {"Upload New Image"}
                </h3>

                <CommunityImageUpload
                    community_id={props.community.id}
                    on_upload={on_upload}
                />
            </div>

            // Images list
            {images_hook.render("images", |images, _, _| {
                if images.is_empty() {
                    return html! {
                        <div class="text-center py-8 text-neutral-500
                                    dark:text-neutral-400">
                            <p>{"No images uploaded yet."}</p>
                            <p class="text-sm mt-1">
                                {"Upload an image above to get started."}
                            </p>
                        </div>
                    };
                }

                html! {
                    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
                        {images.iter().map(|image| {
                            html! {
                                <ImageCard
                                    key={image.id.0.to_string()}
                                    image={image.clone()}
                                    on_delete={on_delete_click.clone()}
                                    on_rename={on_rename.clone()}
                                />
                            }
                        }).collect::<Html>()}
                    </div>
                }
            })}

            // Delete confirmation modal
            {if let Some(image_id) = *delete_target {
                let image_name = images_hook.data.as_ref()
                    .and_then(|images| {
                        images.iter().find(|i| i.id == image_id)
                    })
                    .map(|i| i.name.clone())
                    .unwrap_or_else(|| "this image".to_string());

                html! {
                    <ConfirmationModal
                        title="Delete Image"
                        message={format!(
                            "Are you sure you want to delete '{}'? If this image \
                             is currently used by any sites, they will no longer \
                             display an image.",
                            image_name
                        )}
                        confirm_text="Delete"
                        on_confirm={on_delete_confirm}
                        on_close={on_delete_cancel}
                        is_loading={*is_deleting}
                        is_irreversible={false}
                        error_message={(*delete_error).clone().map(AttrValue::from)}
                    />
                }
            } else {
                html! {}
            }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct ImageCardProps {
    image: responses::SiteImageInfo,
    on_delete: Callback<SiteImageId>,
    on_rename: Callback<(SiteImageId, String)>,
}

#[function_component]
fn ImageCard(props: &ImageCardProps) -> Html {
    let is_editing = use_state(|| false);
    let name_input = use_state(|| props.image.name.clone());

    let api_client = get_api_client();
    let src = api_client.site_image_url(&props.image.id);

    let on_edit_click = {
        let is_editing = is_editing.clone();
        Callback::from(move |_| {
            is_editing.set(true);
        })
    };

    let on_name_change = {
        let name_input = name_input.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            name_input.set(input.value());
        })
    };

    let on_save = {
        let is_editing = is_editing.clone();
        let name_input = name_input.clone();
        let on_rename = props.on_rename.clone();
        let image_id = props.image.id;
        let original_name = props.image.name.clone();

        Callback::from(move |_| {
            let new_name = (*name_input).trim().to_string();
            if !new_name.is_empty() && new_name != original_name {
                on_rename.emit((image_id, new_name));
            }
            is_editing.set(false);
        })
    };

    let on_cancel = {
        let is_editing = is_editing.clone();
        let name_input = name_input.clone();
        let original_name = props.image.name.clone();

        Callback::from(move |_| {
            name_input.set(original_name.clone());
            is_editing.set(false);
        })
    };

    let on_delete = {
        let on_delete = props.on_delete.clone();
        let image_id = props.image.id;
        Callback::from(move |_| {
            on_delete.emit(image_id);
        })
    };

    html! {
        <div
            class="bg-white dark:bg-neutral-800 rounded-lg shadow border
                   border-neutral-200 dark:border-neutral-700 overflow-hidden"
        >
            <a
                href={src.clone()}
                target="_blank"
                rel="noopener noreferrer"
                class="block aspect-video bg-neutral-100 dark:bg-neutral-700
                       cursor-zoom-in"
            >
                <img
                    {src}
                    alt={props.image.name.clone()}
                    class="w-full h-full object-cover"
                />
            </a>
            <div class="p-3">
                {if *is_editing {
                    html! {
                        <div class="space-y-2">
                            <input
                                type="text"
                                value={(*name_input).clone()}
                                onchange={on_name_change}
                                class="w-full px-2 py-1 text-sm border
                                       border-neutral-300 dark:border-neutral-600
                                       rounded bg-white dark:bg-neutral-700
                                       text-neutral-900 dark:text-neutral-100
                                       focus:outline-none focus:ring-1
                                       focus:ring-neutral-500"
                            />
                            <div class="flex justify-end gap-1">
                                <button
                                    type="button"
                                    onclick={on_cancel}
                                    class="text-sm px-3 py-2 rounded
                                           text-neutral-500
                                           hover:text-neutral-700
                                           dark:text-neutral-400
                                           dark:hover:text-neutral-200
                                           hover:bg-neutral-100
                                           dark:hover:bg-neutral-700"
                                >
                                    {"Cancel"}
                                </button>
                                <button
                                    type="button"
                                    onclick={on_save}
                                    class="text-sm px-3 py-2 rounded
                                           text-neutral-900
                                           hover:text-neutral-700
                                           dark:text-neutral-100
                                           dark:hover:text-neutral-300
                                           font-medium
                                           hover:bg-neutral-100
                                           dark:hover:bg-neutral-700"
                                >
                                    {"Save"}
                                </button>
                            </div>
                        </div>
                    }
                } else {
                    html! {
                        <>
                            <p class="text-sm font-medium text-neutral-900
                                      dark:text-neutral-100 truncate">
                                {&props.image.name}
                            </p>
                            <div class="mt-2 flex justify-end gap-1">
                                <button
                                    type="button"
                                    onclick={on_edit_click}
                                    class="text-sm px-3 py-2 rounded
                                           text-neutral-600 hover:text-neutral-800
                                           hover:bg-neutral-100
                                           dark:text-neutral-400
                                           dark:hover:text-neutral-200
                                           dark:hover:bg-neutral-700"
                                >
                                    {"Rename"}
                                </button>
                                <button
                                    type="button"
                                    onclick={on_delete}
                                    class="text-sm px-3 py-2 rounded
                                           text-red-600 hover:text-red-800
                                           hover:bg-red-50
                                           dark:text-red-400 dark:hover:text-red-300
                                           dark:hover:bg-red-900/20"
                                >
                                    {"Delete"}
                                </button>
                            </div>
                        </>
                    }
                }}
            </div>
        </div>
    }
}
