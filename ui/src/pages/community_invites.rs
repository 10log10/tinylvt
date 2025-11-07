use payloads::{CommunityId, responses::CommunityWithRole};
use wasm_bindgen::JsCast;
use web_sys::{Event, FocusEvent, SubmitEvent};
use yew::prelude::*;

use crate::components::{ActiveTab, CommunityPageWrapper, CommunityTabHeader};
use crate::contexts::use_toast;
use crate::hooks::use_issued_invites;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
}

#[function_component]
pub fn CommunityInvitesPage(props: &Props) -> Html {
    let render_content = Callback::from(|community: CommunityWithRole| {
        html! {
            <div>
                <CommunityTabHeader community={community.clone()} active_tab={ActiveTab::Invites} />
                <div class="py-6">
                    <InvitesContent community={community.clone()} />
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
pub struct InvitesContentProps {
    pub community: CommunityWithRole,
}

#[function_component]
fn InvitesContent(props: &InvitesContentProps) -> Html {
    let issued_invites_hook = use_issued_invites(props.community.id);

    html! {
        <div class="space-y-6">
            <div class="flex justify-between items-center">
                <h2 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100">
                    {"Community Invites"}
                </h2>
                <InviteMemberButton community={props.community.clone()} on_invite_created={issued_invites_hook.refetch.clone()} />
            </div>

            // Display issued invites
            <div class="bg-white dark:bg-neutral-800 rounded-lg border border-neutral-200 dark:border-neutral-700">
                <div class="px-6 py-4 border-b border-neutral-200 dark:border-neutral-700">
                    <h3 class="text-lg font-medium text-neutral-900 dark:text-neutral-100">
                        {"Issued Invites"}
                    </h3>
                    <p class="text-sm text-neutral-600 dark:text-neutral-400 mt-1">
                        {"Outstanding invites created for this community"}
                    </p>
                </div>

                <div class="p-6">
                    {if issued_invites_hook.is_loading {
                        html! {
                            <div class="text-center py-8">
                                <p class="text-neutral-600 dark:text-neutral-400">{"Loading invites..."}</p>
                            </div>
                        }
                    } else if let Some(error) = &issued_invites_hook.error {
                        html! {
                            <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                                <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
                            </div>
                        }
                    } else if let Some(invites) = issued_invites_hook.invites.as_ref() {
                        if invites.is_empty() {
                            html! {
                                <div class="text-center py-8">
                                    <p class="text-neutral-600 dark:text-neutral-400">
                                        {"No outstanding invites."}
                                    </p>
                                </div>
                            }
                        } else {
                            html! {
                                <div class="space-y-4">
                                    {for invites.iter().map(|invite| {
                                        html! {
                                            <IssuedInviteCard
                                                invite={invite.clone()}
                                                community_id={props.community.id}
                                                on_invite_deleted={issued_invites_hook.refetch.clone()}
                                            />
                                        }
                                    })}
                                </div>
                            }
                        }
                    } else {
                        html! {
                            <div class="text-center py-8">
                                <p class="text-neutral-600 dark:text-neutral-400">{"Loading..."}</p>
                            </div>
                        }
                    }}
                </div>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct InviteMemberButtonProps {
    pub community: CommunityWithRole,
    pub on_invite_created: Callback<()>,
}

#[function_component]
fn InviteMemberButton(props: &InviteMemberButtonProps) -> Html {
    let show_modal = use_state(|| false);

    let on_click = {
        let show_modal = show_modal.clone();
        Callback::from(move |_| {
            show_modal.set(true);
        })
    };

    let on_close = {
        let show_modal = show_modal.clone();
        Callback::from(move |_: web_sys::MouseEvent| {
            show_modal.set(false);
        })
    };

    html! {
        <>
            <button
                onclick={on_click}
                class="bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors"
            >
                {"Invite Member"}
            </button>

            {if *show_modal {
                html! {
                    <InviteMemberModal
                        community={props.community.clone()}
                        on_close={on_close}
                        on_invite_created={props.on_invite_created.clone()}
                    />
                }
            } else {
                html! {}
            }}
        </>
    }
}

#[derive(Properties, PartialEq)]
pub struct InviteMemberModalProps {
    pub community: CommunityWithRole,
    pub on_close: Callback<web_sys::MouseEvent>,
    pub on_invite_created: Callback<()>,
}

#[function_component]
fn InviteMemberModal(props: &InviteMemberModalProps) -> Html {
    let email = use_state(String::new);
    let single_use = use_state(|| true);
    let is_loading = use_state(|| false);
    let error = use_state(|| None::<String>);
    let success = use_state(|| false);
    let invite_link = use_state(|| None::<String>);

    let on_email_change = {
        let email = email.clone();
        Callback::from(move |e: Event| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            email.set(input.value());
        })
    };

    let on_invite_type_change = {
        let single_use = single_use.clone();
        Callback::from(move |e: Event| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            single_use.set(input.value() == "single");
        })
    };

    let backdrop_ref = use_node_ref();

    let on_backdrop_click = {
        let on_close = props.on_close.clone();
        let backdrop_ref = backdrop_ref.clone();
        Callback::from(move |e: web_sys::MouseEvent| {
            // Only close if clicking the backdrop itself, not its children
            if let Some(backdrop_element) =
                backdrop_ref.cast::<web_sys::Element>()
                && let Some(target) = e.target()
                && target.dyn_ref::<web_sys::Element>()
                    == Some(&backdrop_element)
            {
                on_close.emit(e);
            }
        })
    };

    let on_submit = {
        let email = email.clone();
        let single_use = single_use.clone();
        let is_loading = is_loading.clone();
        let error = error.clone();
        let success = success.clone();
        let invite_link = invite_link.clone();
        let on_invite_created = props.on_invite_created.clone();
        let community_id = props.community.id;

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let email = email.clone();
            let single_use = single_use.clone();
            let is_loading = is_loading.clone();
            let error = error.clone();
            let success = success.clone();
            let invite_link = invite_link.clone();
            let on_invite_created = on_invite_created.clone();

            // No validation needed - all combinations are valid

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = crate::get_api_client();
                let invite_details =
                    payloads::requests::InviteCommunityMember {
                        community_id,
                        new_member_email: if email.trim().is_empty() {
                            None
                        } else {
                            Some((*email).clone())
                        },
                        single_use: *single_use,
                    };

                match api_client.invite_member(&invite_details).await {
                    Ok(invite_id) => {
                        // Build the full invite URL
                        let window = web_sys::window().unwrap();
                        let location = window.location();
                        let origin = location.origin().unwrap();
                        let full_invite_link =
                            format!("{}/accept-invite/{}", origin, invite_id);

                        invite_link.set(Some(full_invite_link));
                        success.set(true);
                        email.set(String::new());

                        // Refetch the issued invites to show the new invite
                        on_invite_created.emit(());
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    html! {
        <div
            ref={backdrop_ref}
            class="fixed inset-0 bg-neutral-900 bg-opacity-50 flex items-center justify-center z-50"
            onclick={on_backdrop_click}
        >
            <div class="bg-white dark:bg-neutral-800 p-6 rounded-lg shadow-xl max-w-md w-full mx-4 border border-neutral-200 dark:border-neutral-700">
                <div class="flex justify-between items-center mb-4">
                    <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100">
                        {"Invite Member"}
                    </h3>
                    <button
                        onclick={props.on_close.clone()}
                        class="text-neutral-500 hover:text-neutral-700 dark:text-neutral-400 dark:hover:text-neutral-200 text-2xl leading-none p-1"
                        title="Close"
                    >
                        {"×"}
                    </button>
                </div>

                {if *success {
                    html! {
                        <div class="p-4 rounded-md bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 mb-4">
                            <div class="space-y-3">
                                <p class="text-sm text-green-700 dark:text-green-400 font-medium">
                                    { if *single_use { "Single-use invite link created successfully!" } else { "Multi-use invite link created successfully!" } }
                                </p>

                                {if let Some(ref link) = *invite_link {
                                    html! {
                                        <div class="space-y-2">
                                            <label class="block text-xs font-medium text-green-700 dark:text-green-400">
                                                {"Invite Link:"}
                                            </label>
                                            <div>
                                                <input
                                                    type="text"
                                                    value={link.clone()}
                                                    readonly={true}
                                                    onfocus={Callback::from(move |e: FocusEvent| {
                                                        if let Some(target) = e.target()
                                                            && let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                                                                input.select();
                                                            }
                                                    })}
                                                    class="w-full px-2 py-1 text-xs border border-green-300 dark:border-green-600 rounded bg-green-50 dark:bg-green-900/30 text-green-800 dark:text-green-200 font-mono cursor-pointer"
                                                    title="Click to select all, then copy"
                                                />
                                                <p class="text-xs text-green-600 dark:text-green-400 mt-1">
                                                    {"Click the link above to select all, then copy (Ctrl+C / Cmd+C)"}
                                                </p>
                                            </div>
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }}

                                <button
                                    type="button"
                                    onclick={{
                                        let success = success.clone();
                                        let invite_link = invite_link.clone();
                                        let error = error.clone();
                                        Callback::from(move |_| {
                                            success.set(false);
                                            invite_link.set(None);
                                            error.set(None);
                                        })
                                    }}
                                    class="text-xs font-medium text-green-700 dark:text-green-300 hover:text-green-800 dark:hover:text-green-200 underline"
                                >
                                    {"Create Another Invite"}
                                </button>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }}

                {if let Some(error_msg) = error.as_ref() {
                    html! {
                        <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 mb-4">
                            <p class="text-sm text-red-700 dark:text-red-400">{error_msg}</p>
                        </div>
                    }
                } else {
                    html! {}
                }}

                {if !*success {
                    html! {
                        <form onsubmit={on_submit} class="space-y-4">
                    <div>
                        <fieldset>
                            <legend class="text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-3">
                                {"Invite Type"}
                            </legend>
                            <div class="space-y-2">
                                <label class="flex items-center space-x-2">
                                    <input
                                        type="radio"
                                        name="invite_type"
                                        value="single"
                                        checked={*single_use}
                                        onchange={on_invite_type_change.clone()}
                                        disabled={*is_loading}
                                        class="w-4 h-4 text-neutral-600 bg-white dark:bg-neutral-700 border-neutral-300 dark:border-neutral-600 focus:ring-neutral-500 dark:focus:ring-neutral-400"
                                    />
                                    <span class="text-sm text-neutral-700 dark:text-neutral-300">
                                        {"Single-use invite"}
                                    </span>
                                </label>
                                <label class="flex items-center space-x-2">
                                    <input
                                        type="radio"
                                        name="invite_type"
                                        value="multi"
                                        checked={!*single_use}
                                        onchange={on_invite_type_change}
                                        disabled={*is_loading}
                                        class="w-4 h-4 text-neutral-600 bg-white dark:bg-neutral-700 border-neutral-300 dark:border-neutral-600 focus:ring-neutral-500 dark:focus:ring-neutral-400"
                                    />
                                    <span class="text-sm text-neutral-700 dark:text-neutral-300">
                                        {"Multi-use invite"}
                                    </span>
                                </label>
                            </div>
                        </fieldset>

                        {if *single_use {
                            html! {
                                <div class="mt-2 p-3 bg-neutral-50 dark:bg-neutral-700/50 rounded-md">
                                    <p class="text-xs text-neutral-600 dark:text-neutral-400 leading-relaxed">
                                        {"Single-use invites can only be used once. You can optionally specify a recipient email address - if provided, they must create an account with that exact email and will receive a notification."}
                                    </p>
                                </div>
                            }
                        } else {
                            html! {
                                <div class="mt-2 p-3 bg-neutral-50 dark:bg-neutral-700/50 rounded-md">
                                    <p class="text-xs text-neutral-600 dark:text-neutral-400 leading-relaxed">
                                        {"Multi-use invites are reusable invite links that can be shared with anyone."}
                                    </p>
                                </div>
                            }
                        }}
                    </div>

                    {if *single_use {
                        html! {
                            <div>
                                <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                                    {"Recipient Email"} <span class="ml-2 text-neutral-400">{"(optional)"}</span>
                                </label>
                                <input
                                    type="email"
                                    value={(*email).clone()}
                                    onchange={on_email_change}
                                    disabled={*is_loading}
                                    class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600 rounded-md text-neutral-900 dark:text-neutral-100 bg-white dark:bg-neutral-700 focus:outline-none focus:ring-2 focus:ring-neutral-500 dark:focus:ring-neutral-400"
                                    placeholder="user@example.com (optional)"
                                />
                            </div>
                        }
                    } else {
                        html! {}
                    }}

                    <div class="flex justify-end space-x-3">
                        <button
                            type="button"
                            onclick={props.on_close.clone()}
                            disabled={*is_loading}
                            class="px-4 py-2 text-sm font-medium text-neutral-700 dark:text-neutral-300 bg-white dark:bg-neutral-700 border border-neutral-300 dark:border-neutral-600 rounded-md hover:bg-neutral-50 dark:hover:bg-neutral-600 disabled:opacity-50"
                        >
                            {"Cancel"}
                        </button>
                        <button
                            type="submit"
                            disabled={*is_loading}
                            class="px-4 py-2 text-sm font-medium text-white bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 rounded-md disabled:opacity-50"
                        >
                            {if *is_loading {
                                "Creating Link..."
                            } else if !*single_use || email.trim().is_empty() {
                                "Create Link"
                            } else {
                                "Create Link & Send Invitation"
                            }}
                        </button>
                    </div>
                        </form>
                    }
                } else {
                    html! {}
                }}
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct IssuedInviteCardProps {
    pub invite: payloads::responses::IssuedCommunityInvite,
    pub community_id: payloads::CommunityId,
    pub on_invite_deleted: Callback<()>,
}

#[function_component]
fn IssuedInviteCard(props: &IssuedInviteCardProps) -> Html {
    let invite = &props.invite;
    let is_deleting = use_state(|| false);
    let toast = use_toast();

    // Format timestamp for display
    let created_date = {
        use jiff::tz;
        let system_tz = tz::TimeZone::system();
        let zoned = invite.created_at.to_zoned(system_tz);
        zoned.strftime("%B %d, %Y at %l:%M %p").to_string()
    };

    let on_delete_click = {
        let is_deleting = is_deleting.clone();
        let community_id = props.community_id;
        let invite_id = invite.id;
        let on_invite_deleted = props.on_invite_deleted.clone();
        let toast = toast.clone();

        Callback::from(move |_| {
            let is_deleting = is_deleting.clone();
            let on_invite_deleted = on_invite_deleted.clone();
            let toast = toast.clone();

            is_deleting.set(true);

            yew::platform::spawn_local(async move {
                let api_client = crate::get_api_client();
                let delete_details = payloads::requests::DeleteInvite {
                    community_id,
                    invite_id,
                };

                match api_client.delete_invite(&delete_details).await {
                    Ok(()) => {
                        // Show success toast and refetch invites
                        toast.success("Invite deleted successfully");
                        on_invite_deleted.emit(());
                    }
                    Err(err) => {
                        toast
                            .error(format!("Failed to delete invite: {}", err));
                        is_deleting.set(false);
                    }
                }
            });
        })
    };

    html! {
        <div class="p-4 border border-neutral-200 dark:border-neutral-700 rounded-lg">
            <div class="flex justify-between items-center mb-2">
                <div class="flex items-center space-x-3">
                    <span class={
                        format!(
                            "inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium {}",
                            if invite.single_use {
                                "bg-blue-100 text-blue-800 dark:bg-blue-900/20 dark:text-blue-400"
                            } else {
                                "bg-green-100 text-green-800 dark:bg-green-900/20 dark:text-green-400"
                            }
                        )
                    }>
                        {if invite.single_use { "Single-use" } else { "Multi-use" }}
                    </span>

                    {if let Some(ref email) = invite.new_member_email {
                        html! {
                            <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-neutral-100 text-neutral-800 dark:bg-neutral-700 dark:text-neutral-300">
                                {email}
                            </span>
                        }
                    } else {
                        html! {}
                    }}
                </div>

                <button
                    onclick={on_delete_click}
                    disabled={*is_deleting}
                    class="px-3 py-1 text-xs font-medium text-red-700 dark:text-red-400 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-md hover:bg-red-100 dark:hover:bg-red-900/30 disabled:opacity-50 disabled:cursor-not-allowed"
                    title="Delete this invite"
                >
                    {if *is_deleting { "Deleting..." } else { "Delete" }}
                </button>
            </div>

            <div class="text-sm text-neutral-600 dark:text-neutral-400">
                <p class="mb-2">{format!("Created on {}", created_date)}</p>
                <div class="flex items-center space-x-2">
                    <label class="text-xs font-medium text-neutral-600 dark:text-neutral-400 whitespace-nowrap">
                        {"Invite Link:"}
                    </label>
                    <input
                        type="text"
                        value={{
                            let window = web_sys::window().unwrap();
                            let location = window.location();
                            let origin = location.origin().unwrap();
                            format!("{}/accept-invite/{}", origin, invite.id)
                        }}
                        readonly={true}
                        onfocus={Callback::from(move |e: FocusEvent| {
                            if let Some(target) = e.target()
                                && let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                                    input.select();
                                }
                        })}
                        class="flex-1 px-2 py-1 text-xs border border-neutral-300 dark:border-neutral-600 rounded bg-neutral-50 dark:bg-neutral-700 text-neutral-800 dark:text-neutral-200 font-mono cursor-pointer"
                        title="Click to select all, then copy"
                    />
                </div>
            </div>
        </div>
    }
}
