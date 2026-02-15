use payloads::{CommunityId, Role, requests, responses};
use yew::prelude::*;

use crate::components::user_identity_display::render_user_name;
use crate::get_api_client;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    pub member: responses::CommunityMember,
    pub actor_role: Role,
    pub on_success: Callback<()>,
    pub on_close: Callback<()>,
}

/// Modal for changing a member's role. Should be rendered conditionally by
/// parent when role change is requested.
#[function_component]
pub fn ChangeRoleModal(props: &Props) -> Html {
    // Get the roles that this actor can change the target to
    let available_roles =
        get_available_roles(&props.actor_role, &props.member.role);

    // Default to first available role (not the member's current role)
    let selected_role =
        use_state(|| available_roles.first().copied().unwrap_or(Role::Member));
    let is_submitting = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    let on_role_change = {
        let selected_role = selected_role.clone();
        Callback::from(move |e: Event| {
            let target = e.target_dyn_into::<web_sys::HtmlSelectElement>();
            if let Some(select) = target
                && let Ok(role) = select.value().parse::<Role>()
            {
                selected_role.set(role);
            }
        })
    };

    let on_confirm = {
        let community_id = props.community_id;
        let member_user_id = props.member.user.user_id;
        let selected_role = selected_role.clone();
        let is_submitting = is_submitting.clone();
        let error_message = error_message.clone();
        let on_success = props.on_success.clone();

        Callback::from(move |_| {
            let selected_role = *selected_role;
            let is_submitting = is_submitting.clone();
            let error_message = error_message.clone();
            let on_success = on_success.clone();

            yew::platform::spawn_local(async move {
                is_submitting.set(true);
                error_message.set(None);

                let request = requests::ChangeMemberRole {
                    community_id,
                    member_user_id,
                    new_role: selected_role,
                };

                match get_api_client().change_member_role(&request).await {
                    Ok(_) => {
                        on_success.emit(());
                    }
                    Err(e) => {
                        error_message
                            .set(Some(format!("Failed to change role: {}", e)));
                    }
                }

                is_submitting.set(false);
            });
        })
    };

    let on_cancel = {
        let on_close = props.on_close.clone();
        let error_message = error_message.clone();
        Callback::from(move |_| {
            error_message.set(None);
            on_close.emit(());
        })
    };

    let on_backdrop_click = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| {
            on_close.emit(());
        })
    };

    // Disable confirm if no available roles
    let can_confirm = !available_roles.is_empty();

    html! {
        <div
            onclick={on_backdrop_click}
            class="fixed inset-0 bg-black bg-opacity-50 flex \
                   items-center justify-center z-50"
        >
            <div
                onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                class="bg-white dark:bg-neutral-800 p-6 rounded-lg \
                       max-w-md w-full mx-4"
            >
                <h3 class="text-lg font-semibold mb-4 text-neutral-900 \
                           dark:text-neutral-100">
                    {"Change Member Role"}
                </h3>
                <p class="mb-4 text-neutral-700 dark:text-neutral-300">
                    {"Change role for "}
                    <span class="font-medium">
                        {render_user_name(&props.member.user)}
                    </span>
                    {":"}
                </p>

                <div class="mb-4">
                    <label
                        class="block text-sm font-medium text-neutral-700 \
                               dark:text-neutral-300 mb-2"
                    >
                        {"New Role"}
                    </label>
                    <select
                        onchange={on_role_change}
                        disabled={*is_submitting}
                        class="w-full px-3 py-2 border border-neutral-300 \
                               dark:border-neutral-600 rounded-md \
                               bg-white dark:bg-neutral-700 \
                               text-neutral-900 dark:text-neutral-100 \
                               focus:outline-none focus:ring-2 \
                               focus:ring-neutral-500 \
                               disabled:opacity-50"
                    >
                        {for available_roles.iter().map(|role| {
                            let role_str = role.to_string();
                            let is_selected = *role == *selected_role;
                            html! {
                                <option
                                    value={role_str.clone()}
                                    selected={is_selected}
                                >
                                    {role_str}
                                </option>
                            }
                        })}
                    </select>
                </div>

                {if let Some(error) = (*error_message).clone() {
                    html! {
                        <div class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 \
                                    border border-red-200 dark:border-red-800 \
                                    rounded">
                            <p class="text-sm text-red-700 dark:text-red-400">
                                {error}
                            </p>
                        </div>
                    }
                } else {
                    html! {}
                }}

                <div class="flex gap-3 justify-end">
                    <button
                        onclick={on_cancel}
                        disabled={*is_submitting}
                        class="px-4 py-2 bg-neutral-200 hover:bg-neutral-300 \
                               dark:bg-neutral-700 dark:hover:bg-neutral-600 \
                               text-neutral-900 dark:text-neutral-100 \
                               rounded disabled:opacity-50"
                    >
                        {"Cancel"}
                    </button>
                    <button
                        onclick={on_confirm}
                        disabled={*is_submitting || !can_confirm}
                        class="px-4 py-2 bg-neutral-900 hover:bg-neutral-800 \
                               dark:bg-neutral-100 dark:hover:bg-neutral-200 \
                               text-white dark:text-neutral-900 \
                               rounded disabled:opacity-50"
                    >
                        {if *is_submitting {
                            "Changing..."
                        } else {
                            "Change Role"
                        }}
                    </button>
                </div>
            </div>
        </div>
    }
}

/// Get the roles that the actor can change the target to.
/// Returns roles in order from lowest to highest.
fn get_available_roles(actor_role: &Role, target_role: &Role) -> Vec<Role> {
    let all_roles = [Role::Member, Role::Moderator, Role::Coleader];
    all_roles
        .into_iter()
        .filter(|new_role| actor_role.can_change_role(target_role, new_role))
        .collect()
}
