use payloads::{requests, responses};
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::{Route, auth::use_auth, get_api_client};

#[derive(Default, Clone, PartialEq)]
pub struct CommunitiesState {
    pub communities: Vec<responses::Community>,
    pub is_loading: bool,
    pub error: Option<String>,
}

#[function_component]
pub fn Communities() -> Html {
    let (auth_state, _) = use_auth();
    let navigator = use_navigator().unwrap();
    let communities_state = use_state(CommunitiesState::default);

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

    // Load communities when component mounts
    {
        let communities_state = communities_state.clone();
        let auth_state = auth_state.clone();

        use_effect_with(auth_state.is_authenticated, move |is_authenticated| {
            if *is_authenticated {
                let communities_state = communities_state.clone();

                yew::platform::spawn_local(async move {
                    let mut state = (*communities_state).clone();
                    state.is_loading = true;
                    state.error = None;
                    communities_state.set(state);

                    let client = get_api_client();
                    match client.get_communities().await {
                        Ok(communities) => {
                            let mut state = (*communities_state).clone();
                            state.communities = communities;
                            state.is_loading = false;
                            communities_state.set(state);
                        }
                        Err(e) => {
                            let mut state = (*communities_state).clone();
                            state.error = Some(format!(
                                "Failed to load communities: {}",
                                e
                            ));
                            state.is_loading = false;
                            communities_state.set(state);
                        }
                    }
                });
            }
            || ()
        });
    }

    let on_create_community = {
        let navigator = navigator.clone();

        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::CreateCommunity);
        })
    };

    let on_join_community = {
        let navigator = navigator.clone();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::CommunityInvites);
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
                        <h1 class="text-2xl sm:text-3xl font-bold text-gray-900 dark:text-white">{"My Communities"}</h1>
                        <p class="mt-1 sm:mt-2 text-sm sm:text-base text-gray-600 dark:text-gray-300">
                            {"Manage your community memberships and create new communities"}
                        </p>
                    </div>
                    <div class="flex flex-col sm:flex-row gap-3 sm:space-x-3 sm:gap-0 flex-shrink-0">
                        <button
                            onclick={on_join_community.clone()}
                            class="inline-flex items-center justify-center px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 hover:bg-gray-50 dark:hover:bg-gray-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                        >
                            <svg class="-ml-1 mr-2 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
                            </svg>
                            <span class="sm:hidden">{"Join"}</span>
                            <span class="hidden sm:inline">{"Join Community"}</span>
                        </button>
                        <button
                            onclick={on_create_community.clone()}
                            class="inline-flex items-center justify-center px-4 py-2 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                        >
                            <svg class="-ml-1 mr-2 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
                            </svg>
                            <span class="sm:hidden">{"Create"}</span>
                            <span class="hidden sm:inline">{"Create Community"}</span>
                        </button>
                    </div>
                </div>

                // Error message
                if let Some(error) = &communities_state.error {
                    <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded">
                        {error}
                    </div>
                }

                // Loading state
                if communities_state.is_loading {
                    <div class="text-center py-8">
                        <svg class="animate-spin h-8 w-8 text-blue-600 mx-auto" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                        </svg>
                        <p class="mt-2 text-gray-600 dark:text-gray-400">{"Loading communities..."}</p>
                    </div>
                } else if communities_state.communities.is_empty() {
                    // Empty state
                    <div class="text-center py-12">
                        <div class="mx-auto h-12 w-12 text-gray-400">
                            <svg fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z"></path>
                            </svg>
                        </div>
                        <h3 class="mt-2 text-sm font-medium text-gray-900 dark:text-white">{"No communities"}</h3>
                        <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
                            {"You're not a member of any communities yet. Create your first community or join an existing one."}
                        </p>
                        <div class="mt-6 flex flex-col sm:flex-row justify-center gap-3 sm:space-x-3 sm:gap-0">
                            <button
                                onclick={on_create_community}
                                class="inline-flex items-center justify-center px-4 py-2 border border-transparent shadow-sm text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                            >
                                {"Create your first community"}
                            </button>
                            <button
                                onclick={on_join_community}
                                class="inline-flex items-center justify-center px-4 py-2 border border-gray-300 dark:border-gray-600 shadow-sm text-sm font-medium rounded-md text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 hover:bg-gray-50 dark:hover:bg-gray-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                            >
                                {"Join a community"}
                            </button>
                        </div>
                    </div>
                } else {
                    // Communities grid
                    <div class="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
                        {for communities_state.communities.iter().map(|community| {
                            let _community_id = community.id.clone();
                            html! {
                                <CommunityCard
                                    key={format!("{}", community.id.0)}
                                    community={community.clone()}
                                />
                            }
                        })}
                    </div>
                }
            </div>
        </main>
    }
}

#[derive(Properties, PartialEq)]
pub struct CommunityCardProps {
    pub community: responses::Community,
}

#[function_component]
pub fn CommunityCard(props: &CommunityCardProps) -> Html {
    let navigator = use_navigator().unwrap();
    let community = &props.community;

    let on_click = {
        let navigator = navigator.clone();
        let community_id = community.id.clone();

        Callback::from(move |_: MouseEvent| {
            // Navigate to community dashboard page
            navigator.push(&Route::CommunityDashboard {
                id: community_id.0.to_string(),
            });
        })
    };

    // Format the creation date
    let created_date = {
        // For now, just show a placeholder
        // TODO: Format the jiff::Timestamp properly
        "Recently created".to_string()
    };

    let first_letter = community
        .name
        .chars()
        .next()
        .unwrap_or('C')
        .to_uppercase()
        .to_string();

    html! {
        <div
            onclick={on_click}
            class="relative group bg-white dark:bg-gray-800 p-6 rounded-lg border border-gray-200 dark:border-gray-700 cursor-pointer hover:shadow-md hover:border-gray-300 dark:hover:border-gray-600 transition-all duration-200"
        >
            <div class="flex items-center justify-between">
                <div class="flex items-center space-x-3">
                    <div class="flex-shrink-0">
                        <div class="w-10 h-10 bg-blue-500 rounded-lg flex items-center justify-center">
                            <span class="text-white font-medium text-lg">
                                {first_letter}
                            </span>
                        </div>
                    </div>
                    <div class="min-w-0 flex-1">
                        <h3 class="text-lg font-medium text-gray-900 dark:text-white group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors">
                            {&community.name}
                        </h3>
                        <p class="text-sm text-gray-500 dark:text-gray-400">
                            {created_date}
                        </p>
                    </div>
                </div>

                // Role badge placeholder - we'll add this when we have role information
                <div class="flex-shrink-0">
                    <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-gray-100 dark:bg-gray-700 text-gray-800 dark:text-gray-300">
                        {"Member"} // Placeholder - will be dynamic when we have role data
                    </span>
                </div>
            </div>

            // Remove active/inactive status display for MVP
        </div>
    }
}

// Create Community Form State
#[derive(Clone, PartialEq)]
struct CreateCommunityForm {
    name: String,
    new_members_default_active: bool,
    is_loading: bool,
    error: Option<String>,
}

impl Default for CreateCommunityForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            new_members_default_active: true, // Always default to true for MVP
            is_loading: false,
            error: None,
        }
    }
}

