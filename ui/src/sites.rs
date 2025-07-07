use base64::Engine;
use payloads::{CommunityId, SiteImageId, requests, responses};
use web_sys::{
    File, HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement,
    KeyboardEvent, MouseEvent, window,
};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::{Route, auth::use_auth, get_api_client};

// ============================================================================
// Sites List Component
// ============================================================================

#[derive(Default, Clone)]
pub struct SitesState {
    pub sites: Vec<responses::Site>,
    pub site_images: Vec<responses::SiteImage>,
    pub is_loading: bool,
    pub error: Option<String>,
}

#[derive(Properties, PartialEq)]
pub struct SitesProps {
    pub community_id: String,
}

#[function_component]
pub fn Sites(props: &SitesProps) -> Html {
    let (auth_state, _) = use_auth();
    let navigator = use_navigator().unwrap();
    let sites_state = use_state(SitesState::default);

    // Parse community ID
    let community_id = match props.community_id.parse::<uuid::Uuid>() {
        Ok(id) => CommunityId(id),
        Err(_) => {
            navigator.push(&Route::Communities);
            return html! {};
        }
    };

    // Redirect if not authenticated
    {
        let navigator = navigator.clone();
        let auth_state = auth_state.clone();
        use_effect_with(auth_state, move |auth_state| {
            if !auth_state.is_loading && !auth_state.is_authenticated {
                navigator.push(&Route::Login);
            }
            || ()
        });
    }

    // Load sites when component mounts
    {
        let sites_state = sites_state.clone();
        let auth_state = auth_state.clone();
        let community_id = community_id.clone();

        use_effect_with(
            (auth_state.is_authenticated, props.community_id.clone()),
            move |(is_authenticated, _)| {
                if *is_authenticated {
                    let sites_state = sites_state.clone();
                    let community_id = community_id.clone();

                    yew::platform::spawn_local(async move {
                        let mut state = (*sites_state).clone();
                        state.is_loading = true;
                        state.error = None;
                        sites_state.set(state);

                        let client = get_api_client();

                        // Load both sites and site images for this community
                        let sites_future = client.list_sites(&community_id);
                        let images_future =
                            client.list_site_images(&community_id);

                        match futures::future::try_join(
                            sites_future,
                            images_future,
                        )
                        .await
                        {
                            Ok((sites, images)) => {
                                let mut state = (*sites_state).clone();
                                state.sites = sites;
                                state.site_images = images;
                                state.is_loading = false;
                                sites_state.set(state);
                            }
                            Err(e) => {
                                let mut state = (*sites_state).clone();
                                state.error =
                                    Some(format!("Failed to load data: {}", e));
                                state.is_loading = false;
                                sites_state.set(state);
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    let on_create_site = {
        let navigator = navigator.clone();
        let community_id = props.community_id.clone();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::CreateSite {
                community_id: community_id.clone(),
            });
        })
    };

    let on_back_to_community = {
        let navigator = navigator.clone();
        let community_id = props.community_id.clone();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::CommunityDashboard {
                id: community_id.clone(),
            });
        })
    };

    // Don't render anything if not authenticated
    if !auth_state.is_authenticated {
        return html! {};
    }

    html! {
        <main class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
            <div class="space-y-6">
                // Header section
                <div class="space-y-4 sm:space-y-0 sm:flex sm:justify-between sm:items-start">
                    <div class="min-w-0 flex-1">
                        <nav class="flex" aria-label="Breadcrumb">
                            <ol role="list" class="flex items-center space-x-4">
                                <li>
                                    <div class="flex">
                                        <Link<Route>
                                            to={Route::Communities}
                                            classes="text-sm font-medium text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300"
                                        >
                                            {"Communities"}
                                        </Link<Route>>
                                    </div>
                                </li>
                                <li>
                                    <div class="flex items-center">
                                        <svg class="flex-shrink-0 h-5 w-5 text-gray-300" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                            <path d="M5.555 17.776l8-16 .894.448-8 16-.894-.448z" />
                                        </svg>
                                        <button
                                            onclick={on_back_to_community}
                                            class="ml-4 text-sm font-medium text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300"
                                        >
                                            {"Community"}
                                        </button>
                                    </div>
                                </li>
                                <li>
                                    <div class="flex items-center">
                                        <svg class="flex-shrink-0 h-5 w-5 text-gray-300" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                            <path d="M5.555 17.776l8-16 .894.448-8 16-.894-.448z" />
                                        </svg>
                                        <span class="ml-4 text-sm font-medium text-gray-500 dark:text-gray-400">{"Sites"}</span>
                                    </div>
                                </li>
                            </ol>
                        </nav>
                        <h1 class="mt-4 text-2xl sm:text-3xl font-bold text-gray-900 dark:text-white">{"Sites"}</h1>
                        <p class="mt-1 sm:mt-2 text-sm sm:text-base text-gray-600 dark:text-gray-300">
                            {"Manage sites and their associated spaces"}
                        </p>
                    </div>
                    <div class="flex flex-col sm:flex-row gap-3 sm:space-x-3 sm:gap-0 flex-shrink-0">
                        <button
                            onclick={on_create_site.clone()}
                            class="inline-flex items-center justify-center px-4 py-2 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                        >
                            <svg class="-ml-1 mr-2 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
                            </svg>
                            {"Create Site"}
                        </button>
                    </div>
                </div>

                // Error message
                if let Some(error) = &sites_state.error {
                    <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded">
                        {error}
                    </div>
                }

                // Loading state
                if sites_state.is_loading {
                    <div class="text-center py-8">
                        <svg class="animate-spin h-8 w-8 text-blue-600 mx-auto" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                        </svg>
                        <p class="mt-2 text-gray-600 dark:text-gray-400">{"Loading sites..."}</p>
                    </div>
                } else if sites_state.sites.is_empty() {
                    // Empty state
                    <div class="text-center py-12">
                        <div class="mx-auto h-12 w-12 text-gray-400">
                            <svg fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4"></path>
                            </svg>
                        </div>
                        <h3 class="mt-2 text-sm font-medium text-gray-900 dark:text-white">{"No sites"}</h3>
                        <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
                            {"Get started by creating your first site."}
                        </p>
                        <div class="mt-6">
                            <button
                                onclick={on_create_site}
                                class="inline-flex items-center justify-center px-4 py-2 border border-transparent shadow-sm text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                            >
                                {"Create your first site"}
                            </button>
                        </div>
                    </div>
                } else {
                    // Sites grid
                    <div class="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
                        {for sites_state.sites.iter().map(|site| {
                            html! {
                                <SiteCard
                                    key={format!("{}", site.site_id.0)}
                                    site={site.clone()}
                                />
                            }
                        })}
                    </div>
                }

                // Site Images Section
                if !sites_state.site_images.is_empty() {
                    <div class="mt-12">
                        <h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">{"Site Images"}</h2>
                        <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                            {for sites_state.site_images.iter().map(|image| {
                                let sites_state_renamed = sites_state.clone();
                                let sites_state_deleted = sites_state.clone();
                                let community_id_renamed = community_id.clone();
                                let community_id_deleted = community_id.clone();

                                let on_renamed = Callback::from(move |(_image_id, _new_name): (SiteImageId, String)| {
                                    let sites_state_renamed = sites_state_renamed.clone();
                                    let community_id = community_id_renamed.clone();

                                    // Refresh site images from server to get updated data
                                    yew::platform::spawn_local(async move {
                                        let client = get_api_client();
                                        match client.list_site_images(&community_id).await {
                                            Ok(updated_images) => {
                                                let mut state = (*sites_state_renamed).clone();
                                                state.site_images = updated_images;
                                                sites_state_renamed.set(state);
                                            }
                                            Err(_) => {
                                                // If refresh fails, we could show an error or just ignore
                                                // For now, we'll just ignore since the rename was successful
                                            }
                                        }
                                    });
                                });

                                let on_deleted = Callback::from(move |image_id: SiteImageId| {
                                    let sites_state_deleted = sites_state_deleted.clone();
                                    let community_id = community_id_deleted.clone();

                                    // Refresh site images from server to get updated data
                                    yew::platform::spawn_local(async move {
                                        let client = get_api_client();
                                        match client.list_site_images(&community_id).await {
                                            Ok(updated_images) => {
                                                let mut state = (*sites_state_deleted).clone();
                                                state.site_images = updated_images;
                                                sites_state_deleted.set(state);
                                            }
                                            Err(_) => {
                                                // If refresh fails, fall back to optimistic removal
                                                let mut state = (*sites_state_deleted).clone();
                                                state.site_images.retain(|img| img.id != image_id);
                                                sites_state_deleted.set(state);
                                            }
                                        }
                                    });
                                });

                                html! {
                                    <SiteImageCard
                                        key={format!("{}", image.id.0)}
                                        image={image.clone()}
                                        on_renamed={Some(on_renamed)}
                                        on_deleted={Some(on_deleted)}
                                    />
                                }
                            })}
                        </div>
                    </div>
                }
            </div>
        </main>
    }
}

// ============================================================================
// Site Card Component
// ============================================================================

#[derive(Properties)]
pub struct SiteCardProps {
    pub site: responses::Site,
}

impl PartialEq for SiteCardProps {
    fn eq(&self, other: &Self) -> bool {
        self.site.site_id == other.site.site_id
    }
}

#[function_component]
pub fn SiteCard(props: &SiteCardProps) -> Html {
    let navigator = use_navigator().unwrap();
    let site = &props.site;

    let on_click = {
        let navigator = navigator.clone();
        let site_id = site.site_id.clone();

        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::SiteDetails {
                id: site_id.0.to_string(),
            });
        })
    };

    let first_letter = site
        .site_details
        .name
        .chars()
        .next()
        .unwrap_or('S')
        .to_uppercase()
        .to_string();

    html! {
        <div
            onclick={on_click}
            class="relative group bg-white dark:bg-gray-800 p-6 rounded-lg border border-gray-200 dark:border-gray-700 cursor-pointer hover:shadow-md hover:border-gray-300 dark:hover:border-gray-600 transition-all duration-200"
        >
            <div class="flex items-center space-x-3">
                <div class="flex-shrink-0">
                    <div class="w-10 h-10 bg-green-500 rounded-lg flex items-center justify-center">
                        <span class="text-white font-medium text-lg">
                            {first_letter}
                        </span>
                    </div>
                </div>
                <div class="min-w-0 flex-1">
                    <h3 class="text-lg font-medium text-gray-900 dark:text-white group-hover:text-green-600 dark:group-hover:text-green-400 transition-colors">
                        {&site.site_details.name}
                    </h3>
                    {if let Some(description) = &site.site_details.description {
                        html! {
                            <p class="text-sm text-gray-500 dark:text-gray-400 line-clamp-2">
                                {description}
                            </p>
                        }
                    } else {
                        html! {
                            <p class="text-sm text-gray-500 dark:text-gray-400">
                                {"No description"}
                            </p>
                        }
                    }}
                </div>
            </div>
        </div>
    }
}

// ============================================================================
// Site Image Card Component
// ============================================================================

#[derive(Properties)]
pub struct SiteImageCardProps {
    pub image: responses::SiteImage,
    pub on_renamed: Option<Callback<(SiteImageId, String)>>,
    pub on_deleted: Option<Callback<SiteImageId>>,
}

impl PartialEq for SiteImageCardProps {
    fn eq(&self, other: &Self) -> bool {
        self.image.id == other.image.id
    }
}

