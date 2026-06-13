use jiff::{Span, Timestamp};
use payloads::{AuctionStatus, Role, requests, responses};
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::{
    Route, components::ConfirmationModal, get_api_client,
    hooks::use_push_route, utils::time::parse_datetime_local,
};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub auction: responses::Auction,
    pub user_role: Role,
}

/// The three modal-confirmed lifecycle actions.
#[derive(Clone, Copy, PartialEq)]
enum AdminAction {
    Start,
    Cancel,
    Delete,
}

/// "Starting now" schedules the start this many seconds out rather than
/// exactly at now, so every viewer (including the actor, after the request
/// and SSE round trips) sees a brief countdown instead of an abrupt jump
/// into round 0.
const START_NOW_DELAY_SECONDS: i64 = 15;

/// Coleader+ controls for managing an auction's lifecycle: start it (a
/// short countdown away), set/change/clear a scheduled start time, cancel
/// it, and permanently delete it once canceled. Renders nothing for other
/// roles or for concluded auctions.
///
/// Successful start/schedule/cancel actions don't refetch anything locally:
/// the backend emits SSE events (`AuctionScheduleChanged` / `AuctionEnded`)
/// that drive the subscribed auction hooks for every viewer, including the
/// actor.
#[function_component]
pub fn AuctionAdminControls(props: &Props) -> Html {
    let error_message = use_state(|| None::<String>);
    let is_submitting = use_state(|| false);
    let show_start_modal = use_state(|| false);
    let show_cancel_modal = use_state(|| false);
    let show_delete_modal = use_state(|| false);
    let schedule_input_ref = use_node_ref();
    let push_route = use_push_route();

    if !props.user_role.is_ge_coleader() {
        return html! {};
    }

    let auction_id = props.auction.auction_id;
    let site_id = props.auction.auction_details.site_id;
    let status = props.auction.status(Timestamp::now());

    // A small helper that runs an API action, surfacing errors in the shared
    // error banner and closing the given modal on success.
    let run_action = |action: AdminAction, modal: UseStateHandle<bool>| {
        let error_message = error_message.clone();
        let is_submitting = is_submitting.clone();
        let push_route = push_route.clone();

        Callback::from(move |_: ()| {
            let error_message = error_message.clone();
            let is_submitting = is_submitting.clone();
            let modal = modal.clone();
            let push_route = push_route.clone();

            yew::platform::spawn_local(async move {
                is_submitting.set(true);
                error_message.set(None);

                let api_client = get_api_client();
                let result = match action {
                    AdminAction::Start => {
                        let start_at = Timestamp::now()
                            + Span::new().seconds(START_NOW_DELAY_SECONDS);
                        api_client
                            .schedule_auction(&requests::ScheduleAuction {
                                auction_id,
                                start_at: Some(start_at),
                            })
                            .await
                    }
                    AdminAction::Cancel => {
                        api_client.cancel_auction(&auction_id).await
                    }
                    AdminAction::Delete => {
                        api_client.delete_auction(&auction_id).await
                    }
                };

                is_submitting.set(false);
                match result {
                    Ok(()) => {
                        modal.set(false);
                        if action == AdminAction::Delete {
                            push_route
                                .emit(Route::SiteAuctions { id: site_id });
                        }
                    }
                    Err(e) => {
                        let verb = match action {
                            AdminAction::Start => "start",
                            AdminAction::Cancel => "cancel",
                            AdminAction::Delete => "delete",
                        };
                        error_message.set(Some(format!(
                            "Failed to {} auction: {}",
                            verb, e
                        )));
                    }
                }
            });
        })
    };

    let on_start = run_action(AdminAction::Start, show_start_modal.clone());
    let on_cancel = run_action(AdminAction::Cancel, show_cancel_modal.clone());
    let on_delete = run_action(AdminAction::Delete, show_delete_modal.clone());

    let on_save_schedule = {
        let schedule_input_ref = schedule_input_ref.clone();
        let error_message = error_message.clone();
        let is_submitting = is_submitting.clone();

        Callback::from(move |_: MouseEvent| {
            let input = schedule_input_ref.cast::<HtmlInputElement>().unwrap();
            let value = input.value();

            if value.is_empty() {
                error_message
                    .set(Some("Enter a start time to schedule".into()));
                return;
            }

            let start_at = match parse_datetime_local(&value, None) {
                Ok(ts) => ts,
                Err(e) => {
                    error_message.set(Some(format!("Invalid start time: {e}")));
                    return;
                }
            };
            if start_at <= Timestamp::now() {
                error_message
                    .set(Some("Start time must be in the future".into()));
                return;
            }

            let error_message = error_message.clone();
            let is_submitting = is_submitting.clone();
            yew::platform::spawn_local(async move {
                is_submitting.set(true);
                error_message.set(None);

                let api_client = get_api_client();
                if let Err(e) = api_client
                    .schedule_auction(&requests::ScheduleAuction {
                        auction_id,
                        start_at: Some(start_at),
                    })
                    .await
                {
                    error_message
                        .set(Some(format!("Failed to schedule: {}", e)));
                }
                is_submitting.set(false);
            });
        })
    };

    let on_clear_schedule = {
        let error_message = error_message.clone();
        let is_submitting = is_submitting.clone();

        Callback::from(move |_: MouseEvent| {
            let error_message = error_message.clone();
            let is_submitting = is_submitting.clone();
            yew::platform::spawn_local(async move {
                is_submitting.set(true);
                error_message.set(None);

                let api_client = get_api_client();
                if let Err(e) = api_client
                    .schedule_auction(&requests::ScheduleAuction {
                        auction_id,
                        start_at: None,
                    })
                    .await
                {
                    error_message
                        .set(Some(format!("Failed to clear schedule: {}", e)));
                }
                is_submitting.set(false);
            });
        })
    };

    let close_modal = |modal: &UseStateHandle<bool>| {
        let modal = modal.clone();
        let error_message = error_message.clone();
        Callback::from(move |_: ()| {
            modal.set(false);
            error_message.set(None);
        })
    };

    // Prefill the schedule input from the current scheduled start, shown in
    // the user's local timezone (auction times are user-local by convention).
    let schedule_prefill = props.auction.auction_details.start_at.map(|ts| {
        ts.to_zoned(jiff::tz::TimeZone::system())
            .strftime("%Y-%m-%dT%H:%M")
            .to_string()
    });

    let secondary_button_classes = "px-4 py-2 text-sm font-medium \
        text-neutral-700 dark:text-neutral-300 bg-white dark:bg-neutral-700 \
        border border-neutral-300 dark:border-neutral-600 rounded-md \
        hover:bg-neutral-50 dark:hover:bg-neutral-600 disabled:opacity-50 \
        disabled:cursor-not-allowed transition-colors";
    let destructive_button_classes = "px-4 py-2 text-sm font-medium \
        text-red-700 dark:text-red-400 bg-white dark:bg-transparent border \
        border-red-300 dark:border-red-900 rounded-md hover:bg-red-50 \
        dark:hover:bg-red-900/20 disabled:opacity-50 \
        disabled:cursor-not-allowed transition-colors";

    let body = match status {
        AuctionStatus::NotScheduled | AuctionStatus::Upcoming => html! {
            <>
                <div class="space-y-2">
                    <label
                        for="schedule-start"
                        class="block text-sm font-medium text-neutral-700 \
                               dark:text-neutral-300"
                    >
                        {"Scheduled start time"}
                    </label>
                    <div class="flex flex-wrap gap-3 items-center">
                        <input
                            ref={schedule_input_ref}
                            type="datetime-local"
                            id="schedule-start"
                            value={schedule_prefill.clone()}
                            disabled={*is_submitting}
                            class="px-3 py-2 border border-neutral-300 \
                                   dark:border-neutral-600 rounded-md \
                                   shadow-sm bg-white dark:bg-neutral-700 \
                                   text-neutral-900 dark:text-neutral-100 \
                                   focus:outline-none focus:ring-2 \
                                   focus:ring-neutral-500 \
                                   dark:focus:ring-neutral-400"
                        />
                        <button
                            onclick={on_save_schedule}
                            disabled={*is_submitting}
                            class={secondary_button_classes}
                        >
                            {"Save schedule"}
                        </button>
                        {if schedule_prefill.is_some() {
                            html! {
                                <button
                                    onclick={on_clear_schedule}
                                    disabled={*is_submitting}
                                    class={secondary_button_classes}
                                >
                                    {"Clear schedule"}
                                </button>
                            }
                        } else {
                            html! {}
                        }}
                    </div>
                    <p class="text-xs text-neutral-500 dark:text-neutral-400">
                        {"Times are in your local timezone. The auction can \
                          also be started immediately below."}
                    </p>
                </div>
                <div class="flex flex-wrap gap-3 pt-2 border-t \
                            border-neutral-200 dark:border-neutral-700">
                    <button
                        onclick={
                            let show_start_modal = show_start_modal.clone();
                            Callback::from(move |_| {
                                show_start_modal.set(true)
                            })
                        }
                        disabled={*is_submitting}
                        class="px-4 py-2 text-sm font-medium text-white \
                               bg-neutral-900 hover:bg-neutral-800 \
                               dark:bg-neutral-100 dark:text-neutral-900 \
                               dark:hover:bg-neutral-200 rounded-md \
                               disabled:opacity-50 \
                               disabled:cursor-not-allowed transition-colors"
                    >
                        {"Start auction now"}
                    </button>
                    <button
                        onclick={
                            let show_cancel_modal = show_cancel_modal.clone();
                            Callback::from(move |_| {
                                show_cancel_modal.set(true)
                            })
                        }
                        disabled={*is_submitting}
                        class={destructive_button_classes}
                    >
                        {"Cancel auction"}
                    </button>
                </div>
            </>
        },
        AuctionStatus::Ongoing => html! {
            <div class="flex flex-wrap gap-3">
                <button
                    onclick={
                        let show_cancel_modal = show_cancel_modal.clone();
                        Callback::from(move |_| show_cancel_modal.set(true))
                    }
                    disabled={*is_submitting}
                    class={destructive_button_classes}
                >
                    {"Cancel auction"}
                </button>
            </div>
        },
        AuctionStatus::Canceled => html! {
            <div class="flex flex-wrap gap-3">
                <button
                    onclick={
                        let show_delete_modal = show_delete_modal.clone();
                        Callback::from(move |_| show_delete_modal.set(true))
                    }
                    disabled={*is_submitting}
                    class={destructive_button_classes}
                >
                    {"Delete permanently"}
                </button>
            </div>
        },
        AuctionStatus::Concluded => return html! {},
    };

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-6 bg-white dark:bg-neutral-800 space-y-4">
            <h3 class="text-sm font-medium text-neutral-700 \
                       dark:text-neutral-300 uppercase tracking-wide">
                {"Manage Auction"}
            </h3>

            {if let Some(error) = &*error_message {
                html! {
                    <div class="p-3 rounded-md bg-red-50 dark:bg-red-900/20 \
                                border border-red-200 dark:border-red-800">
                        <p class="text-sm text-red-700 dark:text-red-400">
                            {error}
                        </p>
                    </div>
                }
            } else {
                html! {}
            }}

            {body}

            {if *show_start_modal {
                html! {
                    <ConfirmationModal
                        title="Start auction now?"
                        message={format!(
                            "The first bidding round begins after a \
                             {START_NOW_DELAY_SECONDS}-second countdown and \
                             members can start placing bids."
                        )}
                        confirm_text="Start Auction"
                        on_confirm={on_start}
                        on_close={close_modal(&show_start_modal)}
                        is_loading={*is_submitting}
                        is_irreversible={false}
                    />
                }
            } else {
                html! {}
            }}

            {if *show_cancel_modal {
                html! {
                    <ConfirmationModal
                        title="Cancel auction?"
                        message="The auction stops immediately: any standing \
                                 bids are discarded, no spaces are allocated, \
                                 and no settlement occurs. The auction stays \
                                 visible to members as canceled."
                        confirm_text="Cancel Auction"
                        on_confirm={on_cancel}
                        on_close={close_modal(&show_cancel_modal)}
                        is_loading={*is_submitting}
                    />
                }
            } else {
                html! {}
            }}

            {if *show_delete_modal {
                html! {
                    <ConfirmationModal
                        title="Delete auction permanently?"
                        message="The canceled auction and its round and bid \
                                 history are removed permanently."
                        confirm_text="Delete Auction"
                        on_confirm={on_delete}
                        on_close={close_modal(&show_delete_modal)}
                        is_loading={*is_submitting}
                    />
                }
            } else {
                html! {}
            }}
        </div>
    }
}