#[function_component]
pub fn CreateCommunity() -> Html {
    let (auth_state, _) = use_auth();
    let navigator = use_navigator().unwrap();
    let form = use_state(CreateCommunityForm::default);

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

    let on_name_change = {
        let form = form.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*form).clone();
            form_data.name = input.value();
            form.set(form_data);
        })
    };

    let on_cancel = {
        let navigator = navigator.clone();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::Communities);
        })
    };

    let on_submit = {
        let form = form.clone();
        let navigator = navigator.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let form_data = (*form).clone();

            // Validation
            if form_data.name.trim().is_empty() {
                let mut new_form = form_data;
                new_form.error = Some("Community name is required".to_string());
                form.set(new_form);
                return;
            }

            if form_data.name.trim().len() > 255 {
                let mut new_form = form_data;
                new_form.error = Some(
                    "Community name must be 255 characters or less".to_string(),
                );
                form.set(new_form);
                return;
            }

            let form = form.clone();
            let navigator = navigator.clone();
            let name = form_data.name.trim().to_string();
            let new_members_default_active = true; // Always true for MVP

            yew::platform::spawn_local(async move {
                // Set loading state
                {
                    let mut new_form = (*form).clone();
                    new_form.is_loading = true;
                    new_form.error = None;
                    form.set(new_form);
                }

                let client = get_api_client();
                let community_details = requests::CreateCommunity {
                    name,
                    new_members_default_active,
                };

                match client.create_community(&community_details).await {
                    Ok(_community_id) => {
                        // Community created successfully, navigate back to communities list
                        navigator.push(&Route::Communities);
                    }
                    Err(e) => {
                        let mut new_form = (*form).clone();
                        new_form.is_loading = false;
                        new_form.error =
                            Some(format!("Failed to create community: {}", e));
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
                                    <span class="ml-4 text-sm font-medium text-gray-500 dark:text-gray-400">{"Create Community"}</span>
                                </div>
                            </li>
                        </ol>
                    </nav>
                    <h1 class="mt-4 text-3xl font-bold text-gray-900 dark:text-white">{"Create a New Community"}</h1>
                    <p class="mt-2 text-gray-600 dark:text-gray-300">
                        {"Set up your community to start managing shared spaces and resources."}
                    </p>
                </div>

                // Form
                <div class="bg-white dark:bg-gray-800 shadow-sm rounded-lg">
                    <form onsubmit={on_submit} class="space-y-6 p-6">
                        // Community Name
                        <div>
                            <label for="name" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                {"Community Name"}
                                <span class="text-red-500 ml-1">{"*"}</span>
                            </label>
                            <div class="mt-1">
                                <input
                                    type="text"
                                    id="name"
                                    name="name"
                                    required=true
                                    maxlength="255"
                                    class="block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm placeholder-gray-400 focus:outline-none focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:text-white sm:text-sm"
                                    placeholder="Enter a name for your community"
                                    value={form.name.clone()}
                                    oninput={on_name_change}
                                    disabled={form.is_loading}
                                />
                            </div>
                            <p class="mt-2 text-sm text-gray-500 dark:text-gray-400">
                                {"Choose a descriptive name that members will easily recognize."}
                            </p>
                        </div>

                        // Error message
                        if let Some(error) = &form.error {
                            <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded-md">
                                {error}
                            </div>
                        }

                        // Action buttons
                        <div class="flex flex-col sm:flex-row justify-end gap-3 sm:space-x-3 sm:gap-0 pt-4 border-t border-gray-200 dark:border-gray-700">
                            <button
                                type="button"
                                onclick={on_cancel}
                                disabled={form.is_loading}
                                class={format!("w-full sm:w-auto px-4 py-2 text-sm font-medium rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 border {}",
                                    if form.is_loading {
                                        "border-gray-300 bg-gray-300 text-gray-500 cursor-not-allowed opacity-50"
                                    } else {
                                        "border-gray-300 bg-white text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600"
                                    }
                                )}
                            >
                                {"Cancel"}
                            </button>
                            <button
                                type="submit"
                                disabled={form.is_loading || form.name.trim().is_empty()}
                                class={format!("w-full sm:w-auto px-4 py-2 text-sm font-medium rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 border {}",
                                    if form.is_loading || form.name.trim().is_empty() {
                                        "border-gray-300 bg-gray-300 text-gray-500 cursor-not-allowed opacity-50"
                                    } else {
                                        "border-transparent bg-blue-600 text-white hover:bg-blue-700"
                                    }
                                )}
                            >
                                if form.is_loading {
                                    <span class="flex items-center justify-center">
                                        <svg class="animate-spin -ml-1 mr-2 h-4 w-4 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                        </svg>
                                        {"Creating..."}
                                    </span>
                                } else {
                                    {"Create Community"}
                                }
                            </button>
                        </div>
                    </form>
                </div>

                // Additional info card
                <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-6">
                    <div class="flex">
                        <div class="flex-shrink-0">
                            <svg class="h-5 w-5 text-blue-400" fill="currentColor" viewBox="0 0 20 20">
                                <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd"></path>
                            </svg>
                        </div>
                        <div class="ml-3">
                            <h3 class="text-sm font-medium text-blue-800 dark:text-blue-200">
                                {"What happens next?"}
                            </h3>
                            <div class="mt-2 text-sm text-blue-700 dark:text-blue-300">
                                <ul class="list-disc list-inside space-y-1">
                                    <li>{"You'll be assigned as the community leader"}</li>
                                    <li>{"You can invite members and create sites"}</li>
                                    <li>{"Start setting up spaces and auction schedules"}</li>
                                    <li>{"Configure community settings and permissions"}</li>
                                </ul>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </main>
    }
}

// Community Invites State
#[derive(Default, Clone, PartialEq)]
struct CommunityInvitesState {
    invites: Vec<responses::CommunityInvite>,
    is_loading: bool,
    error: Option<String>,
    accepting_invite: Option<payloads::InviteId>,
}

#[function_component]
pub fn CommunityInvites() -> Html {
    let (auth_state, _) = use_auth();
    let navigator = use_navigator().unwrap();
    let invites_state = use_state(CommunityInvitesState::default);

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

    // Load invites when component mounts
    {
        let invites_state = invites_state.clone();
        let auth_state = auth_state.clone();

        use_effect_with(auth_state.is_authenticated, move |is_authenticated| {
            if *is_authenticated {
                let invites_state = invites_state.clone();

                yew::platform::spawn_local(async move {
                    let mut state = (*invites_state).clone();
                    state.is_loading = true;
                    state.error = None;
                    invites_state.set(state);

                    let client = get_api_client();
                    match client.get_invites().await {
                        Ok(invites) => {
                            let mut state = (*invites_state).clone();
                            state.invites = invites;
                            state.is_loading = false;
                            invites_state.set(state);
                        }
                        Err(e) => {
                            let mut state = (*invites_state).clone();
                            state.error =
                                Some(format!("Failed to load invites: {}", e));
                            state.is_loading = false;
                            invites_state.set(state);
                        }
                    }
                });
            }
            || ()
        });
    }

    // Check for accept query parameter and auto-accept invite
    {
        let navigator = navigator.clone();
        let auth_state = auth_state.clone();
        let invites_state = invites_state.clone();

        use_effect_with(auth_state.is_authenticated, move |is_authenticated| {
            if *is_authenticated {
                let navigator = navigator.clone();
                let invites_state = invites_state.clone();

                yew::platform::spawn_local(async move {
                    let window = web_sys::window().unwrap();
                    let location = window.location();

                    // Parse query parameters
                    if let Ok(search) = location.search() {
                        if !search.is_empty() {
                            // Parse query string (starts with '?')
                            let query_string = &search[1..]; // Remove the '?' prefix
                            for param in query_string.split('&') {
                                if let Some((key, value)) =
                                    param.split_once('=')
                                {
                                    if key == "accept" {
                                        // Try to parse the invite ID
                                        if let Ok(uuid) =
                                            value.parse::<uuid::Uuid>()
                                        {
                                            let invite_id =
                                                payloads::InviteId(uuid);

                                            // Set accepting state
                                            {
                                                let mut state =
                                                    (*invites_state).clone();
                                                state.accepting_invite =
                                                    Some(invite_id);
                                                invites_state.set(state);
                                            }

                                            let client = get_api_client();
                                            match client
                                                .accept_invite(&invite_id)
                                                .await
                                            {
                                                Ok(()) => {
                                                    // Successfully accepted, navigate to communities
                                                    navigator.push(
                                                        &Route::Communities,
                                                    );
                                                }
                                                Err(e) => {
                                                    let mut state =
                                                        (*invites_state)
                                                            .clone();
                                                    state.accepting_invite =
                                                        None;
                                                    state.error = Some(
                                                        format!(
                                                            "Failed to accept invite: {}",
                                                            e
                                                        ),
                                                    );
                                                    invites_state.set(state);
                                                }
                                            }
                                        } else {
                                            // Invalid invite ID format
                                            let mut state =
                                                (*invites_state).clone();
                                            state.error = Some(
                                                "Invalid invite link format"
                                                    .to_string(),
                                            );
                                            invites_state.set(state);
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                    }
                });
            }
            || ()
        });
    }

    let on_accept_invite = {
        let invites_state = invites_state.clone();
        let navigator = navigator.clone();

        Callback::from(move |invite_id: payloads::InviteId| {
            let invites_state = invites_state.clone();
            let navigator = navigator.clone();

            yew::platform::spawn_local(async move {
                // Set accepting state
                {
                    let mut state = (*invites_state).clone();
                    state.accepting_invite = Some(invite_id);
                    invites_state.set(state);
                }

                let client = get_api_client();
                match client.accept_invite(&invite_id).await {
                    Ok(()) => {
                        // Successfully accepted, navigate to communities
                        navigator.push(&Route::Communities);
                    }
                    Err(e) => {
                        let mut state = (*invites_state).clone();
                        state.accepting_invite = None;
                        state.error =
                            Some(format!("Failed to accept invite: {}", e));
                        invites_state.set(state);
                    }
                }
            });
        })
    };

    let on_back_to_communities = {
        let navigator = navigator.clone();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::Communities);
        })
    };

    // Don't render anything if not authenticated
    if !auth_state.is_authenticated {
        return html! {};
    }

    html! {
        <main class="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
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
                                    <span class="ml-4 text-sm font-medium text-gray-500 dark:text-gray-400">{"Community Invites"}</span>
                                </div>
                            </li>
                        </ol>
                    </nav>
                    <div class="mt-4 flex justify-between items-center">
                        <div>
                            <h1 class="text-3xl font-bold text-gray-900 dark:text-white">{"Community Invites"}</h1>
                            <p class="mt-2 text-gray-600 dark:text-gray-300">
                                {"Join communities by accepting invitations you've received."}
                            </p>
                        </div>
                        <button
                            onclick={on_back_to_communities.clone()}
                            class="inline-flex items-center px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 hover:bg-gray-50 dark:hover:bg-gray-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                        >
                            <svg class="-ml-1 mr-2 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18" />
                            </svg>
                            {"Back to Communities"}
                        </button>
                    </div>
                </div>

                // Error message
                if let Some(error) = &invites_state.error {
                    <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded">
                        {error}
                    </div>
                }

                // Loading state
                if invites_state.is_loading {
                    <div class="text-center py-12">
                        <svg class="animate-spin h-8 w-8 text-blue-600 mx-auto" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                        </svg>
                        <p class="mt-2 text-gray-600 dark:text-gray-400">{"Loading invites..."}</p>
                    </div>
                } else if invites_state.invites.is_empty() {
                    // Empty state
                    <div class="text-center py-12">
                        <div class="mx-auto h-12 w-12 text-gray-400">
                            <svg fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"></path>
                            </svg>
                        </div>
                        <h3 class="mt-2 text-lg font-medium text-gray-900 dark:text-white">{"No pending invites"}</h3>
                        <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
                            {"You don't have any pending community invitations at the moment."}
                        </p>
                        <div class="mt-6">
                            <button
                                onclick={on_back_to_communities.clone()}
                                class="inline-flex items-center px-4 py-2 border border-transparent shadow-sm text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                            >
                                {"View My Communities"}
                            </button>
                        </div>
                    </div>
                } else {
                    // Invites list
                    <div class="bg-white dark:bg-gray-800 shadow overflow-hidden sm:rounded-md">
                        <ul role="list" class="divide-y divide-gray-200 dark:divide-gray-700">
                            {for invites_state.invites.iter().map(|invite| {
                                let invite_id = invite.id;
                                let is_accepting = invites_state.accepting_invite == Some(invite_id);

                                html! {
                                    <InviteItem
                                        key={format!("{}", invite.id.0)}
                                        invite={invite.clone()}
                                        is_accepting={is_accepting}
                                        on_accept={on_accept_invite.clone()}
                                    />
                                }
                            })}
                        </ul>
                    </div>
                }

                // Help card
                <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-6">
                    <div class="flex">
                        <div class="flex-shrink-0">
                            <svg class="h-5 w-5 text-blue-400" fill="currentColor" viewBox="0 0 20 20">
                                <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd"></path>
                            </svg>
                        </div>
                        <div class="ml-3">
                            <h3 class="text-sm font-medium text-blue-800 dark:text-blue-200">
                                {"About Community Invites"}
                            </h3>
                            <div class="mt-2 text-sm text-blue-700 dark:text-blue-300">
                                <ul class="list-disc list-inside space-y-1">
                                    <li>{"Invites are sent to your verified email address"}</li>
                                    <li>{"You can only see invites for your current email"}</li>
                                    <li>{"Accepting an invite makes you a member of that community"}</li>
                                    <li>{"You'll start with the 'Member' role in new communities"}</li>
                                </ul>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </main>
    }
}

#[derive(Properties, PartialEq)]
pub struct InviteItemProps {
    pub invite: responses::CommunityInvite,
    pub is_accepting: bool,
    pub on_accept: Callback<payloads::InviteId>,
}

#[function_component]
pub fn InviteItem(props: &InviteItemProps) -> Html {
    let invite = &props.invite;
    let is_accepting = props.is_accepting;

    let on_accept = {
        let invite_id = invite.id;
        let on_accept = props.on_accept.clone();

        Callback::from(move |_: MouseEvent| {
            on_accept.emit(invite_id);
        })
    };

    // Format the creation date
    let created_date = {
        // For now, just show a placeholder
        // TODO: Format the jiff::Timestamp properly
        "Recently".to_string()
    };

    let first_letter = invite
        .community_name
        .chars()
        .next()
        .unwrap_or('C')
        .to_uppercase()
        .to_string();

    html! {
        <li class="px-6 py-4">
            <div class="flex items-center justify-between">
                <div class="flex items-center">
                    <div class="flex-shrink-0 h-10 w-10">
                        <div class="h-10 w-10 bg-green-500 rounded-lg flex items-center justify-center">
                            <span class="text-white font-medium text-sm">
                                {first_letter}
                            </span>
                        </div>
                    </div>
                    <div class="ml-4">
                        <div class="text-sm font-medium text-gray-900 dark:text-white">
                            {&invite.community_name}
                        </div>
                        <div class="text-sm text-gray-500 dark:text-gray-400">
                            {"Invited "}{created_date}
                        </div>
                    </div>
                </div>
                <div class="flex items-center space-x-3">
                    <div class="flex items-center">
                        <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 dark:bg-green-900/30 text-green-800 dark:text-green-300">
                            <svg class="-ml-0.5 mr-1.5 h-2 w-2 text-green-400" fill="currentColor" viewBox="0 0 8 8">
                                <circle cx="4" cy="4" r="3" />
                            </svg>
                            {"Pending"}
                        </span>
                    </div>
                    <button
                        onclick={on_accept}
                        disabled={is_accepting}
                        class="inline-flex items-center px-3 py-2 border border-transparent text-sm leading-4 font-medium rounded-md text-white bg-green-600 hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                        if is_accepting {
                            <span class="flex items-center">
                                <svg class="animate-spin -ml-1 mr-2 h-4 w-4 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                </svg>
                                {"Accepting..."}
                            </span>
                        } else {
                            <>
                                <svg class="-ml-1 mr-2 h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                                </svg>
                                {"Accept Invite"}
                            </>
                        }
                    </button>
                </div>
            </div>
        </li>
    }
}

// Community Management State
#[derive(Default, Clone, PartialEq)]
struct CommunitySettingsState {
    community: Option<responses::Community>,
    members: Vec<responses::CommunityMember>,
    pending_invites: Vec<responses::CommunityInvite>,
    is_loading: bool,
    error: Option<String>,
}

#[derive(Default, Clone, PartialEq)]
struct InviteForm {
    invitation_type: InvitationType,
    email: String,
    is_loading: bool,
    error: Option<String>,
    success_message: Option<String>,
    generated_invite_link: Option<String>,
}

#[derive(Clone, PartialEq)]
enum InvitationType {
    Email,
    Link,
}

impl Default for InvitationType {
    fn default() -> Self {
        Self::Email
    }
}

#[derive(Properties, PartialEq)]
pub struct CommunitySettingsProps {
    pub community_id: String,
}

#[function_component]
pub fn CommunitySettings(props: &CommunitySettingsProps) -> Html {
    let (auth_state, _) = use_auth();
    let navigator = use_navigator().unwrap();
    let community_state = use_state(CommunitySettingsState::default);
    let invite_form = use_state(InviteForm::default);

    // Parse community ID
    let community_id = match props.community_id.parse::<uuid::Uuid>() {
        Ok(id) => payloads::CommunityId(id),
        Err(_) => {
            // Invalid UUID, navigate back to communities
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

    // Load community data when component mounts
    {
        let community_state = community_state.clone();
        let auth_state = auth_state.clone();
        let community_id = community_id.clone();

        use_effect_with(
            (auth_state.is_authenticated, props.community_id.clone()),
            move |(is_authenticated, _)| {
                web_sys::console::log_1(
                    &format!(
                        "Effect triggered: authenticated={}",
                        is_authenticated
                    )
                    .into(),
                );
                if *is_authenticated {
                    let community_state = community_state.clone();
                    let community_id = community_id.clone();

                    yew::platform::spawn_local(async move {
                        web_sys::console::log_1(
                            &"Starting community data load".into(),
                        );
                        let mut state = (*community_state).clone();
                        state.is_loading = true;
                        state.error = None;
                        community_state.set(state);

                        let client = get_api_client();

                        // First get all communities to find this one
                        web_sys::console::log_1(
                            &"Fetching communities...".into(),
                        );
                        match client.get_communities().await {
                            Ok(communities) => {
                                web_sys::console::log_1(
                                    &format!(
                                        "Got {} communities",
                                        communities.len()
                                    )
                                    .into(),
                                );
                                // Find the community we're managing
                                let community = communities
                                    .into_iter()
                                    .find(|c| c.id == community_id);

                                if let Some(community) = community {
                                    web_sys::console::log_1(
                                        &format!(
                                            "Found community: {}",
                                            community.name
                                        )
                                        .into(),
                                    );
                                    // Set the community data first, so we can show the page even if members fail
                                    {
                                        let mut state =
                                            (*community_state).clone();
                                        state.community =
                                            Some(community.clone());
                                        community_state.set(state);
                                    }

                                    // Now try to get the members
                                    web_sys::console::log_1(
                                        &"Fetching members...".into(),
                                    );
                                    match client
                                        .get_members(&community_id)
                                        .await
                                    {
                                        Ok(members) => {
                                            web_sys::console::log_1(
                                                &format!(
                                                    "Got {} members",
                                                    members.len()
                                                )
                                                .into(),
                                            );
                                            let mut state =
                                                (*community_state).clone();
                                            state.community = Some(community);
                                            state.members = members;
                                            state.is_loading = false;
                                            community_state.set(state);
                                        }
                                        Err(e) => {
                                            web_sys::console::log_1(
                                                &format!(
                                                    "Members error: {}",
                                                    e
                                                )
                                                .into(),
                                            );
                                            let mut state =
                                                (*community_state).clone();
                                            state.community = Some(community);
                                            state.members = Vec::new(); // Empty members list
                                            state.is_loading = false;
                                            state.error = Some(format!(
                                                "Failed to load members: {}. You may not have permission to view members.",
                                                e
                                            ));
                                            community_state.set(state);
                                        }
                                    }
                                } else {
                                    web_sys::console::log_1(&"Community not found in user's communities".into());
                                    let mut state = (*community_state).clone();
                                    state.error = Some("Community not found or you don't have access".to_string());
                                    state.is_loading = false;
                                    community_state.set(state);
                                }
                            }
                            Err(e) => {
                                web_sys::console::log_1(
                                    &format!("Communities error: {}", e).into(),
                                );
                                let mut state = (*community_state).clone();
                                state.error = Some(format!(
                                    "Failed to load communities: {} (API Error)",
                                    e
                                ));
                                state.is_loading = false;
                                community_state.set(state);
                            }
                        }
                    });
                } else {
                    web_sys::console::log_1(&"User not authenticated".into());
                }
                || ()
            },
        );
    }

    let on_email_change = {
        let invite_form = invite_form.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut form_data = (*invite_form).clone();
            form_data.email = input.value();
            invite_form.set(form_data);
        })
    };

    let on_invitation_type_change = {
        let invite_form = invite_form.clone();
        Callback::from(move |invitation_type: InvitationType| {
            let mut form_data = (*invite_form).clone();
            form_data.invitation_type = invitation_type;
            // Clear any previous state when switching types
            form_data.error = None;
            form_data.success_message = None;
            form_data.generated_invite_link = None;
            if matches!(form_data.invitation_type, InvitationType::Link) {
                form_data.email.clear();
            }
            invite_form.set(form_data);
        })
    };

    let on_invite_submit = {
        let invite_form = invite_form.clone();
        let community_id = community_id.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let form_data = (*invite_form).clone();

            // Validation based on invitation type
            match form_data.invitation_type {
                InvitationType::Email => {
                    if form_data.email.trim().is_empty() {
                        let mut new_form = form_data;
                        new_form.error =
                            Some("Email address is required".to_string());
                        invite_form.set(new_form);
                        return;
                    }

                    // Basic email validation
                    if !form_data.email.contains('@') {
                        let mut new_form = form_data;
                        new_form.error = Some(
                            "Please enter a valid email address".to_string(),
                        );
                        invite_form.set(new_form);
                        return;
                    }
                }
                InvitationType::Link => {
                    // No validation needed for link type
                }
            }

            let invite_form = invite_form.clone();
            let community_id = community_id.clone();
            let email =
                if matches!(form_data.invitation_type, InvitationType::Email) {
                    Some(form_data.email.trim().to_string())
                } else {
                    None
                };

            yew::platform::spawn_local(async move {
                // Set loading state
                {
                    let mut new_form = (*invite_form).clone();
                    new_form.is_loading = true;
                    new_form.error = None;
                    new_form.success_message = None;
                    new_form.generated_invite_link = None;
                    invite_form.set(new_form);
                }

                let client = get_api_client();
                let invite_request = requests::InviteCommunityMember {
                    community_id,
                    new_member_email: email.clone(),
                };

                match client.invite_member(&invite_request).await {
                    Ok(invite_id) => {
                        let mut new_form = (*invite_form).clone();
                        new_form.is_loading = false;

                        match email {
                            Some(email_addr) => {
                                // Email invitation - show success message
                                new_form.success_message = Some(format!(
                                    "Invitation sent to {}",
                                    email_addr
                                ));
                                // Reset form for next invitation
                                new_form.email.clear();
                            }
                            None => {
                                // Link invitation - generate the link
                                let window = web_sys::window().unwrap();
                                let location = window.location();
                                let origin = location.origin().unwrap();
                                let invite_link = format!(
                                    "{}/communities/invites?accept={}",
                                    origin, invite_id.0
                                );

                                new_form.generated_invite_link =
                                    Some(invite_link.clone());
                                new_form.success_message = Some("Invite link generated! Share this link with the person you want to invite.".to_string());
                            }
                        }

                        invite_form.set(new_form);
                    }
                    Err(e) => {
                        let mut new_form = (*invite_form).clone();
                        new_form.is_loading = false;
                        new_form.error =
                            Some(format!("Failed to create invite: {}", e));
                        invite_form.set(new_form);
                    }
                }
            });
        })
    };

    let on_back_to_communities = {
        let navigator = navigator.clone();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::Communities);
        })
    };

    // Don't render anything if not authenticated
    if !auth_state.is_authenticated {
        return html! {};
    }

    // Loading state
    if community_state.is_loading {
        return html! {
            <main class="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
                <div class="text-center py-12">
                    <svg class="animate-spin h-8 w-8 text-blue-600 mx-auto" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                    </svg>
                    <p class="mt-2 text-gray-600 dark:text-gray-400">{"Loading community..."}</p>
                </div>
            </main>
        };
    }

    // Error state
    if let Some(error) = &community_state.error {
        return html! {
            <main class="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
                <div class="text-center py-12">
                    <div class="mx-auto h-12 w-12 text-red-400">
                        <svg fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
                            <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clip-rule="evenodd"></path>
                        </svg>
                    </div>
                    <h3 class="mt-2 text-lg font-medium text-gray-900 dark:text-white">{"Error loading community"}</h3>
                    <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">{error}</p>
                    <div class="mt-6">
                        <button
                            onclick={on_back_to_communities}
                            class="inline-flex items-center px-4 py-2 border border-transparent shadow-sm text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                        >
                            {"Back to Communities"}
                        </button>
                    </div>
                </div>
            </main>
        };
    }

    // Get community data
    let community = match &community_state.community {
        Some(community) => community,
        None => return html! {}, // This shouldn't happen but just in case
    };

    let first_letter = community
        .name
        .chars()
        .next()
        .unwrap_or('C')
        .to_uppercase()
        .to_string();

    html! {
        <main class="max-w-6xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
            <div class="space-y-8">
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
                                        to={Route::CommunityDashboard { id: props.community_id.clone() }}
                                        classes="ml-4 text-sm font-medium text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300"
                                    >
                                        {&community.name}
                                    </Link<Route>>
                                </div>
                            </li>
                            <li>
                                <div class="flex items-center">
                                    <svg class="flex-shrink-0 h-5 w-5 text-gray-300" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                        <path d="M5.555 17.776l8-16 .894.448-8 16-.894-.448z" />
                                    </svg>
                                    <span class="ml-4 text-sm font-medium text-gray-500 dark:text-gray-400">{"Settings"}</span>
                                </div>
                            </li>
                        </ol>
                    </nav>

                    <div class="mt-4 space-y-4 sm:space-y-0 sm:flex sm:items-center sm:justify-between">
                        <div class="flex items-center space-x-3 sm:space-x-4 min-w-0 flex-1">
                            <div class="flex-shrink-0">
                                <div class="w-12 h-12 sm:w-16 sm:h-16 bg-blue-500 rounded-xl flex items-center justify-center">
                                    <span class="text-white font-bold text-lg sm:text-2xl">
                                        {first_letter}
                                    </span>
                                </div>
                            </div>
                            <div class="min-w-0 flex-1">
                                <h1 class="text-xl sm:text-2xl lg:text-3xl font-bold text-gray-900 dark:text-white truncate">{&community.name}</h1>
                                <p class="mt-1 sm:mt-2 text-sm sm:text-base text-gray-600 dark:text-gray-300">
                                    {"Community settings and member management"}
                                </p>
                            </div>
                        </div>
                        <div class="flex-shrink-0">
                            <button
                                onclick={on_back_to_communities}
                                class="w-full sm:w-auto inline-flex items-center justify-center px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 hover:bg-gray-50 dark:hover:bg-gray-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                            >
                                <svg class="-ml-1 mr-2 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18" />
                                </svg>
                                <span class="sm:hidden">{"Back"}</span>
                                <span class="hidden sm:inline">{"Back to Communities"}</span>
                            </button>
                        </div>
                    </div>
                </div>

                <div class="grid grid-cols-1 lg:grid-cols-3 gap-6 lg:gap-8">
                    // Main content area
                    <div class="lg:col-span-2 space-y-6 order-2 lg:order-1">
                        // Error message for members loading
                        if let Some(error) = &community_state.error {
                            <div class="bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg p-4">
                                <div class="flex">
                                    <div class="flex-shrink-0">
                                        <svg class="h-5 w-5 text-yellow-400" viewBox="0 0 20 20" fill="currentColor">
                                            <path fill-rule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clip-rule="evenodd"></path>
                                        </svg>
                                    </div>
                                    <div class="ml-3">
                                        <h3 class="text-sm font-medium text-yellow-800 dark:text-yellow-200">
                                            {"Warning"}
                                        </h3>
                                        <div class="mt-2 text-sm text-yellow-700 dark:text-yellow-300">
                                            {error}
                                        </div>
                                    </div>
                                </div>
                            </div>
                        }

                        // Invite new members section
                        <div class="bg-white dark:bg-gray-800 shadow rounded-lg">
                            <div class="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                                <h2 class="text-lg font-medium text-gray-900 dark:text-white">{"Invite New Members"}</h2>
                                <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
                                    {"Send email invitations or create shareable invite links."}
                                </p>
                            </div>
                            <div class="px-6 py-4">
                                <form onsubmit={on_invite_submit} class="space-y-4">
                                    // Invitation type selector
                                    <div>
                                        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">
                                            {"Invitation Type"}
                                        </label>
                                        <div class="flex space-x-4">
                                            <label class="flex items-center">
                                                <input
                                                    type="radio"
                                                    name="invitation-type"
                                                    class="h-4 w-4 text-blue-600 focus:ring-blue-500 border-gray-300 dark:border-gray-600"
                                                    checked={matches!(invite_form.invitation_type, InvitationType::Email)}
                                                    onchange={on_invitation_type_change.reform(|_| InvitationType::Email)}
                                                />
                                                <span class="ml-2 text-sm text-gray-700 dark:text-gray-300">{"Email Invitation"}</span>
                                            </label>
                                            <label class="flex items-center">
                                                <input
                                                    type="radio"
                                                    name="invitation-type"
                                                    class="h-4 w-4 text-blue-600 focus:ring-blue-500 border-gray-300 dark:border-gray-600"
                                                    checked={matches!(invite_form.invitation_type, InvitationType::Link)}
                                                    onchange={on_invitation_type_change.reform(|_| InvitationType::Link)}
                                                />
                                                <span class="ml-2 text-sm text-gray-700 dark:text-gray-300">{"Invite Link"}</span>
                                            </label>
                                        </div>
                                        <p class="mt-2 text-sm text-gray-500 dark:text-gray-400">
                                            {
                                                match invite_form.invitation_type {
                                                    InvitationType::Email => "Send an invitation directly to someone's email address.",
                                                    InvitationType::Link => "Create a one-time link that can be shared with anyone."
                                                }
                                            }
                                        </p>
                                    </div>

                                    // Email input (only shown for email invitations)
                                    if matches!(invite_form.invitation_type, InvitationType::Email) {
                                        <div>
                                            <label for="invite-email" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
                                                {"Email Address"}
                                            </label>
                                            <div class="mt-1">
                                                <input
                                                    type="email"
                                                    id="invite-email"
                                                    name="invite-email"
                                                    required=true
                                                    class="block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm placeholder-gray-400 focus:outline-none focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-700 dark:text-white sm:text-sm"
                                                placeholder="Enter email address to invite"
                                                value={invite_form.email.clone()}
                                                oninput={on_email_change}
                                                disabled={invite_form.is_loading}
                                            />
                                        </div>
                                        </div>
                                    }

                                    // Generated invite link (shown after creating a link)
                                    if let Some(link) = &invite_form.generated_invite_link {
                                        <div class="bg-gray-50 dark:bg-gray-700 border border-gray-200 dark:border-gray-600 rounded-md p-4">
                                            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                                {"Invite Link"}
                                            </label>
                                            <input
                                                type="text"
                                                readonly=true
                                                class="w-full px-3 py-2 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-md text-sm font-mono text-gray-900 dark:text-white"
                                                value={link.clone()}
                                            />
                                            <p class="mt-2 text-sm text-gray-500 dark:text-gray-400">
                                                {"Share this link with the person you want to invite. The link can only be used once."}
                                            </p>
                                        </div>
                                    }

                                    if let Some(error) = &invite_form.error {
                                        <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded-md">
                                            {error}
                                        </div>
                                    }

                                    if let Some(success) = &invite_form.success_message {
                                        <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 text-green-600 dark:text-green-400 px-4 py-3 rounded-md">
                                            {success}
                                        </div>
                                    }

                                    <div class="flex justify-end">
                                        <button
                                            type="submit"
                                            disabled={invite_form.is_loading || (matches!(invite_form.invitation_type, InvitationType::Email) && invite_form.email.trim().is_empty())}
                                            class={format!("inline-flex items-center px-4 py-2 text-sm font-medium rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 border {}",
                                                if invite_form.is_loading || (matches!(invite_form.invitation_type, InvitationType::Email) && invite_form.email.trim().is_empty()) {
                                                    "border-gray-300 bg-gray-300 text-gray-500 cursor-not-allowed opacity-50"
                                                } else {
                                                    "border-transparent bg-blue-600 text-white hover:bg-blue-700"
                                                }
                                            )}
                                        >
                                            if invite_form.is_loading {
                                                <span class="flex items-center">
                                                    <svg class="animate-spin -ml-1 mr-2 h-4 w-4 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 714 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                                    </svg>
                                                    {
                                                        match invite_form.invitation_type {
                                                            InvitationType::Email => "Sending Invite...",
                                                            InvitationType::Link => "Creating Link..."
                                                        }
                                                    }
                                                </span>
                                            } else {
                                                <>
                                                    <svg class="-ml-1 mr-2 h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
                                                    </svg>
                                                    {
                                                        match invite_form.invitation_type {
                                                            InvitationType::Email => "Send Invitation",
                                                            InvitationType::Link => "Create Invite Link"
                                                        }
                                                    }
                                                </>
                                            }
                                        </button>
                                    </div>
                                </form>
                            </div>
                        </div>

                        // Members list section
                        <div class="bg-white dark:bg-gray-800 shadow rounded-lg">
                            <div class="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                                <h2 class="text-lg font-medium text-gray-900 dark:text-white">
                                    {"Members "}
                                    <span class="text-sm font-normal text-gray-500 dark:text-gray-400">
                                        {"("}{community_state.members.len()}{")"}
                                    </span>
                                </h2>
                            </div>
                            <div class="divide-y divide-gray-200 dark:divide-gray-700">
                                {for community_state.members.iter().map(|member| {
                                    html! {
                                        <MemberItem
                                            key={member.username.clone()}
                                            member={member.clone()}
                                        />
                                    }
                                })}

                                if community_state.members.is_empty() {
                                    <div class="px-6 py-8 text-center">
                                        <div class="mx-auto h-12 w-12 text-gray-400">
                                            <svg fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z"></path>
                                            </svg>
                                        </div>
                                        <h3 class="mt-2 text-sm font-medium text-gray-900 dark:text-white">{"No members yet"}</h3>
                                        <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
                                            {"Start by inviting your first member."}
                                        </p>
                                    </div>
                                }
                            </div>
                        </div>
                    </div>

                    // Sidebar
                    <div class="space-y-6 order-1 lg:order-2">
                        // Community info card
                        <div class="bg-white dark:bg-gray-800 shadow rounded-lg p-6">
                            <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-4">{"Community Info"}</h3>
                            <dl class="space-y-3">
                                // Remove new members status display for MVP
                                <div>
                                    <dt class="text-sm font-medium text-gray-500 dark:text-gray-400">{"Sites"}</dt>
                                    <dd class="text-sm text-gray-900 dark:text-white">{"0 sites"}</dd>
                                </div>
                                <div>
                                    <dt class="text-sm font-medium text-gray-500 dark:text-gray-400">{"Members"}</dt>
                                    <dd class="text-sm text-gray-900 dark:text-white">{"View in settings"}</dd>
                                </div>
                                <div>
                                    <dt class="text-sm font-medium text-gray-500 dark:text-gray-400">{"Created"}</dt>
                                    <dd class="text-sm text-gray-900 dark:text-white">{"Recently"}</dd>
                                </div>
                            </dl>
                        </div>

                        // Help card
                        <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-6">
                            <div class="flex">
                                <div class="flex-shrink-0">
                                    <svg class="h-5 w-5 text-blue-400" fill="currentColor" viewBox="0 0 20 20">
                                        <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd"></path>
                                    </svg>
                                </div>
                                <div class="ml-3">
                                    <h3 class="text-sm font-medium text-blue-800 dark:text-blue-200">
                                        {"Managing Your Community"}
                                    </h3>
                                    <div class="mt-2 text-sm text-blue-700 dark:text-blue-300">
                                        <ul class="list-disc list-inside space-y-1">
                                            <li>{"Invite members by email"}</li>
                                            <li>{"View member roles and permissions"}</li>
                                            <li>{"Manage community settings"}</li>
                                            <li>{"Create sites and auctions"}</li>
                                        </ul>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </main>
    }
}