#[function_component]
pub fn SiteImageCard(props: &SiteImageCardProps) -> Html {
    let image = &props.image;
    let is_editing = use_state(|| false);
    let is_deleting = use_state(|| false);
    let new_name = use_state(|| image.name.clone());
    let error = use_state(|| None::<String>);

    // Convert image data to base64 for display
    let image_src = format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&image.image_data)
    );

    let on_edit_click = {
        let is_editing = is_editing.clone();
        let new_name = new_name.clone();
        let image_name = image.name.clone();

        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            new_name.set(image_name.clone());
            is_editing.set(true);
        })
    };

    let on_delete_click = {
        let is_deleting = is_deleting.clone();
        let on_deleted = props.on_deleted.clone();
        let image_id = image.id.clone();

        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            if window().unwrap().confirm_with_message("Are you sure you want to delete this image? This action cannot be undone.").unwrap_or(false) {
                let is_deleting = is_deleting.clone();
                let on_deleted = on_deleted.clone();
                let image_id = image_id.clone();

                is_deleting.set(true);

                yew::platform::spawn_local(async move {
                    let client = get_api_client();
                    match client.delete_site_image(&image_id).await {
                        Ok(_) => {
                            if let Some(callback) = on_deleted {
                                callback.emit(image_id);
                            }
                        }
                        Err(_) => {
                            is_deleting.set(false);
                        }
                    }
                });
            }
        })
    };

    let on_name_change = {
        let new_name = new_name.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            new_name.set(input.value());
        })
    };

    let on_save_click = {
        let is_editing = is_editing.clone();
        let new_name = new_name.clone();
        let error = error.clone();
        let on_renamed = props.on_renamed.clone();
        let image_id = image.id.clone();

        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            let is_editing = is_editing.clone();
            let new_name = new_name.clone();
            let error = error.clone();
            let on_renamed = on_renamed.clone();
            let image_id = image_id.clone();
            let name = (*new_name).clone();

            if name.trim().is_empty() {
                error.set(Some("Name cannot be empty".to_string()));
                return;
            }

            yew::platform::spawn_local(async move {
                let client = get_api_client();
                let update_request = payloads::requests::UpdateSiteImage {
                    id: image_id,
                    name: Some(name.clone()),
                    image_data: None,
                };

                match client.update_site_image(&update_request).await {
                    Ok(_) => {
                        is_editing.set(false);
                        error.set(None);
                        if let Some(callback) = on_renamed {
                            callback.emit((image_id, name));
                        }
                    }
                    Err(_) => {
                        error.set(Some("Failed to rename image".to_string()));
                    }
                }
            });
        })
    };

    let on_cancel_click = {
        let is_editing = is_editing.clone();
        let error = error.clone();
        let new_name = new_name.clone();
        let original_name = image.name.clone();

        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            is_editing.set(false);
            error.set(None);
            new_name.set(original_name.clone());
        })
    };

    let on_key_down = {
        let on_save_click = on_save_click.clone();
        let on_cancel_click = on_cancel_click.clone();

        Callback::from(move |e: KeyboardEvent| match e.key().as_str() {
            "Enter" => {
                e.prevent_default();
                on_save_click.emit(MouseEvent::new("click").unwrap());
            }
            "Escape" => {
                e.prevent_default();
                on_cancel_click.emit(MouseEvent::new("click").unwrap());
            }
            _ => {}
        })
    };

    html! {
        <div class="relative group bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
            <div class="aspect-w-16 aspect-h-9">
                <img
                    src={image_src}
                    alt={image.name.clone()}
                    class="w-full h-32 object-cover"
                />

                // Hover overlay with action buttons
                if !*is_editing && !*is_deleting {
                    <div class="absolute inset-0 bg-black bg-opacity-50 opacity-0 group-hover:opacity-100 transition-opacity duration-200 flex items-center justify-center space-x-2">
                        <button
                            onclick={on_edit_click}
                            class="p-2 bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-300 rounded-full hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
                            title="Rename image"
                        >
                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                            </svg>
                        </button>
                        <button
                            onclick={on_delete_click}
                            class="p-2 bg-white dark:bg-gray-800 text-red-600 dark:text-red-400 rounded-full hover:bg-red-50 dark:hover:bg-red-900 transition-colors"
                            title="Delete image"
                        >
                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                            </svg>
                        </button>
                    </div>
                }

                // Loading overlay for deletion
                if *is_deleting {
                    <div class="absolute inset-0 bg-black bg-opacity-50 flex items-center justify-center">
                        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-white"></div>
                    </div>
                }
            </div>

            <div class="p-3">
                if *is_editing {
                    <div class="space-y-2">
                        <input
                            type="text"
                            value={(*new_name).clone()}
                            oninput={on_name_change}
                            onkeydown={on_key_down}
                            class="w-full px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded dark:bg-gray-700 dark:text-white focus:outline-none focus:ring-2 focus:ring-green-500"
                            placeholder="Image name"
                            autofocus=true
                        />

                        if let Some(err) = &*error {
                            <p class="text-xs text-red-600 dark:text-red-400">{err}</p>
                        }

                        <div class="flex justify-end space-x-2">
                            <button
                                onclick={on_cancel_click}
                                class="px-2 py-1 text-xs text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200"
                            >
                                {"Cancel"}
                            </button>
                            <button
                                onclick={on_save_click}
                                class="px-2 py-1 text-xs bg-green-600 text-white rounded hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500"
                            >
                                {"Save"}
                            </button>
                        </div>
                    </div>
                } else {
                    <h4 class="text-sm font-medium text-gray-900 dark:text-white truncate">
                        {&image.name}
                    </h4>
                }
            </div>
        </div>
    }
}

// ============================================================================
// Create Site Component
// ============================================================================

#[derive(Clone, PartialEq)]
struct CreateSiteForm {
    name: String,
    description: String,
    timezone: String,
    selected_image: Option<SiteImageId>,
    image_file: Option<File>,
    image_name: String,
    is_loading: bool,
    error: Option<String>,
}

impl Default for CreateSiteForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            timezone: get_system_timezone(),
            selected_image: None,
            image_file: None,
            image_name: String::new(),
            is_loading: false,
            error: None,
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct CreateSiteProps {
    pub community_id: String,
}

