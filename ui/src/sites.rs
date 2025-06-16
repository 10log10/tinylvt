use base64::Engine;
use payloads::{requests, responses, CommunityId, SiteImageId};
use web_sys::{HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement, File};
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
                        let images_future = client.list_site_images(&community_id);
                        
                        match futures::future::try_join(sites_future, images_future).await {
                            Ok((sites, images)) => {
                                let mut state = (*sites_state).clone();
                                state.sites = sites;
                                state.site_images = images;
                                state.is_loading = false;
                                sites_state.set(state);
                            }
                            Err(e) => {
                                let mut state = (*sites_state).clone();
                                state.error = Some(format!("Failed to load data: {}", e));
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
                                html! {
                                    <SiteImageCard
                                        key={format!("{}", image.id.0)}
                                        image={image.clone()}
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
}

impl PartialEq for SiteImageCardProps {
    fn eq(&self, other: &Self) -> bool {
        self.image.id == other.image.id
    }
}

#[function_component]
pub fn SiteImageCard(props: &SiteImageCardProps) -> Html {
    let image = &props.image;

    // Convert image data to base64 for display
    let image_src = format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&image.image_data)
    );

    html! {
        <div class="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
            <div class="aspect-w-16 aspect-h-9">
                <img
                    src={image_src}
                    alt={image.name.clone()}
                    class="w-full h-32 object-cover"
                />
            </div>
            <div class="p-3">
                <h4 class="text-sm font-medium text-gray-900 dark:text-white truncate">
                    {&image.name}
                </h4>
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
            
            if let Some(files) = input.files() {
                if files.length() > 0 {
                    if let Some(file) = files.get(0) {
                        form_data.image_file = Some(file);
                        form_data.selected_image = None; // Clear existing image selection
                    }
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
            if form_data.image_file.is_some() && form_data.image_name.trim().is_empty() {
                let mut new_form = form_data;
                new_form.error = Some("Image name is required when uploading an image".to_string());
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
                let site_image_id = if let Some(image_file) = form_data.image_file {
                    // Read file data
                    match read_file_as_bytes(&image_file).await {
                        Ok(image_data) => {
                            let create_image_request = requests::CreateSiteImage {
                                community_id: community_id.clone(),
                                name: form_data.image_name.trim().to_string(),
                                image_data,
                            };

                            match client.create_site_image(&create_image_request).await {
                                Ok(image_id) => Some(image_id),
                                Err(e) => {
                                    let mut new_form = (*form).clone();
                                    new_form.is_loading = false;
                                    new_form.error = Some(format!("Failed to upload image: {}", e));
                                    form.set(new_form);
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            let mut new_form = (*form).clone();
                            new_form.is_loading = false;
                            new_form.error = Some(format!("Failed to read image file: {}", e));
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
                    auction_lead_time: jiff::Span::new().days(7), // 7 days
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
                        new_form.error = Some(format!("Failed to create site: {}", e));
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
// Site Details Component (Placeholder)
// ============================================================================

#[derive(Properties, PartialEq)]
pub struct SiteDetailsProps {
    pub site_id: String,
}

#[function_component]
pub fn SiteDetails(props: &SiteDetailsProps) -> Html {
    html! {
        <main class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
            <div class="text-center">
                <h1 class="text-3xl font-bold text-gray-900 dark:text-white">{"Site Details"}</h1>
                <p class="text-gray-600 dark:text-gray-300">{"Site ID: "}{&props.site_id}</p>
                <p class="text-gray-500 dark:text-gray-400 mt-4">{"Coming soon..."}</p>
            </div>
        </main>
    }
}

// ============================================================================
// Edit Site Component (Placeholder)
// ============================================================================

#[derive(Properties, PartialEq)]
pub struct EditSiteProps {
    pub site_id: String,
}

#[function_component]
pub fn EditSite(props: &EditSiteProps) -> Html {
    html! {
        <main class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
            <div class="text-center">
                <h1 class="text-3xl font-bold text-gray-900 dark:text-white">{"Edit Site"}</h1>
                <p class="text-gray-600 dark:text-gray-300">{"Site ID: "}{&props.site_id}</p>
                <p class="text-gray-500 dark:text-gray-400 mt-4">{"Coming soon..."}</p>
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

    let file_reader = FileReader::new().map_err(|_| "Failed to create FileReader")?;
    
    let file_reader_clone = file_reader.clone();
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let file_reader_for_closure = file_reader_clone.clone();
        let onload = wasm_bindgen::closure::Closure::wrap(Box::new(move |_: web_sys::Event| {
            if let Ok(array_buffer) = file_reader_for_closure.result() {
                resolve.call1(&wasm_bindgen::JsValue::NULL, &array_buffer).unwrap();
            } else {
                reject.call1(&wasm_bindgen::JsValue::NULL, &wasm_bindgen::JsValue::from_str("Failed to read file")).unwrap();
            }
        }) as Box<dyn FnMut(_)>);
        
        file_reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();
    });

    file_reader.read_as_array_buffer(file).map_err(|_| "Failed to start reading file")?;
    
    let result = JsFuture::from(promise).await.map_err(|_| "Failed to read file")?;
    let array_buffer: js_sys::ArrayBuffer = result.dyn_into().map_err(|_| "Invalid file data")?;
    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
    
    Ok(uint8_array.to_vec())
}

fn get_timezone_options() -> Vec<String> {
    // Get all available timezones from jiff's timezone database
    let mut timezones: Vec<String> = jiff::tz::db().available()
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