#[derive(Properties, PartialEq)]
pub struct MemberItemProps {
    pub member: responses::CommunityMember,
}

#[function_component]
pub fn MemberItem(props: &MemberItemProps) -> Html {
    let member = &props.member;

    // Get role color and text
    let (role_bg, role_text, role_display) = match member.role {
        payloads::Role::Leader => (
            "bg-purple-100 dark:bg-purple-900/30",
            "text-purple-800 dark:text-purple-300",
            "Leader",
        ),
        payloads::Role::Coleader => (
            "bg-purple-100 dark:bg-purple-900/30",
            "text-purple-800 dark:text-purple-300",
            "Co-leader",
        ),
        payloads::Role::Moderator => (
            "bg-blue-100 dark:bg-blue-900/30",
            "text-blue-800 dark:text-blue-300",
            "Moderator",
        ),
        payloads::Role::Member => (
            "bg-gray-100 dark:bg-gray-700",
            "text-gray-800 dark:text-gray-300",
            "Member",
        ),
    };

    let first_letter = member
        .username
        .chars()
        .next()
        .unwrap_or('U')
        .to_uppercase()
        .to_string();

    html! {
        <div class="px-6 py-4">
            <div class="flex items-center justify-between">
                <div class="flex items-center">
                    <div class="flex-shrink-0 h-10 w-10">
                        <div class="h-10 w-10 bg-gray-500 rounded-full flex items-center justify-center">
                            <span class="text-white font-medium text-sm">
                                {first_letter}
                            </span>
                        </div>
                    </div>
                    <div class="ml-4">
                        <div class="text-sm font-medium text-gray-900 dark:text-white">
                            {&member.username}
                        </div>
                        // <div class="text-sm text-gray-500 dark:text-gray-400">
                            // {"Member"} // Remove active/inactive status for MVP
                        // </div>
                    </div>
                </div>
                <div class="flex items-center space-x-2">
                    // Remove inactive status badge for MVP
                    <span class={format!("inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium {} {}", role_bg, role_text)}>
                        {role_display}
                    </span>
                </div>
            </div>
        </div>
    }
}