#[function_component]
pub fn CreateSite(props: &CreateSiteProps) -> Html {
    let (auth_state, _) = use_auth();
    let navigator = use_navigator().unwrap();
    let form = use_state(CreateSiteForm::default);
    let site_images = use_state(Vec::<responses::SiteImage>::new);

    // Parse community ID
    let community_id = match props.community_id.parse::<uuid::Uuid>() {
        Ok(id) => CommunityId(id),
        Err(_) => {
            navigator.push(&Route::Communities);
            return html! {};
        }
    };

    // Redirect if not authenticated
    {
        let navigator = navigator.clone();
        let auth_state = auth_state.clone();
        use_effect_with(auth_state, move |auth_state| {
            if !auth_state.is_loading && !auth_state.is_authenticated {
                navigator.push(&Route::Login);
            }
            || ()
        });
    }

    // Load existing site images
    {
        let site_images = site_images.clone();
        let community_id = community_id.clone();

        use_effect_with(props.community_id.clone(), move |_| {
            let site_images = site_images.clone();
            let community_id = community_id.clone();

            yew::platform::spawn_local(async move {
                let client = get_api_client();
                match client.list_site_images(&community_id).await {
                    Ok(images) => {
                        site_images.set(images);
                    }
                    Err(_) => {
                        // Ignore error, just show empty list
                        site_images.set(Vec::new());
                    }
                }
            });
            || ()
        });
    }

    let on_name_change = {
        let form = form.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.name = input.value();
            form.set(form_data);
        })
    };

    let on_description_change = {
        let form = form.clone();
        Callback::from(move |e: InputEvent| {
            let textarea: HtmlTextAreaElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.description = textarea.value();
            form.set(form_data);
        })
    };

    let on_timezone_change = {
        let form = form.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.timezone = select.value();
            form.set(form_data);
        })
    };

    let on_image_select_change = {
        let form = form.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            if select.value().is_empty() {
                form_data.selected_image = None;
            } else {
                if let Ok(uuid) = select.value().parse::<uuid::Uuid>() {
                    form_data.selected_image = Some(SiteImageId(uuid));
                }
            }
            form.set(form_data);
        })
    };

    let on_image_file_change = {
        let form = form.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();

            // Clear any existing error
            form_data.error = None;

            if let Some(files) = input.files() {
                if files.length() > 0 {
                    if let Some(file) = files.get(0) {
                        // Check file size (1MB = 1024 * 1024 bytes)
                        const MAX_FILE_SIZE: f64 = 1024.0 * 1024.0; // 1MB

                        if file.size() > MAX_FILE_SIZE {
                            form_data.error = Some("Image file size must be under 1MB. Please choose a smaller file or compress the image.".to_string());
                            form_data.image_file = None;
                        } else {
                            form_data.image_file = Some(file);
                            form_data.selected_image = None; // Clear existing image selection
                        }
                    }
                } else {
                    form_data.image_file = None;
                }
            }
            form.set(form_data);
        })
    };

    let on_image_name_change = {
        let form = form.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.image_name = input.value();
            form.set(form_data);
        })
    };

    let on_cancel = {
        let navigator = navigator.clone();
        let community_id = props.community_id.clone();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::Sites {
                community_id: community_id.clone(),
            });
        })
    };

    let on_submit = {
        let form = form.clone();
        let navigator = navigator.clone();
        let community_id = community_id.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let form_data = (*form).clone();

            // Validation
            if form_data.name.trim().is_empty() {
                let mut new_form = form_data;
                new_form.error = Some("Site name is required".to_string());
                form.set(new_form);
                return;
            }

            // If uploading a new image, image name is required
            if form_data.image_file.is_some()
                && form_data.image_name.trim().is_empty()
            {
                let mut new_form = form_data;
                new_form.error = Some(
                    "Image name is required when uploading an image"
                        .to_string(),
                );
                form.set(new_form);
                return;
            }

            let form = form.clone();
            let navigator = navigator.clone();
            let community_id = community_id.clone();

            yew::platform::spawn_local(async move {
                // Set loading state
                {
                    let mut new_form = (*form).clone();
                    new_form.is_loading = true;
                    new_form.error = None;
                    form.set(new_form);
                }

                let client = get_api_client();
                let form_data = (*form).clone();

                // First, upload image if provided
                let site_image_id =
                    if let Some(image_file) = form_data.image_file {
                        // Read file data
                        match read_file_as_bytes(&image_file).await {
                            Ok(image_data) => {
                                let create_image_request =
                                    requests::CreateSiteImage {
                                        community_id: community_id.clone(),
                                        name: form_data
                                            .image_name
                                            .trim()
                                            .to_string(),
                                        image_data,
                                    };

                                match client
                                    .create_site_image(&create_image_request)
                                    .await
                                {
                                    Ok(image_id) => Some(image_id),
                                    Err(e) => {
                                        let mut new_form = (*form).clone();
                                        new_form.is_loading = false;
                                        new_form.error = Some(format!(
                                            "Failed to upload image: {}",
                                            e
                                        ));
                                        form.set(new_form);
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                let mut new_form = (*form).clone();
                                new_form.is_loading = false;
                                new_form.error = Some(format!(
                                    "Failed to read image file: {}",
                                    e
                                ));
                                form.set(new_form);
                                return;
                            }
                        }
                    } else {
                        form_data.selected_image
                    };

                // Create site with default values for required fields
                let site_details = payloads::Site {
                    community_id: community_id.clone(),
                    name: form_data.name.trim().to_string(),
                    description: if form_data.description.trim().is_empty() {
                        None
                    } else {
                        Some(form_data.description.trim().to_string())
                    },
                    default_auction_params: payloads::AuctionParams {
                        round_duration: jiff::Span::new().hours(1), // 1 hour rounds
                        bid_increment: rust_decimal::Decimal::new(100, 2), // $1.00
                        activity_rule_params: payloads::ActivityRuleParams {
                            eligibility_progression: vec![(1, 1.0)], // 100% eligibility required
                        },
                    },
                    possession_period: jiff::Span::new().days(30), // 30 days
                    auction_lead_time: jiff::Span::new().days(7),  // 7 days
                    // Set default proxy bidding lead time (not used in MVP since auto_schedule is false)
                    proxy_bidding_lead_time: jiff::Span::new().days(1), // 1 day default
                    open_hours: None,
                    auto_schedule: false, // Always false for MVP
                    timezone: form_data.timezone.clone(),
                    site_image_id,
                };

                match client.create_site(&site_details).await {
                    Ok(_site_id) => {
                        // Site created successfully, navigate back to sites list
                        navigator.push(&Route::Sites {
                            community_id: community_id.0.to_string(),
                        });
                    }
                    Err(e) => {
                        let mut new_form = (*form).clone();
                        new_form.is_loading = false;
                        new_form.error =
                            Some(format!("Failed to create site: {}", e));
                        form.set(new_form);
                    }
                }
            });
        })
    };

    // Don't render anything if not authenticated
    if !auth_state.is_authenticated {
        return html! {};
    }

    html! {
        <main class="max-w-2xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
            <div class="space-y-6">
                // Header section
                <div>
                    <nav class="flex" aria-label="Breadcrumb">
                        <ol role="list" class="flex items-center space-x-4">
                            <li>
                                <div class="flex">
                                    <Link<Route>
                                        to={Route::Communities}
                                        classes="text-sm font-medium text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300"
                                    >
                                        {"Communities"}
                                    </Link<Route>>
                                </div>
                            </li>
                            <li>
                                <div class="flex items-center">
                                    <svg class="flex-shrink-0 h-5 w-5 text-gray-300" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                        <path d="M5.555 17.776l8-16 .894.448-8 16-.894-.448z" />
                                    </svg>
                                    <Link<Route>
                                        to={Route::Sites { community_id: props.community_id.clone() }}
                                        classes="ml-4 text-sm font-medium text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300"
                                    >
                                        {"Sites"}
                                    </Link<Route>>
                                </div>
                            </li>
                            <li>
                                <div class="flex items-center">
                                    <svg class="flex-shrink-0 h-5 w-5 text-gray-300" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                        <path d="M5.555 17.776l8-16 .894.448-8 16-.894-.448z" />
                                    </svg>
                                    <span class="ml-4 text-sm font-medium text-gray-500 dark:text-gray-400">{"Create Site"}</span>
                                </div>
                            </li>
                        </ol>
                    </nav>
                    <h1 class="mt-4 text-3xl font-bold text-gray-900 dark:text-white">{"Create a New Site"}</h1>
                    <p class="mt-2 text-gray-600 dark:text-gray-300">
                        {"Set up a new site for managing spaces and auctions."}
                    </p>
                </div>

                // Form
                <div class="bg-white dark:bg-gray-800 shadow-sm rounded-lg">
                    <form onsubmit={on_submit} class="space-y-6 p-6">
                        // Error message
                        if let Some(error) = &form.error {
                            <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded">
                                {error}
                            </div>
                        }

                        // Site Name
                        <div>
                            <label for="name" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                {"Site Name"}
                                <span class="text-red-500 ml-1">{"*"}</span>
                            </label>
                            <div class="mt-1">
                                <input
                                    type="text"
                                    id="name"
                                    name="name"
                                    required=true
                                    class="block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm placeholder-gray-400 focus:outline-none focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:text-white sm:text-sm"
                                    placeholder="Enter a name for your site"
                                    value={form.name.clone()}
                                    oninput={on_name_change}
                                    disabled={form.is_loading}
                                />
                            </div>
                        </div>

                        // Description
                        <div>
                            <label for="description" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                {"Description"}
                            </label>
                            <div class="mt-1">
                                <textarea
                                    id="description"
                                    name="description"
                                    rows="3"
                                    class="block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm placeholder-gray-400 focus:outline-none focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:text-white sm:text-sm"
                                    placeholder="Optional description of the site"
                                    value={form.description.clone()}
                                    oninput={on_description_change}
                                    disabled={form.is_loading}
                                />
                            </div>
                        </div>

                        // Timezone
                        <div>
                            <label for="timezone" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                {"Timezone"}
                            </label>
                            <div class="mt-1">
                                <select
                                    id="timezone"
                                    name="timezone"
                                    class="block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:text-white sm:text-sm"
                                    value={form.timezone.clone()}
                                    onchange={on_timezone_change}
                                    disabled={form.is_loading}
                                >
                                    {for get_timezone_options().iter().map(|tz| {
                                        html! {
                                            <option value={tz.clone()}>
                                                {tz}
                                            </option>
                                        }
                                    })}
                                </select>
                            </div>
                        </div>



                        // Site Image Section
                        <div class="border-t border-gray-200 dark:border-gray-700 pt-6">
                            <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-4">{"Site Image"}</h3>

                            // Existing Images
                            if !site_images.is_empty() {
                                <div class="mb-4">
                                    <label for="existing_image" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                        {"Select Existing Image"}
                                    </label>
                                    <select
                                        id="existing_image"
                                        name="existing_image"
                                        class="block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:text-white sm:text-sm"
                                        value={form.selected_image.map(|id| id.0.to_string()).unwrap_or_default()}
                                        onchange={on_image_select_change}
                                        disabled={form.is_loading}
                                    >
                                        <option value="">{"No image selected"}</option>
                                        {for site_images.iter().map(|image| {
                                            html! {
                                                <option value={image.id.0.to_string()}>
                                                    {&image.name}
                                                </option>
                                            }
                                        })}
                                    </select>
                                </div>

                                <div class="text-center text-sm text-gray-500 dark:text-gray-400 my-4">
                                    {"— OR —"}
                                </div>
                            }

                            // Upload New Image
                            <div>
                                <label for="image_file" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                    {"Upload New Image"}
                                </label>
                                <input
                                    type="file"
                                    id="image_file"
                                    name="image_file"
                                    accept="image/*"
                                    class="block w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-blue-50 file:text-blue-700 hover:file:bg-blue-100"
                                    onchange={on_image_file_change}
                                    disabled={form.is_loading}
                                />
                                <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                                    {"Maximum file size: 1MB. Supported formats: JPEG, PNG, GIF, WebP"}
                                </p>

                                if form.image_file.is_some() {
                                    <div class="mt-2">
                                        <label for="image_name" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                            {"Image Name"}
                                            <span class="text-red-500 ml-1">{"*"}</span>
                                        </label>
                                        <input
                                            type="text"
                                            id="image_name"
                                            name="image_name"
                                            class="mt-1 block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm placeholder-gray-400 focus:outline-none focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:text-white sm:text-sm"
                                            placeholder="Enter a name for the image"
                                            value={form.image_name.clone()}
                                            oninput={on_image_name_change}
                                            disabled={form.is_loading}
                                        />
                                    </div>
                                }
                            </div>
                        </div>

                        // Submit buttons
                        <div class="flex justify-end space-x-3 pt-6 border-t border-gray-200 dark:border-gray-700">
                            <button
                                type="button"
                                onclick={on_cancel}
                                class="px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 hover:bg-gray-50 dark:hover:bg-gray-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                                disabled={form.is_loading}
                            >
                                {"Cancel"}
                            </button>
                            <button
                                type="submit"
                                class="px-4 py-2 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 disabled:opacity-50"
                                disabled={form.is_loading}
                            >
                                if form.is_loading {
                                    {"Creating..."}
                                } else {
                                    {"Create Site"}
                                }
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </main>
    }
}

// ============================================================================
// Site Details Component
// ============================================================================

#[derive(Default, Clone)]
pub struct SiteDetailsState {
    pub site: Option<responses::Site>,
    pub spaces: Vec<responses::Space>,
    pub site_image: Option<responses::SiteImage>,
    pub is_loading: bool,
    pub error: Option<String>,
}

#[derive(Properties, PartialEq)]
pub struct SiteDetailsProps {
    pub site_id: String,
}