// Community Dashboard State
#[derive(Default, Clone, PartialEq)]
pub struct CommunityDashboardState {
    pub community: Option<responses::Community>,
    pub is_loading: bool,
    pub error: Option<String>,
}

#[derive(Properties, PartialEq)]
pub struct CommunityDashboardProps {
    pub community_id: String,
}

#[function_component]
pub fn CommunityDashboard(props: &CommunityDashboardProps) -> Html {
    let (auth_state, _) = use_auth();
    let navigator = use_navigator().unwrap();
    let community_state = use_state(CommunityDashboardState::default);

    // Parse community ID
    let community_id = match props.community_id.parse::<uuid::Uuid>() {
        Ok(id) => payloads::CommunityId(id),
        Err(_) => {
            // Invalid UUID, navigate back to communities
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

    // Load community data when component mounts
    {
        let community_state = community_state.clone();
        let auth_state = auth_state.clone();
        let community_id = community_id.clone();

        use_effect_with(
            (auth_state.is_authenticated, props.community_id.clone()),
            move |(is_authenticated, _)| {
                if *is_authenticated {
                    let community_state = community_state.clone();
                    let community_id = community_id.clone();

                    yew::platform::spawn_local(async move {
                        let mut state = (*community_state).clone();
                        state.is_loading = true;
                        state.error = None;
                        community_state.set(state);

                        let client = get_api_client();

                        // Get all communities to find this one
                        match client.get_communities().await {
                            Ok(communities) => {
                                // Find the community we're viewing
                                let community = communities
                                    .into_iter()
                                    .find(|c| c.id == community_id);

                                if let Some(community) = community {
                                    let mut state = (*community_state).clone();
                                    state.community = Some(community);
                                    state.is_loading = false;
                                    community_state.set(state);
                                } else {
                                    let mut state = (*community_state).clone();
                                    state.error = Some("Community not found or you don't have access".to_string());
                                    state.is_loading = false;
                                    community_state.set(state);
                                }
                            }
                            Err(e) => {
                                let mut state = (*community_state).clone();
                                state.error = Some(format!(
                                    "Failed to load community: {}",
                                    e
                                ));
                                state.is_loading = false;
                                community_state.set(state);
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    let on_back_to_communities = {
        let navigator = navigator.clone();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::Communities);
        })
    };

    let on_settings = {
        let navigator = navigator.clone();
        let community_id = props.community_id.clone();
        Callback::from(move |_: MouseEvent| {
            navigator.push(&Route::CommunitySettings {
                id: community_id.clone(),
            });
        })
    };

    // Don't render anything if not authenticated
    if !auth_state.is_authenticated {
        return html! {};
    }

    // Loading state
    if community_state.is_loading {
        return html! {
            <main class="max-w-6xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
                <div class="text-center py-12">
                    <svg class="animate-spin h-8 w-8 text-blue-600 mx-auto" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                    </svg>
                    <p class="mt-2 text-gray-600 dark:text-gray-400">{"Loading community..."}</p>
                </div>
            </main>
        };
    }

    // Error state
    if let Some(error) = &community_state.error {
        return html! {
            <main class="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
                <div class="text-center py-12">
                    <div class="mx-auto h-12 w-12 text-red-400">
                        <svg fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
                            <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clip-rule="evenodd"></path>
                        </svg>
                    </div>
                    <h3 class="mt-2 text-lg font-medium text-gray-900 dark:text-white">{"Error loading community"}</h3>
                    <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">{error}</p>
                    <div class="mt-6">
                        <button
                            onclick={on_back_to_communities}
                            class="inline-flex items-center px-4 py-2 border border-transparent shadow-sm text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                        >
                            {"Back to Communities"}
                        </button>
                    </div>
                </div>
            </main>
        };
    }

    // Get community data
    let community = match &community_state.community {
        Some(community) => community,
        None => return html! {}, // This shouldn't happen but just in case
    };

    let first_letter = community
        .name
        .chars()
        .next()
        .unwrap_or('C')
        .to_uppercase()
        .to_string();

    html! {
        <main class="max-w-6xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
            <div class="space-y-8">
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
                                    <span class="ml-4 text-sm font-medium text-gray-500 dark:text-gray-400">{&community.name}</span>
                                </div>
                            </li>
                            <li>
                                <div class="flex items-center">
                                    <svg class="flex-shrink-0 h-5 w-5 text-gray-300" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
                                        <path d="M5.555 17.776l8-16 .894.448-8 16-.894-.448z" />
                                    </svg>
                                    <span class="ml-4 text-sm font-medium text-gray-500 dark:text-gray-400">{"Community Dashboard"}</span>
                                </div>
                            </li>
                        </ol>
                    </nav>

                    <div class="mt-4 space-y-4 sm:space-y-0 sm:flex sm:items-center sm:justify-between">
                        <div class="flex items-center space-x-3 sm:space-x-4 min-w-0 flex-1">
                            <div class="flex-shrink-0">
                                <div class="w-12 h-12 sm:w-16 sm:h-16 bg-blue-500 rounded-xl flex items-center justify-center">
                                    <span class="text-white font-bold text-lg sm:text-2xl">
                                        {first_letter}
                                    </span>
                                </div>
                            </div>
                            <div class="min-w-0 flex-1">
                                <h1 class="text-xl sm:text-2xl lg:text-3xl font-bold text-gray-900 dark:text-white truncate">{&community.name}</h1>
                                <p class="mt-1 sm:mt-2 text-sm sm:text-base text-gray-600 dark:text-gray-300">
                                    {"Community overview and statistics"}
                                </p>
                            </div>
                        </div>
                        <div class="flex space-x-3">
                            <button
                                onclick={on_settings.clone()}
                                class="w-full sm:w-auto inline-flex items-center justify-center px-4 py-2 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                            >
                                <svg class="-ml-1 mr-2 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                                </svg>
                                {"Settings"}
                            </button>
                            <button
                                onclick={on_back_to_communities}
                                class="w-full sm:w-auto inline-flex items-center justify-center px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 hover:bg-gray-50 dark:hover:bg-gray-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                            >
                                {"Back to Communities"}
                            </button>
                        </div>
                    </div>
                </div>

                // Main dashboard content
                <div class="grid grid-cols-1 lg:grid-cols-3 gap-6 lg:gap-8">
                    // Main content area - Sites and Statistics
                    <div class="lg:col-span-2 space-y-6">
                        // Sites section
                        <div class="bg-white dark:bg-gray-800 shadow rounded-lg">
                            <div class="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                                <h2 class="text-lg font-medium text-gray-900 dark:text-white">{"Sites"}</h2>
                                <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
                                    {"Spaces and auctions in this community"}
                                </p>
                            </div>
                            <div class="p-6">
                                <div class="text-center py-8">
                                    <div class="mx-auto h-12 w-12 text-gray-400">
                                        <svg fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 11V9a1 1 0 011-1h2a1 1 0 011 1v11M7 21h4"></path>
                                        </svg>
                                    </div>
                                    <h3 class="mt-2 text-sm font-medium text-gray-900 dark:text-white">{"No sites yet"}</h3>
                                    <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
                                        {"Create your first site to start hosting auctions."}
                                    </p>
                                    <div class="mt-6">
                                        <button
                                            class="inline-flex items-center px-4 py-2 border border-transparent shadow-sm text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                                        >
                                            {"Create Site"}
                                        </button>
                                    </div>
                                </div>
                            </div>
                        </div>

                        // Statistics section
                        <div class="bg-white dark:bg-gray-800 shadow rounded-lg">
                            <div class="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                                <h2 class="text-lg font-medium text-gray-900 dark:text-white">{"Recent Activity"}</h2>
                                <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
                                    {"Community activity and updates"}
                                </p>
                            </div>
                            <div class="p-6">
                                <div class="text-center py-8">
                                    <div class="mx-auto h-12 w-12 text-gray-400">
                                        <svg fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
                                        </svg>
                                    </div>
                                    <h3 class="mt-2 text-sm font-medium text-gray-900 dark:text-white">{"No activity yet"}</h3>
                                    <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
                                        {"Activity will appear here as members join and sites are created."}
                                    </p>
                                </div>
                            </div>
                        </div>
                    </div>

                    // Sidebar - Quick Links and Community Info
                    <div class="space-y-6">
                        // Quick actions
                        <div class="bg-white dark:bg-gray-800 shadow rounded-lg p-6">
                            <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-4">{"Quick Actions"}</h3>
                            <div class="space-y-3">
                                <button
                                    onclick={on_settings}
                                    class="w-full flex items-center px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-50 dark:bg-gray-700 hover:bg-gray-100 dark:hover:bg-gray-600 rounded-md transition-colors"
                                >
                                    <svg class="mr-3 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                                    </svg>
                                    {"Community Settings"}
                                </button>
                                <button
                                    class="w-full flex items-center px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-50 dark:bg-gray-700 hover:bg-gray-100 dark:hover:bg-gray-600 rounded-md transition-colors"
                                >
                                    <svg class="mr-3 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 11V9a1 1 0 011-1h2a1 1 0 011 1v11M7 21h4"></path>
                                    </svg>
                                    {"Manage Sites"}
                                </button>
                                <button
                                    class="w-full flex items-center px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-50 dark:bg-gray-700 hover:bg-gray-100 dark:hover:bg-gray-600 rounded-md transition-colors"
                                >
                                    <svg class="mr-3 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z"></path>
                                    </svg>
                                    {"View Members"}
                                </button>
                            </div>
                        </div>

                        // Community info card
                        <div class="bg-white dark:bg-gray-800 shadow rounded-lg p-6">
                            <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-4">{"Community Info"}</h3>
                            <dl class="space-y-3">
                                // Remove new members status display for MVP
                                <div>
                                    <dt class="text-sm font-medium text-gray-500 dark:text-gray-400">{"Sites"}</dt>
                                    <dd class="text-sm text-gray-900 dark:text-white">{"0 sites"}</dd>
                                </div>
                                <div>
                                    <dt class="text-sm font-medium text-gray-500 dark:text-gray-400">{"Members"}</dt>
                                    <dd class="text-sm text-gray-900 dark:text-white">{"View in settings"}</dd>
                                </div>
                                <div>
                                    <dt class="text-sm font-medium text-gray-500 dark:text-gray-400">{"Created"}</dt>
                                    <dd class="text-sm text-gray-900 dark:text-white">{"Recently"}</dd>
                                </div>
                            </dl>
                        </div>
                    </div>
                </div>
            </div>
        </main>
    }
}