#[function_component]
pub fn SiteDetails(props: &SiteDetailsProps) -> Html {
    let (auth_state, _) = use_auth();
    let navigator = use_navigator().unwrap();
    let state = use_state(SiteDetailsState::default);

    // Parse site ID
    let site_id = match props.site_id.parse::<uuid::Uuid>() {
        Ok(id) => payloads::SiteId(id),
        Err(_) => {
            navigator.push(&Route::Communities);
            return html! {};
        }
    };

    // Redirect if not authenticated
    {
        let navigator = navigator.clone();
        let auth_state = auth_state.clone();
        use_effect_with(auth_state, move |auth_state| {
            if !auth_state.is_loading && !auth_state.is_authenticated {
                navigator.push(&Route::Login);
            }
            || ()
        });
    }

    // Load site details when component mounts
    {
        let state = state.clone();
        let auth_state = auth_state.clone();
        let site_id = site_id.clone();

        use_effect_with(
            (auth_state.is_authenticated, props.site_id.clone()),
            move |(is_authenticated, _)| {
                if *is_authenticated {
                    let state = state.clone();
                    let site_id = site_id.clone();

                    yew::platform::spawn_local(async move {
                        let mut current_state = (*state).clone();
                        current_state.is_loading = true;
                        current_state.error = None;
                        state.set(current_state);

                        let client = get_api_client();

                        // Load site details and spaces
                        let site_future = client.get_site(&site_id);
                        let spaces_future = client.list_spaces(&site_id);

                        match futures::future::try_join(
                            site_future,
                            spaces_future,
                        )
                        .await
                        {
                            Ok((site, spaces)) => {
                                let site_image = if let Some(site_image_id) =
                                    site.site_details.site_image_id
                                {
                                    client
                                        .get_site_image(&site_image_id)
                                        .await
                                        .ok()
                                } else {
                                    None
                                };

                                let mut current_state = (*state).clone();
                                current_state.site = Some(site);
                                current_state.spaces = spaces;
                                current_state.site_image = site_image;
                                current_state.is_loading = false;
                                state.set(current_state);
                            }
                            Err(e) => {
                                let mut current_state = (*state).clone();
                                current_state.error =
                                    Some(format!("Failed to load site: {}", e));
                                current_state.is_loading = false;
                                state.set(current_state);
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    let on_edit = {
        let navigator = navigator.clone();
        let site_id = props.site_id.clone();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::EditSite {
                id: site_id.clone(),
            });
        })
    };

    let on_delete = {
        let navigator = navigator.clone();
        let site_id = site_id.clone();
        let state = state.clone();
        Callback::from(move |_: MouseEvent| {
            let site_id = site_id.clone();
            let navigator = navigator.clone();
            let state = state.clone();

            // Show confirmation dialog
            if let Some(window) = web_sys::window() {
                if window.confirm_with_message("Are you sure you want to delete this site? This action cannot be undone.").unwrap_or(false) {
                    yew::platform::spawn_local(async move {
                        let client = get_api_client();
                        match client.delete_site(&site_id).await {
                            Ok(()) => {
                                navigator.push(&Route::Communities);
                            }
                            Err(e) => {
                                let mut current_state = (*state).clone();
                                current_state.error = Some(format!("Failed to delete site: {}", e));
                                state.set(current_state);
                            }
                        }
                    });
                }
            }
        })
    };

    let on_back = {
        let navigator = navigator.clone();
        let state = state.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(site) = &state.site {
                navigator.push(&Route::Sites {
                    community_id: site.site_details.community_id.0.to_string(),
                });
            } else {
                navigator.push(&Route::Communities);
            }
        })
    };

    // Don't render anything if not authenticated
    if !auth_state.is_authenticated {
        return html! {};
    }

    html! {
        <main class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
            <div class="space-y-6">
                // Header section
                <div class="space-y-4 sm:space-y-0 sm:flex sm:justify-between sm:items-start">
                    <div class="min-w-0 flex-1">
                        <nav class="flex" aria-label="Breadcrumb">
                            <ol role="list" class="flex items-center space-x-4">
                                <li>
                                    <div class="flex">
                                        <Link<Route>
                                            to={Route::Communities}
                                            classes="text-sm font-medium text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300"
                                        >
                                            {"Communities"}
                                        </Link<Route>>
                                    </div>
                                </li>
                                <li>
                                    <div class="flex items-center">
                                        <svg class="flex-shrink-0 h-5 w-5 text-gray-300" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                            <path d="M5.555 17.776l8-16 .894.448-8 16-.894-.448z" />
                                        </svg>
                                        <button
                                            onclick={on_back.clone()}
                                            class="ml-4 text-sm font-medium text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300"
                                        >
                                            {"Sites"}
                                        </button>
                                    </div>
                                </li>
                                <li>
                                    <div class="flex items-center">
                                        <svg class="flex-shrink-0 h-5 w-5 text-gray-300" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                            <path d="M5.555 17.776l8-16 .894.448-8 16-.894-.448z" />
                                        </svg>
                                        <span class="ml-4 text-sm font-medium text-gray-500 dark:text-gray-400">
                                            {if let Some(site) = &state.site { &site.site_details.name } else { "Site Details" }}
                                        </span>
                                    </div>
                                </li>
                            </ol>
                        </nav>
                        <h1 class="mt-4 text-2xl sm:text-3xl font-bold text-gray-900 dark:text-white">
                            {if let Some(site) = &state.site { &site.site_details.name } else { "Site Details" }}
                        </h1>
                        if let Some(site) = &state.site {
                            if let Some(description) = &site.site_details.description {
                                <p class="mt-1 sm:mt-2 text-sm sm:text-base text-gray-600 dark:text-gray-300">
                                    {description}
                                </p>
                            }
                        }
                    </div>
                    <div class="flex flex-col sm:flex-row gap-3 sm:space-x-3 sm:gap-0 flex-shrink-0">
                        <button
                            onclick={on_edit.clone()}
                            class="inline-flex items-center justify-center px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 hover:bg-gray-50 dark:hover:bg-gray-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                        >
                            <svg class="-ml-1 mr-2 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                            </svg>
                            {"Edit Site"}
                        </button>
                        <button
                            onclick={on_delete.clone()}
                            class="inline-flex items-center justify-center px-4 py-2 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-red-600 hover:bg-red-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-red-500"
                        >
                            <svg class="-ml-1 mr-2 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                            </svg>
                            {"Delete Site"}
                        </button>
                    </div>
                </div>

                // Error message
                if let Some(error) = &state.error {
                    <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded">
                        {error}
                    </div>
                }

                // Loading state
                if state.is_loading {
                    <div class="text-center py-8">
                        <svg class="animate-spin h-8 w-8 text-blue-600 mx-auto" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                        </svg>
                        <p class="mt-2 text-gray-600 dark:text-gray-400">{"Loading site details..."}</p>
                    </div>
                } else if let Some(site) = &state.site {
                    // Site details content
                    <div class="bg-white dark:bg-gray-800 shadow rounded-lg">
                        <div class="px-4 py-5 sm:p-6">
                            <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
                                // Site image
                                <div class="lg:col-span-1">
                                    if let Some(site_image) = &state.site_image {
                                        <div class="aspect-square rounded-lg overflow-hidden bg-gray-100 dark:bg-gray-700">
                                            <img
                                                src={format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(&site_image.image_data))}
                                                alt={site_image.name.clone()}
                                                class="w-full h-full object-cover"
                                            />
                                        </div>
                                    } else {
                                        <div class="aspect-square rounded-lg bg-gray-100 dark:bg-gray-700 flex items-center justify-center">
                                            <svg class="h-16 w-16 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
                                            </svg>
                                        </div>
                                    }
                                </div>

                                // Site details
                                <div class="lg:col-span-2">
                                    <dl class="grid grid-cols-1 sm:grid-cols-2 gap-4">
                                        <div>
                                            <dt class="text-sm font-medium text-gray-500 dark:text-gray-400">{"Name"}</dt>
                                            <dd class="mt-1 text-sm text-gray-900 dark:text-white">{&site.site_details.name}</dd>
                                        </div>
                                        if let Some(description) = &site.site_details.description {
                                            <div class="sm:col-span-2">
                                                <dt class="text-sm font-medium text-gray-500 dark:text-gray-400">{"Description"}</dt>
                                                <dd class="mt-1 text-sm text-gray-900 dark:text-white">{description}</dd>
                                            </div>
                                        }
                                        <div>
                                            <dt class="text-sm font-medium text-gray-500 dark:text-gray-400">{"Timezone"}</dt>
                                            <dd class="mt-1 text-sm text-gray-900 dark:text-white">{&site.site_details.timezone}</dd>
                                        </div>
                                        <div>
                                            <dt class="text-sm font-medium text-gray-500 dark:text-gray-400">{"Auto Schedule"}</dt>
                                            <dd class="mt-1 text-sm text-gray-900 dark:text-white">
                                                {if site.site_details.auto_schedule { "Enabled" } else { "Disabled" }}
                                            </dd>
                                        </div>
                                        <div>
                                            <dt class="text-sm font-medium text-gray-500 dark:text-gray-400">{"Created"}</dt>
                                            <dd class="mt-1 text-sm text-gray-900 dark:text-white">
                                                {format!("{}", site.created_at.strftime("%B %d, %Y at %H:%M"))}
                                            </dd>
                                        </div>
                                        <div>
                                            <dt class="text-sm font-medium text-gray-500 dark:text-gray-400">{"Last Updated"}</dt>
                                            <dd class="mt-1 text-sm text-gray-900 dark:text-white">
                                                {format!("{}", site.updated_at.strftime("%B %d, %Y at %H:%M"))}
                                            </dd>
                                        </div>
                                    </dl>
                                </div>
                            </div>
                        </div>
                    </div>

                    // Spaces section
                    <div class="bg-white dark:bg-gray-800 shadow rounded-lg">
                        <div class="px-4 py-5 sm:p-6">
                            <div class="flex items-center justify-between mb-4">
                                <h3 class="text-lg font-medium text-gray-900 dark:text-white">
                                    {"Spaces"}
                                    <span class="ml-2 text-sm text-gray-500 dark:text-gray-400">
                                        {"("}{state.spaces.len()}{" total)"}
                                    </span>
                                </h3>
                            </div>

                            if state.spaces.is_empty() {
                                <div class="text-center py-8">
                                    <svg class="mx-auto h-12 w-12 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4"></path>
                                    </svg>
                                    <h3 class="mt-2 text-sm font-medium text-gray-900 dark:text-white">{"No spaces"}</h3>
                                    <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">{"This site doesn't have any spaces yet."}</p>
                                </div>
                            } else {
                                <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
                                    {for state.spaces.iter().map(|space| {
                                        html! {
                                            <div class="border border-gray-200 dark:border-gray-700 rounded-lg p-4">
                                                <h4 class="font-medium text-gray-900 dark:text-white">{&space.space_details.name}</h4>
                                                if let Some(description) = &space.space_details.description {
                                                    <p class="mt-1 text-sm text-gray-600 dark:text-gray-300">{description}</p>
                                                }
                                                <div class="mt-2 space-y-1">
                                                    <div class="flex justify-between text-sm">
                                                        <span class="text-gray-500 dark:text-gray-400">{"Eligibility Points:"}</span>
                                                        <span class="text-gray-900 dark:text-white">{space.space_details.eligibility_points}</span>
                                                    </div>
                                                    <div class="flex justify-between text-sm">
                                                        <span class="text-gray-500 dark:text-gray-400">{"Available:"}</span>
                                                        <span class={if space.space_details.is_available { "text-green-600" } else { "text-red-600" }}>
                                                            {if space.space_details.is_available { "Yes" } else { "No" }}
                                                        </span>
                                                    </div>
                                                </div>
                                            </div>
                                        }
                                    })}
                                </div>
                            }
                        </div>
                    </div>
                }
            </div>
        </main>
    }
}

// ============================================================================
// Edit Site Component
// ============================================================================

#[derive(Default, Clone)]
pub struct EditSiteState {
    pub site: Option<responses::Site>,
    pub site_images: Vec<responses::SiteImage>,
    pub form: EditSiteForm,
    pub is_loading: bool,
    pub error: Option<String>,
}

#[derive(Default, Clone)]
struct EditSiteForm {
    name: String,
    description: String,
    timezone: String,
    auto_schedule: bool,
    selected_image: Option<SiteImageId>,
    image_file: Option<File>,
    image_name: String,
    is_loading: bool,
    error: Option<String>,
}

#[derive(Properties, PartialEq)]
pub struct EditSiteProps {
    pub site_id: String,
}

#[function_component]
pub fn EditSite(props: &EditSiteProps) -> Html {
    let (auth_state, _) = use_auth();
    let navigator = use_navigator().unwrap();
    let state = use_state(EditSiteState::default);

    // Parse site ID
    let site_id = match props.site_id.parse::<uuid::Uuid>() {
        Ok(id) => payloads::SiteId(id),
        Err(_) => {
            navigator.push(&Route::Communities);
            return html! {};
        }
    };

    // Redirect if not authenticated
    {
        let navigator = navigator.clone();
        let auth_state = auth_state.clone();
        use_effect_with(auth_state, move |auth_state| {
            if !auth_state.is_loading && !auth_state.is_authenticated {
                navigator.push(&Route::Login);
            }
            || ()
        });
    }

    // Load site details when component mounts
    {
        let state = state.clone();
        let auth_state = auth_state.clone();
        let site_id = site_id.clone();

        use_effect_with(
            (auth_state.is_authenticated, props.site_id.clone()),
            move |(is_authenticated, _)| {
                if *is_authenticated {
                    let state = state.clone();
                    let site_id = site_id.clone();

                    yew::platform::spawn_local(async move {
                        let mut current_state = (*state).clone();
                        current_state.is_loading = true;
                        current_state.error = None;
                        state.set(current_state);

                        let client = get_api_client();

                        // Load site details and site images
                        let site_future = client.get_site(&site_id);

                        match site_future.await {
                            Ok(site) => {
                                let images_future = client.list_site_images(
                                    &site.site_details.community_id,
                                );

                                match images_future.await {
                                    Ok(images) => {
                                        let mut current_state =
                                            (*state).clone();
                                        current_state.form.name =
                                            site.site_details.name.clone();
                                        current_state.form.description = site
                                            .site_details
                                            .description
                                            .clone()
                                            .unwrap_or_default();
                                        current_state.form.timezone =
                                            site.site_details.timezone.clone();
                                        current_state.form.auto_schedule =
                                            site.site_details.auto_schedule;
                                        current_state.form.selected_image =
                                            site.site_details.site_image_id;
                                        current_state.site = Some(site);
                                        current_state.site_images = images;
                                        current_state.is_loading = false;
                                        state.set(current_state);
                                    }
                                    Err(e) => {
                                        let mut current_state =
                                            (*state).clone();
                                        current_state.error = Some(format!(
                                            "Failed to load site images: {}",
                                            e
                                        ));
                                        current_state.is_loading = false;
                                        state.set(current_state);
                                    }
                                }
                            }
                            Err(e) => {
                                let mut current_state = (*state).clone();
                                current_state.error =
                                    Some(format!("Failed to load site: {}", e));
                                current_state.is_loading = false;
                                state.set(current_state);
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    let on_name_change = {
        let state = state.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut current_state = (*state).clone();
            current_state.form.name = input.value();
            state.set(current_state);
        })
    };

    let on_description_change = {
        let state = state.clone();
        Callback::from(move |e: InputEvent| {
            let textarea: HtmlTextAreaElement = e.target_unchecked_into();
            let mut current_state = (*state).clone();
            current_state.form.description = textarea.value();
            state.set(current_state);
        })
    };

    let on_timezone_change = {
        let state = state.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            let mut current_state = (*state).clone();
            current_state.form.timezone = select.value();
            state.set(current_state);
        })
    };

    let on_auto_schedule_change = {
        let state = state.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut current_state = (*state).clone();
            current_state.form.auto_schedule = input.checked();
            state.set(current_state);
        })
    };

    let on_image_select_change = {
        let state = state.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            let mut current_state = (*state).clone();
            if select.value().is_empty() {
                current_state.form.selected_image = None;
            } else {
                if let Ok(uuid) = select.value().parse::<uuid::Uuid>() {
                    current_state.form.selected_image = Some(SiteImageId(uuid));
                }
            }
            state.set(current_state);
        })
    };

    let on_image_file_change = {
        let state = state.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut current_state = (*state).clone();

            // Clear any existing error
            current_state.form.error = None;

            if let Some(files) = input.files() {
                if files.length() > 0 {
                    if let Some(file) = files.get(0) {
                        // Check file size (1MB = 1024 * 1024 bytes)
                        const MAX_FILE_SIZE: f64 = 1024.0 * 1024.0; // 1MB

                        if file.size() > MAX_FILE_SIZE {
                            current_state.form.error = Some("Image file size must be under 1MB. Please choose a smaller file or compress the image.".to_string());
                            current_state.form.image_file = None;
                        } else {
                            current_state.form.image_file = Some(file);
                            current_state.form.selected_image = None; // Clear existing selection
                        }
                    }
                } else {
                    current_state.form.image_file = None;
                }
            }

            state.set(current_state);
        })
    };

    let on_image_name_change = {
        let state = state.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut current_state = (*state).clone();
            current_state.form.image_name = input.value();
            state.set(current_state);
        })
    };

    let on_submit = {
        let state = state.clone();
        let navigator = navigator.clone();
        let site_id = site_id.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let state = state.clone();
            let navigator = navigator.clone();
            let site_id = site_id.clone();

            yew::platform::spawn_local(async move {
                let mut current_state = (*state).clone();
                current_state.form.is_loading = true;
                current_state.form.error = None;
                state.set(current_state.clone());

                let client = get_api_client();
                let form = &current_state.form;
                let site = current_state.site.as_ref();

                if let Some(site) = site {
                    // Handle image upload if a new file is selected
                    let final_image_id = if let Some(image_file) =
                        &form.image_file
                    {
                        if form.image_name.trim().is_empty() {
                            let mut current_state = (*state).clone();
                            current_state.form.error = Some(
                                "Image name is required when uploading a file."
                                    .to_string(),
                            );
                            current_state.form.is_loading = false;
                            state.set(current_state);
                            return;
                        }

                        match read_file_as_bytes(image_file).await {
                            Ok(image_data) => {
                                let create_image_request =
                                    requests::CreateSiteImage {
                                        community_id: site
                                            .site_details
                                            .community_id,
                                        name: form.image_name.clone(),
                                        image_data,
                                    };

                                match client
                                    .create_site_image(&create_image_request)
                                    .await
                                {
                                    Ok(image_id) => Some(image_id),
                                    Err(e) => {
                                        let mut current_state =
                                            (*state).clone();
                                        current_state.form.error =
                                            Some(format!(
                                                "Failed to upload image: {}",
                                                e
                                            ));
                                        current_state.form.is_loading = false;
                                        state.set(current_state);
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                let mut current_state = (*state).clone();
                                current_state.form.error = Some(format!(
                                    "Failed to read image file: {}",
                                    e
                                ));
                                current_state.form.is_loading = false;
                                state.set(current_state);
                                return;
                            }
                        }
                    } else {
                        form.selected_image
                    };

                    // Update the site
                    let site_details = payloads::Site {
                        community_id: site.site_details.community_id,
                        name: form.name.clone(),
                        description: if form.description.trim().is_empty() {
                            None
                        } else {
                            Some(form.description.clone())
                        },
                        default_auction_params: site
                            .site_details
                            .default_auction_params
                            .clone(),
                        possession_period: site
                            .site_details
                            .possession_period
                            .clone(),
                        auction_lead_time: site
                            .site_details
                            .auction_lead_time
                            .clone(),
                        proxy_bidding_lead_time: site
                            .site_details
                            .proxy_bidding_lead_time
                            .clone(),
                        open_hours: site.site_details.open_hours.clone(),
                        auto_schedule: form.auto_schedule,
                        timezone: form.timezone.clone(),
                        site_image_id: final_image_id,
                    };

                    let update_request = requests::UpdateSite {
                        site_id,
                        site_details,
                    };

                    match client.update_site(&update_request).await {
                        Ok(_) => {
                            navigator.push(&Route::SiteDetails {
                                id: site_id.0.to_string(),
                            });
                        }
                        Err(e) => {
                            let mut current_state = (*state).clone();
                            current_state.form.error =
                                Some(format!("Failed to update site: {}", e));
                            current_state.form.is_loading = false;
                            state.set(current_state);
                        }
                    }
                }
            });
        })
    };

    let on_cancel = {
        let navigator = navigator.clone();
        let site_id = props.site_id.clone();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::SiteDetails {
                id: site_id.clone(),
            });
        })
    };

    // Don't render anything if not authenticated
    if !auth_state.is_authenticated {
        return html! {};
    }

    let timezone_options = get_timezone_options();

    html! {
        <main class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
            <div class="space-y-6">
                // Header section
                <div class="space-y-4 sm:space-y-0 sm:flex sm:justify-between sm:items-start">
                    <div class="min-w-0 flex-1">
                        <nav class="flex" aria-label="Breadcrumb">
                            <ol role="list" class="flex items-center space-x-4">
                                <li>
                                    <div class="flex">
                                        <Link<Route>
                                            to={Route::Communities}
                                            classes="text-sm font-medium text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300"
                                        >
                                            {"Communities"}
                                        </Link<Route>>
                                    </div>
                                </li>
                                <li>
                                    <div class="flex items-center">
                                        <svg class="flex-shrink-0 h-5 w-5 text-gray-300" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                            <path d="M5.555 17.776l8-16 .894.448-8 16-.894-.448z" />
                                        </svg>
                                        <Link<Route>
                                            to={Route::SiteDetails { id: props.site_id.clone() }}
                                            classes="ml-4 text-sm font-medium text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300"
                                        >
                                            {if let Some(site) = &state.site { &site.site_details.name } else { "Site" }}
                                        </Link<Route>>
                                    </div>
                                </li>
                                <li>
                                    <div class="flex items-center">
                                        <svg class="flex-shrink-0 h-5 w-5 text-gray-300" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                            <path d="M5.555 17.776l8-16 .894.448-8 16-.894-.448z" />
                                        </svg>
                                        <span class="ml-4 text-sm font-medium text-gray-500 dark:text-gray-400">{"Edit"}</span>
                                    </div>
                                </li>
                            </ol>
                        </nav>
                        <h1 class="mt-4 text-2xl sm:text-3xl font-bold text-gray-900 dark:text-white">
                            {"Edit Site"}
                        </h1>
                        <p class="mt-1 sm:mt-2 text-sm sm:text-base text-gray-600 dark:text-gray-300">
                            {"Update site information and settings"}
                        </p>
                    </div>
                </div>

                // Error message
                if let Some(error) = &state.error {
                    <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded">
                        {error}
                    </div>
                }

                // Loading state
                if state.is_loading {
                    <div class="text-center py-8">
                        <svg class="animate-spin h-8 w-8 text-blue-600 mx-auto" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                        </svg>
                        <p class="mt-2 text-gray-600 dark:text-gray-400">{"Loading site details..."}</p>
                    </div>
                } else if state.site.is_some() {
                    // Edit form
                    <div class="bg-white dark:bg-gray-800 shadow rounded-lg">
                        <div class="px-4 py-5 sm:p-6">
                            <form onsubmit={on_submit}>
                                // Form error message
                                if let Some(error) = &state.form.error {
                                    <div class="mb-6 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded">
                                        {error}
                                    </div>
                                }

                                <div class="space-y-6">
                                    // Basic Information
                                    <div>
                                        <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-4">{"Basic Information"}</h3>
                                        <div class="grid grid-cols-1 gap-4">
                                            <div>
                                                <label for="name" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                                    {"Site Name"}
                                                    <span class="text-red-500 ml-1">{"*"}</span>
                                                </label>
                                                <input
                                                    type="text"
                                                    id="name"
                                                    name="name"
                                                    required=true
                                                    class="mt-1 block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm placeholder-gray-400 focus:outline-none focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:text-white sm:text-sm"
                                                    placeholder="Enter site name"
                                                    value={state.form.name.clone()}
                                                    oninput={on_name_change}
                                                    disabled={state.form.is_loading}
                                                />
                                            </div>

                                            <div>
                                                <label for="description" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                                    {"Description"}
                                                </label>
                                                <textarea
                                                    id="description"
                                                    name="description"
                                                    rows="3"
                                                    class="mt-1 block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm placeholder-gray-400 focus:outline-none focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:text-white sm:text-sm"
                                                    placeholder="Optional description of the site"
                                                    value={state.form.description.clone()}
                                                    oninput={on_description_change}
                                                    disabled={state.form.is_loading}
                                                />
                                            </div>
                                        </div>
                                    </div>

                                    // Site Configuration
                                    <div class="border-t border-gray-200 dark:border-gray-700 pt-6">
                                        <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-4">{"Configuration"}</h3>
                                        <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
                                            <div>
                                                <label for="timezone" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                                    {"Timezone"}
                                                    <span class="text-red-500 ml-1">{"*"}</span>
                                                </label>
                                                <select
                                                    id="timezone"
                                                    name="timezone"
                                                    required=true
                                                    class="mt-1 block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:text-white sm:text-sm"
                                                    value={state.form.timezone.clone()}
                                                    onchange={on_timezone_change}
                                                    disabled={state.form.is_loading}
                                                >
                                                    {for timezone_options.iter().map(|tz| {
                                                        html! {
                                                            <option value={tz.clone()}>
                                                                {tz}
                                                            </option>
                                                        }
                                                    })}
                                                </select>
                                            </div>

                                            <div class="flex items-center pt-6">
                                                <input
                                                    id="auto_schedule"
                                                    name="auto_schedule"
                                                    type="checkbox"
                                                    class="h-4 w-4 text-blue-600 focus:ring-blue-500 border-gray-300 dark:border-gray-600 rounded dark:bg-gray-700"
                                                    checked={state.form.auto_schedule}
                                                    onchange={on_auto_schedule_change}
                                                    disabled={state.form.is_loading}
                                                />
                                                <label for="auto_schedule" class="ml-2 block text-sm text-gray-900 dark:text-white">
                                                    {"Enable automatic scheduling"}
                                                </label>
                                            </div>
                                        </div>
                                    </div>

                                    // Site Image
                                    <div class="border-t border-gray-200 dark:border-gray-700 pt-6">
                                        <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-4">{"Site Image"}</h3>

                                        // Existing Images
                                        if !state.site_images.is_empty() {
                                            <div class="mb-4">
                                                <label for="existing_image" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                                    {"Select Existing Image"}
                                                </label>
                                                <select
                                                    id="existing_image"
                                                    name="existing_image"
                                                    class="block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:text-white sm:text-sm"
                                                    value={state.form.selected_image.map(|id| id.0.to_string()).unwrap_or_default()}
                                                    onchange={on_image_select_change}
                                                    disabled={state.form.is_loading}
                                                >
                                                    <option value="">{"No image selected"}</option>
                                                    {for state.site_images.iter().map(|image| {
                                                        html! {
                                                            <option value={image.id.0.to_string()}>
                                                                {&image.name}
                                                            </option>
                                                        }
                                                    })}
                                                </select>
                                            </div>

                                            <div class="text-center text-sm text-gray-500 dark:text-gray-400 my-4">
                                                {"— OR —"}
                                            </div>
                                        }

                                        // Upload New Image
                                        <div>
                                            <label for="image_file" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                                {"Upload New Image"}
                                            </label>
                                            <input
                                                type="file"
                                                id="image_file"
                                                name="image_file"
                                                accept="image/*"
                                                class="block w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-full file:border-0 file:text-sm file:font-semibold file:bg-blue-50 file:text-blue-700 hover:file:bg-blue-100"
                                                onchange={on_image_file_change}
                                                disabled={state.form.is_loading}
                                            />
                                            <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                                                {"Maximum file size: 1MB. Supported formats: JPEG, PNG, GIF, WebP"}
                                            </p>

                                            if state.form.image_file.is_some() {
                                                <div class="mt-2">
                                                    <label for="image_name" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                                        {"Image Name"}
                                                        <span class="text-red-500 ml-1">{"*"}</span>
                                                    </label>
                                                    <input
                                                        type="text"
                                                        id="image_name"
                                                        name="image_name"
                                                        class="mt-1 block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm placeholder-gray-400 focus:outline-none focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:text-white sm:text-sm"
                                                        placeholder="Enter a name for the image"
                                                        value={state.form.image_name.clone()}
                                                        oninput={on_image_name_change}
                                                        disabled={state.form.is_loading}
                                                    />
                                                </div>
                                            }
                                        </div>
                                    </div>

                                    // Add Spaces Section
                                    <div class="border-t border-gray-200 dark:border-gray-700 pt-6">
                                        <AddSpacesToSite site_id={site_id.clone()} />
                                    </div>

                                    // Submit buttons
                                    <div class="flex justify-end space-x-3 pt-6 border-t border-gray-200 dark:border-gray-700">
                                        <button
                                            type="button"
                                            onclick={on_cancel}
                                            class="px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 hover:bg-gray-50 dark:hover:bg-gray-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                                            disabled={state.form.is_loading}
                                        >
                                            {"Cancel"}
                                        </button>
                                        <button
                                            type="submit"
                                            class="px-4 py-2 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 disabled:opacity-50"
                                            disabled={state.form.is_loading}
                                        >
                                            if state.form.is_loading {
                                                {"Updating..."}
                                            } else {
                                                {"Update Site"}
                                            }
                                        </button>
                                    </div>
                                </div>
                            </form>
                        </div>
                    </div>
                }
            </div>
        </main>
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

async fn read_file_as_bytes(file: &File) -> Result<Vec<u8>, String> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::FileReader;

    let file_reader =
        FileReader::new().map_err(|_| "Failed to create FileReader")?;

    let file_reader_clone = file_reader.clone();
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let file_reader_for_closure = file_reader_clone.clone();
        let onload = wasm_bindgen::closure::Closure::wrap(Box::new(
            move |_: web_sys::Event| {
                if let Ok(array_buffer) = file_reader_for_closure.result() {
                    resolve
                        .call1(&wasm_bindgen::JsValue::NULL, &array_buffer)
                        .unwrap();
                } else {
                    reject
                        .call1(
                            &wasm_bindgen::JsValue::NULL,
                            &wasm_bindgen::JsValue::from_str(
                                "Failed to read file",
                            ),
                        )
                        .unwrap();
                }
            },
        )
            as Box<dyn FnMut(_)>);

        file_reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();
    });

    file_reader
        .read_as_array_buffer(file)
        .map_err(|_| "Failed to start reading file")?;

    let result = JsFuture::from(promise)
        .await
        .map_err(|_| "Failed to read file")?;
    let array_buffer: js_sys::ArrayBuffer =
        result.dyn_into().map_err(|_| "Invalid file data")?;
    let uint8_array = js_sys::Uint8Array::new(&array_buffer);

    Ok(uint8_array.to_vec())
}

fn get_timezone_options() -> Vec<String> {
    // Get all available timezones from jiff's timezone database
    let mut timezones: Vec<String> = jiff::tz::db()
        .available()
        .map(|tz| tz.to_string())
        .collect();

    // Sort alphabetically for better user experience
    timezones.sort();

    timezones
}

fn get_system_timezone() -> String {
    // Get the system timezone using jiff
    let tz = jiff::tz::TimeZone::system();

    if let Some(name) = tz.iana_name() {
        name.to_string()
    } else {
        // Fallback to UTC if no IANA name is available
        "UTC".to_string()
    }
}

// ============================================================================
// Add Spaces to Site Component
// ============================================================================

#[derive(Clone)]
struct SpaceForm {
    name: String,
    description: String,
    eligibility_points: String,
    is_available: bool,
    selected_image: Option<SiteImageId>,
    is_loading: bool,
    error: Option<String>,
}

impl Default for SpaceForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            eligibility_points: "1.0".to_string(), // Default to 1.0 eligibility points
            is_available: true,                    // Default to available
            selected_image: None,
            is_loading: false,
            error: None,
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct AddSpacesToSiteProps {
    pub site_id: payloads::SiteId,
}

#[function_component]
pub fn AddSpacesToSite(props: &AddSpacesToSiteProps) -> Html {
    let form = use_state(SpaceForm::default);
    let spaces = use_state(Vec::<responses::Space>::new);
    let site_images = use_state(Vec::<responses::SiteImage>::new);
    let community_id = use_state(|| None::<CommunityId>);
    let show_form = use_state(|| false);

    // Load existing spaces for this site
    {
        let spaces = spaces.clone();
        let site_id = props.site_id.clone();

        use_effect_with(props.site_id, move |_| {
            let spaces = spaces.clone();
            let site_id = site_id.clone();

            yew::platform::spawn_local(async move {
                let client = get_api_client();
                match client.list_spaces(&site_id).await {
                    Ok(space_list) => {
                        spaces.set(space_list);
                    }
                    Err(_) => {
                        // Ignore error, just show empty list
                        spaces.set(Vec::new());
                    }
                }
            });
            || ()
        });
    }

    // Load site images when component mounts (needed for editing existing spaces)
    {
        let site_images = site_images.clone();
        let community_id = community_id.clone();
        let site_id = props.site_id.clone();

        use_effect_with(props.site_id, move |_| {
            let site_images = site_images.clone();
            let community_id = community_id.clone();
            let site_id = site_id.clone();

            yew::platform::spawn_local(async move {
                let client = get_api_client();
                // First get the site to find its community_id
                match client.get_site(&site_id).await {
                    Ok(site) => {
                        let site_community_id = site.site_details.community_id;
                        community_id.set(Some(site_community_id));

                        match client.list_site_images(&site_community_id).await
                        {
                            Ok(images) => {
                                site_images.set(images);
                            }
                            Err(_) => {
                                site_images.set(Vec::new());
                            }
                        }
                    }
                    Err(_) => {
                        site_images.set(Vec::new());
                        community_id.set(None);
                    }
                }
            });
            || ()
        });
    }

    let on_add_space_click = {
        let show_form = show_form.clone();
        let form = form.clone();
        Callback::from(move |_: MouseEvent| {
            show_form.set(true);
            form.set(SpaceForm::default());
        })
    };

    let on_cancel_add = {
        let show_form = show_form.clone();
        Callback::from(move |_: MouseEvent| {
            show_form.set(false);
        })
    };

    let on_name_change = {
        let form = form.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.name = input.value();
            form.set(form_data);
        })
    };

    let on_description_change = {
        let form = form.clone();
        Callback::from(move |e: InputEvent| {
            let textarea: HtmlTextAreaElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.description = textarea.value();
            form.set(form_data);
        })
    };

    let on_eligibility_points_change = {
        let form = form.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.eligibility_points = input.value();
            form.set(form_data);
        })
    };

    let on_is_available_change = {
        let form = form.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.is_available = input.checked();
            form.set(form_data);
        })
    };

    let on_image_select_change = {
        let form = form.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            if select.value().is_empty() {
                form_data.selected_image = None;
            } else {
                if let Ok(uuid) = select.value().parse::<uuid::Uuid>() {
                    form_data.selected_image = Some(SiteImageId(uuid));
                }
            }
            form.set(form_data);
        })
    };

    let on_submit_space = {
        let form = form.clone();
        let spaces = spaces.clone();
        let show_form = show_form.clone();
        let site_id = props.site_id.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let form_data = (*form).clone();

            // Validation
            if form_data.name.trim().is_empty() {
                let mut new_form = form_data;
                new_form.error = Some("Space name is required".to_string());
                form.set(new_form);
                return;
            }

            let eligibility_points = match form_data
                .eligibility_points
                .parse::<f64>()
            {
                Ok(points) if points >= 0.0 => points,
                _ => {
                    let mut new_form = form_data;
                    new_form.error = Some("Eligibility points must be a valid non-negative number".to_string());
                    form.set(new_form);
                    return;
                }
            };

            let form = form.clone();
            let spaces = spaces.clone();
            let show_form = show_form.clone();
            let site_id = site_id.clone();

            yew::platform::spawn_local(async move {
                // Set loading state
                {
                    let mut new_form = (*form).clone();
                    new_form.is_loading = true;
                    new_form.error = None;
                    form.set(new_form);
                }

                let client = get_api_client();
                let form_data = (*form).clone();

                let space_details = payloads::Space {
                    site_id,
                    name: form_data.name.trim().to_string(),
                    description: if form_data.description.trim().is_empty() {
                        None
                    } else {
                        Some(form_data.description.trim().to_string())
                    },
                    eligibility_points,
                    is_available: form_data.is_available,
                    site_image_id: form_data.selected_image,
                };

                match client.create_space(&space_details).await {
                    Ok(_space_id) => {
                        // Refresh the spaces list
                        match client.list_spaces(&site_id).await {
                            Ok(updated_spaces) => {
                                spaces.set(updated_spaces);
                                show_form.set(false);
                                form.set(SpaceForm::default());
                            }
                            Err(_) => {
                                let mut new_form = (*form).clone();
                                new_form.error = Some(
                                    "Space created but failed to refresh list"
                                        .to_string(),
                                );
                                new_form.is_loading = false;
                                form.set(new_form);
                            }
                        }
                    }
                    Err(e) => {
                        let mut new_form = (*form).clone();
                        new_form.error =
                            Some(format!("Failed to create space: {}", e));
                        new_form.is_loading = false;
                        form.set(new_form);
                    }
                }
            });
        })
    };

    html! {
        <div>
            <div class="flex items-center justify-between mb-4">
                <h3 class="text-lg font-medium text-gray-900 dark:text-white">
                    {"Spaces"}
                    <span class="ml-2 text-sm text-gray-500 dark:text-gray-400">
                        {"("}{spaces.len()}{" total, "}{site_images.len()}{" images loaded)"}
                    </span>
                </h3>
                if !*show_form {
                    <button
                        type="button"
                        onclick={on_add_space_click}
                        class="inline-flex items-center px-3 py-2 border border-transparent text-sm leading-4 font-medium rounded-md text-white bg-green-600 hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500"
                    >
                        <svg class="-ml-0.5 mr-2 h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
                        </svg>
                        {"Add Space"}
                    </button>
                }
            </div>

            // Existing spaces list
            if spaces.is_empty() && !*show_form {
                <div class="text-center py-6 text-gray-500 dark:text-gray-400">
                    <svg class="mx-auto h-12 w-12 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4"></path>
                    </svg>
                    <p class="mt-2">{"No spaces added yet"}</p>
                </div>
            } else if !spaces.is_empty() {
                <div class="space-y-3 mb-6">
                    {for spaces.iter().enumerate().map(|(_index, space)| {
                        let spaces_for_edit = spaces.clone();
                        let spaces_for_delete = spaces.clone();

                        let space_id_for_update = space.space_id;
                        let space_id_for_delete = space.space_id;

                        html! {
                            if let Some(comm_id) = *community_id {
                                <SpaceDisplayCard
                                    key={format!("{}", space.space_id.0)}
                                    space={space.clone()}
                                    community_id={comm_id}
                                    on_updated={Callback::from(move |updated_space: responses::Space| {
                                        let mut current_spaces = (*spaces_for_edit).clone();
                                        if let Some(idx) = current_spaces.iter().position(|s| s.space_id == space_id_for_update) {
                                            current_spaces[idx] = updated_space;
                                            spaces_for_edit.set(current_spaces);
                                        }
                                    })}
                                    on_deleted={Callback::from(move |_deleted_space_id: payloads::SpaceId| {
                                        let mut current_spaces = (*spaces_for_delete).clone();
                                        current_spaces.retain(|s| s.space_id != space_id_for_delete);
                                        spaces_for_delete.set(current_spaces);
                                    })}
                                />
                            }
                        }
                    })}
                </div>
            }

            // Add space form
            if *show_form {
                <div class="bg-gray-50 dark:bg-gray-700 rounded-lg p-6">
                    <h4 class="text-lg font-medium text-gray-900 dark:text-white mb-4">{"Add New Space"}</h4>

                    <form onsubmit={on_submit_space}>
                        // Form error message
                        if let Some(error) = &form.error {
                            <div class="mb-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded">
                                {error}
                            </div>
                        }

                        <div class="grid grid-cols-1 gap-4">
                            // Space Name
                            <div>
                                <label for="space_name" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                    {"Space Name"}
                                    <span class="text-red-500 ml-1">{"*"}</span>
                                </label>
                                <input
                                    type="text"
                                    id="space_name"
                                    name="space_name"
                                    required=true
                                    class="mt-1 block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm placeholder-gray-400 focus:outline-none focus:ring-green-500 focus:border-green-500 dark:bg-gray-600 dark:text-white sm:text-sm"
                                    placeholder="Enter space name"
                                    value={form.name.clone()}
                                    oninput={on_name_change}
                                    disabled={form.is_loading}
                                />
                            </div>

                            // Description
                            <div>
                                <label for="space_description" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                    {"Description"}
                                </label>
                                <textarea
                                    id="space_description"
                                    name="space_description"
                                    rows="2"
                                    class="mt-1 block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm placeholder-gray-400 focus:outline-none focus:ring-green-500 focus:border-green-500 dark:bg-gray-600 dark:text-white sm:text-sm"
                                    placeholder="Optional description of the space"
                                    value={form.description.clone()}
                                    oninput={on_description_change}
                                    disabled={form.is_loading}
                                />
                            </div>

                            // Eligibility Points
                            <div>
                                <label for="eligibility_points" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                    {"Eligibility Points"}
                                    <span class="text-red-500 ml-1">{"*"}</span>
                                </label>
                                <input
                                    type="number"
                                    id="eligibility_points"
                                    name="eligibility_points"
                                    required=true
                                    min="0"
                                    step="0.1"
                                    class="mt-1 block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm placeholder-gray-400 focus:outline-none focus:ring-green-500 focus:border-green-500 dark:bg-gray-600 dark:text-white sm:text-sm"
                                    placeholder="0.0"
                                    value={form.eligibility_points.clone()}
                                    oninput={on_eligibility_points_change}
                                    disabled={form.is_loading}
                                />
                            </div>

                            // Availability checkbox
                            <div class="flex items-center">
                                <input
                                    id="is_available"
                                    name="is_available"
                                    type="checkbox"
                                    class="h-4 w-4 text-green-600 focus:ring-green-500 border-gray-300 dark:border-gray-600 rounded dark:bg-gray-600"
                                    checked={form.is_available}
                                    onchange={on_is_available_change}
                                    disabled={form.is_loading}
                                />
                                <label for="is_available" class="ml-2 block text-sm text-gray-900 dark:text-white">
                                    {"Space is available for bidding"}
                                </label>
                            </div>

                            // Site Image Selection
                            if !site_images.is_empty() {
                                <div>
                                    <label for="space_image" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                        {"Site Image"}
                                    </label>
                                    <select
                                        id="space_image"
                                        name="space_image"
                                        class="mt-1 block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-green-500 focus:border-green-500 dark:bg-gray-600 dark:text-white sm:text-sm"
                                        value={form.selected_image.map(|id| id.0.to_string()).unwrap_or_default()}
                                        onchange={on_image_select_change}
                                        disabled={form.is_loading}
                                    >
                                        <option value="">{"No image selected"}</option>
                                        {for site_images.iter().map(|image| {
                                            html! {
                                                <option value={image.id.0.to_string()}>
                                                    {&image.name}
                                                </option>
                                            }
                                        })}
                                    </select>
                                </div>
                            }
                        </div>

                        // Form buttons
                        <div class="flex justify-end space-x-3 mt-6">
                            <button
                                type="button"
                                onclick={on_cancel_add}
                                class="px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-600 hover:bg-gray-50 dark:hover:bg-gray-500 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500"
                                disabled={form.is_loading}
                            >
                                {"Cancel"}
                            </button>
                            <button
                                type="submit"
                                class="px-4 py-2 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-green-600 hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500 disabled:opacity-50"
                                disabled={form.is_loading}
                            >
                                if form.is_loading {
                                    {"Creating..."}
                                } else {
                                    {"Create Space"}
                                }
                            </button>
                        </div>
                    </form>
                </div>
            }
        </div>
    }
}

// ============================================================================
// Space Display Card Component (with edit/delete functionality)
// ============================================================================

#[derive(Properties)]
pub struct SpaceDisplayCardProps {
    pub space: responses::Space,
    pub community_id: CommunityId,
    pub on_updated: Callback<responses::Space>,
    pub on_deleted: Callback<payloads::SpaceId>,
}

impl PartialEq for SpaceDisplayCardProps {
    fn eq(&self, other: &Self) -> bool {
        self.space.space_id == other.space.space_id
    }
}

#[derive(Clone)]
struct EditSpaceForm {
    name: String,
    description: String,
    eligibility_points: String,
    is_available: bool,
    selected_image: Option<SiteImageId>,
    is_loading: bool,
    error: Option<String>,
}

impl From<&payloads::Space> for EditSpaceForm {
    fn from(space: &payloads::Space) -> Self {
        Self {
            name: space.name.clone(),
            description: space.description.clone().unwrap_or_default(),
            eligibility_points: space.eligibility_points.to_string(),
            is_available: space.is_available,
            selected_image: space.site_image_id,
            is_loading: false,
            error: None,
        }
    }
}

#[function_component]
pub fn SpaceDisplayCard(props: &SpaceDisplayCardProps) -> Html {
    let is_editing = use_state(|| false);
    let is_deleting = use_state(|| false);
    let edit_form =
        use_state(|| EditSpaceForm::from(&props.space.space_details));
    let site_images = use_state(Vec::<responses::SiteImage>::new);
    let space_image = use_state(|| None::<responses::SiteImage>);

    let space = &props.space;

    // Load site images when component mounts or when editing starts
    {
        let site_images = site_images.clone();
        let space_image = space_image.clone();
        let community_id = props.community_id.clone();
        let current_image_id = space.space_details.site_image_id;

        use_effect_with(props.community_id, move |_| {
            let site_images = site_images.clone();
            let space_image = space_image.clone();
            let community_id = community_id.clone();

            yew::platform::spawn_local(async move {
                let client = get_api_client();
                match client.list_site_images(&community_id).await {
                    Ok(images) => {
                        // Find the current space's image if it has one
                        let current_image =
                            if let Some(image_id) = current_image_id {
                                images
                                    .iter()
                                    .find(|img| img.id == image_id)
                                    .cloned()
                            } else {
                                None
                            };

                        site_images.set(images);
                        space_image.set(current_image);
                    }
                    Err(_) => {
                        site_images.set(Vec::new());
                        space_image.set(None);
                    }
                }
            });
            || ()
        });
    }

    let on_edit_click = {
        let is_editing = is_editing.clone();
        let edit_form = edit_form.clone();
        let space_details = space.space_details.clone();

        Callback::from(move |_: MouseEvent| {
            edit_form.set(EditSpaceForm::from(&space_details));
            is_editing.set(true);
        })
    };

    let on_delete_click = {
        let is_deleting = is_deleting.clone();
        let on_deleted = props.on_deleted.clone();
        let space_id = space.space_id;
        let space_name = space.space_details.name.clone();

        Callback::from(move |_: MouseEvent| {
            let space_name = space_name.clone();
            if window().unwrap().confirm_with_message(&format!("Are you sure you want to delete the space '{}'? This action cannot be undone.", space_name)).unwrap_or(false) {
                let is_deleting = is_deleting.clone();
                let on_deleted = on_deleted.clone();

                is_deleting.set(true);

                yew::platform::spawn_local(async move {
                    let client = get_api_client();
                    match client.delete_space(&space_id).await {
                        Ok(_) => {
                            on_deleted.emit(space_id);
                        }
                        Err(_) => {
                            is_deleting.set(false);
                        }
                    }
                });
            }
        })
    };

    let on_cancel_edit = {
        let is_editing = is_editing.clone();
        let edit_form = edit_form.clone();
        let space_details = space.space_details.clone();
        Callback::from(move |_: MouseEvent| {
            edit_form.set(EditSpaceForm::from(&space_details));
            is_editing.set(false);
        })
    };

    let on_name_change = {
        let edit_form = edit_form.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*edit_form).clone();
            form_data.name = input.value();
            edit_form.set(form_data);
        })
    };

    let on_description_change = {
        let edit_form = edit_form.clone();
        Callback::from(move |e: InputEvent| {
            let textarea: HtmlTextAreaElement = e.target_unchecked_into();
            let mut form_data = (*edit_form).clone();
            form_data.description = textarea.value();
            edit_form.set(form_data);
        })
    };

    let on_eligibility_points_change = {
        let edit_form = edit_form.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*edit_form).clone();
            form_data.eligibility_points = input.value();
            edit_form.set(form_data);
        })
    };

    let on_is_available_change = {
        let edit_form = edit_form.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*edit_form).clone();
            form_data.is_available = input.checked();
            edit_form.set(form_data);
        })
    };

    let on_image_select_change = {
        let edit_form = edit_form.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            let mut form_data = (*edit_form).clone();
            if select.value().is_empty() {
                form_data.selected_image = None;
            } else {
                if let Ok(uuid) = select.value().parse::<uuid::Uuid>() {
                    form_data.selected_image = Some(SiteImageId(uuid));
                }
            }
            edit_form.set(form_data);
        })
    };

    let on_save_edit = {
        let edit_form = edit_form.clone();
        let is_editing = is_editing.clone();
        let on_updated = props.on_updated.clone();
        let space_id = space.space_id;
        let site_id = space.space_details.site_id;

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let form_data = (*edit_form).clone();

            // Validation
            if form_data.name.trim().is_empty() {
                let mut new_form = form_data;
                new_form.error = Some("Space name is required".to_string());
                edit_form.set(new_form);
                return;
            }

            let eligibility_points = match form_data
                .eligibility_points
                .parse::<f64>()
            {
                Ok(points) if points >= 0.0 => points,
                _ => {
                    let mut new_form = form_data;
                    new_form.error = Some("Eligibility points must be a valid non-negative number".to_string());
                    edit_form.set(new_form);
                    return;
                }
            };

            let edit_form = edit_form.clone();
            let is_editing = is_editing.clone();
            let on_updated = on_updated.clone();

            yew::platform::spawn_local(async move {
                // Set loading state
                {
                    let mut new_form = (*edit_form).clone();
                    new_form.is_loading = true;
                    new_form.error = None;
                    edit_form.set(new_form);
                }

                let client = get_api_client();
                let form_data = (*edit_form).clone();

                let space_details = payloads::Space {
                    site_id,
                    name: form_data.name.trim().to_string(),
                    description: if form_data.description.trim().is_empty() {
                        None
                    } else {
                        Some(form_data.description.trim().to_string())
                    },
                    eligibility_points,
                    is_available: form_data.is_available,
                    site_image_id: form_data.selected_image,
                };

                let update_request = payloads::requests::UpdateSpace {
                    space_id,
                    space_details,
                };

                match client.update_space(&update_request).await {
                    Ok(updated_space) => {
                        is_editing.set(false);
                        on_updated.emit(updated_space);
                    }
                    Err(e) => {
                        let mut new_form = (*edit_form).clone();
                        new_form.error =
                            Some(format!("Failed to update space: {}", e));
                        new_form.is_loading = false;
                        edit_form.set(new_form);
                    }
                }
            });
        })
    };

    html! {
        <div class="bg-gray-50 dark:bg-gray-700 rounded-lg p-4">
            if *is_editing {
                // Edit form
                <form onsubmit={on_save_edit}>
                    if let Some(error) = &edit_form.error {
                        <div class="mb-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-3 py-2 rounded text-sm">
                            {error}
                        </div>
                    }

                    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                        // Left column - image preview
                        <div>
                            {
                                // Show preview of selected image from form, not the current space image
                                if let Some(selected_id) = edit_form.selected_image {
                                    if let Some(img) = site_images.iter().find(|img| img.id == selected_id) {
                                        html! {
                                            <>
                                                <div class="aspect-square w-full max-w-xs rounded-lg overflow-hidden bg-gray-100 dark:bg-gray-600">
                                                    <img
                                                        src={format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(&img.image_data))}
                                                        alt={img.name.clone()}
                                                        class="w-full h-full object-cover"
                                                    />
                                                </div>
                                                <p class="mt-2 text-sm text-gray-600 dark:text-gray-400">
                                                    {"Selected image: "}{&img.name}
                                                </p>
                                            </>
                                        }
                                    } else {
                                        html! {
                                            <>
                                                <div class="aspect-square w-full max-w-xs rounded-lg bg-gray-200 dark:bg-gray-600 flex items-center justify-center">
                                                    <svg class="h-16 w-16 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
                                                    </svg>
                                                </div>
                                                <p class="mt-2 text-sm text-gray-600 dark:text-gray-400">
                                                    {"No image selected"}
                                                </p>
                                            </>
                                        }
                                    }
                                } else {
                                    html! {
                                        <>
                                            <div class="aspect-square w-full max-w-xs rounded-lg bg-gray-200 dark:bg-gray-600 flex items-center justify-center">
                                                <svg class="h-16 w-16 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
                                                </svg>
                                            </div>
                                            <p class="mt-2 text-sm text-gray-600 dark:text-gray-400">
                                                {"No image selected"}
                                            </p>
                                        </>
                                    }
                                }
                            }
                        </div>

                        // Right column - form fields
                        <div class="space-y-4">
                            <div>
                                <label for="edit_space_name" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                    {"Space Name"}
                                    <span class="text-red-500 ml-1">{"*"}</span>
                                </label>
                                <input
                                    type="text"
                                    id="edit_space_name"
                                    required=true
                                    class="mt-1 block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm focus:outline-none focus:ring-green-500 focus:border-green-500 dark:bg-gray-600 dark:text-white"
                                    value={edit_form.name.clone()}
                                    oninput={on_name_change}
                                    disabled={edit_form.is_loading}
                                />
                            </div>

                            <div>
                                <label for="edit_space_description" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                    {"Description"}
                                </label>
                                <textarea
                                    id="edit_space_description"
                                    rows="2"
                                    class="mt-1 block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm focus:outline-none focus:ring-green-500 focus:border-green-500 dark:bg-gray-600 dark:text-white"
                                    value={edit_form.description.clone()}
                                    oninput={on_description_change}
                                    disabled={edit_form.is_loading}
                                />
                            </div>

                            <div>
                                <label for="edit_eligibility_points" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                    {"Eligibility Points"}
                                    <span class="text-red-500 ml-1">{"*"}</span>
                                </label>
                                <input
                                    type="number"
                                    id="edit_eligibility_points"
                                    required=true
                                    min="0"
                                    step="0.1"
                                    class="mt-1 block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm focus:outline-none focus:ring-green-500 focus:border-green-500 dark:bg-gray-600 dark:text-white"
                                    value={edit_form.eligibility_points.clone()}
                                    oninput={on_eligibility_points_change}
                                    disabled={edit_form.is_loading}
                                />
                            </div>

                            <div class="flex items-center">
                                <input
                                    id="edit_is_available"
                                    type="checkbox"
                                    class="h-4 w-4 text-green-600 focus:ring-green-500 border-gray-300 dark:border-gray-600 rounded dark:bg-gray-600"
                                    checked={edit_form.is_available}
                                    onchange={on_is_available_change}
                                    disabled={edit_form.is_loading}
                                />
                                <label for="edit_is_available" class="ml-2 block text-sm text-gray-900 dark:text-white">
                                    {"Available for bidding"}
                                </label>
                            </div>

                            <div>
                                <label for="edit_space_image" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                    {"Site Image"}
                                </label>
                                if !site_images.is_empty() {
                                    <select
                                        id="edit_space_image"
                                        class="mt-1 block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm focus:outline-none focus:ring-green-500 focus:border-green-500 dark:bg-gray-600 dark:text-white"
                                        value={edit_form.selected_image.map(|id| id.0.to_string()).unwrap_or_default()}
                                        onchange={on_image_select_change}
                                        disabled={edit_form.is_loading}
                                    >
                                        <option value="">{"No image"}</option>
                                        {for site_images.iter().map(|image| {
                                            html! {
                                                <option value={image.id.0.to_string()}>
                                                    {&image.name}
                                                </option>
                                            }
                                        })}
                                    </select>
                                } else {
                                    <div class="mt-1 px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-gray-100 dark:bg-gray-600 text-gray-500 dark:text-gray-400 text-sm">
                                        {"Loading site images..."}
                                    </div>
                                }
                            </div>
                        </div>
                    </div>

                    <div class="flex justify-end space-x-3 mt-6">
                        <button
                            type="button"
                            onclick={on_cancel_edit}
                            class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-600 hover:bg-gray-50 dark:hover:bg-gray-500 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500"
                            disabled={edit_form.is_loading}
                        >
                            {"Cancel"}
                        </button>
                        <button
                            type="submit"
                            class="px-3 py-2 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-green-600 hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500 disabled:opacity-50"
                            disabled={edit_form.is_loading}
                        >
                            if edit_form.is_loading {
                                {"Saving..."}
                            } else {
                                {"Save Changes"}
                            }
                        </button>
                    </div>
                </form>
            } else {
                // Display mode
                <div class="flex items-start justify-between">
                    <div class="flex-1 flex space-x-4">
                        // Space image thumbnail
                        <div class="flex-shrink-0">
                            if let Some(img) = &*space_image {
                                <div class="w-16 h-16 rounded-lg overflow-hidden bg-gray-100 dark:bg-gray-600">
                                    <img
                                        src={format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(&img.image_data))}
                                        alt={img.name.clone()}
                                        class="w-full h-full object-cover"
                                    />
                                </div>
                            } else {
                                <div class="w-16 h-16 rounded-lg bg-gray-200 dark:bg-gray-600 flex items-center justify-center">
                                    <svg class="h-8 w-8 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
                                    </svg>
                                </div>
                            }
                        </div>

                        // Space details
                        <div class="flex-1">
                            <h4 class="font-medium text-gray-900 dark:text-white">{&space.space_details.name}</h4>
                            if let Some(description) = &space.space_details.description {
                                <p class="mt-1 text-sm text-gray-600 dark:text-gray-300">{description}</p>
                            }
                            <div class="mt-2 flex items-center flex-wrap gap-4 text-sm text-gray-500 dark:text-gray-400">
                                <span>{"Eligibility Points: "}{space.space_details.eligibility_points}</span>
                                <span class={if space.space_details.is_available { "text-green-600" } else { "text-red-600" }}>
                                    {if space.space_details.is_available { "Available" } else { "Unavailable" }}
                                </span>
                                if space_image.is_some() {
                                    <span class="inline-flex items-center text-blue-600">
                                        <svg class="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
                                        </svg>
                                        {"Image attached"}
                                    </span>
                                }
                            </div>
                        </div>
                    </div>

                    // Action buttons
                    <div class="flex space-x-2 ml-4">
                        <button
                            onclick={on_edit_click}
                            type="button" // avoid form submission in UI testing
                            class="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
                            title="Edit space"
                        >
                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                            </svg>
                        </button>
                        <button
                            onclick={on_delete_click}
                            class="p-2 text-gray-400 hover:text-red-600 dark:hover:text-red-400 transition-colors"
                            title="Delete space"
                            disabled={*is_deleting}
                        >
                            if *is_deleting {
                                <svg class="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                </svg>
                            } else {
                                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                                </svg>
                            }
                        </button>
                    </div>
                </div>
            }
        </div>
    }
}
