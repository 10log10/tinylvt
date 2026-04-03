use payloads::{CommunityId, CommunityStorageUsage, SubscriptionTier};
use yew::prelude::*;

use crate::hooks::use_storage_usage;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub usage: CommunityStorageUsage,
}

/// Section component that fetches and displays storage usage.
/// Should only be rendered for coleader+ users.
#[derive(Properties, PartialEq)]
pub struct StorageUsageSectionProps {
    pub community_id: CommunityId,
}

#[function_component]
pub fn StorageUsageSection(props: &StorageUsageSectionProps) -> Html {
    let storage_hook = use_storage_usage(props.community_id);

    // Refetch storage usage on mount to ensure we have the latest data
    // from the backend when viewing the detailed settings page.
    {
        let refetch = storage_hook.refetch.clone();
        use_effect_with(props.community_id, move |_| {
            refetch.emit(());
        });
    }

    html! {
        <div>
            <h2 class="text-xl font-semibold text-neutral-900 \
                       dark:text-neutral-100 mb-6">
                {"Storage Usage"}
            </h2>

            {storage_hook.render("storage usage", |usage, is_loading, error| {
                html! {
                    <div>
                        {if let Some(err) = error {
                            html! {
                                <div class="p-4 rounded-md bg-red-50 \
                                            dark:bg-red-900/20 border \
                                            border-red-200 dark:border-red-800">
                                    <p class="text-sm text-red-700 \
                                              dark:text-red-400">
                                        {err}
                                    </p>
                                </div>
                            }
                        } else {
                            html! {}
                        }}
                        <div class={classes!(
                            is_loading.then_some("opacity-50")
                        )}>
                            <StorageUsageDisplay usage={usage.clone()} />
                        </div>
                    </div>
                }
            })}
        </div>
    }
}

/// Formats bytes as human-readable string (KB, MB, GB).
pub fn format_bytes(bytes: i64) -> String {
    const KB: i64 = 1_000;
    const MB: i64 = 1_000_000;
    const GB: i64 = 1_000_000_000;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// A single row in the storage breakdown.
#[derive(Properties, PartialEq)]
struct StorageRowProps {
    label: &'static str,
    bytes: i64,
    total_bytes: i64,
}

#[function_component]
fn StorageRow(props: &StorageRowProps) -> Html {
    let percentage = if props.total_bytes > 0 {
        (props.bytes as f64 / props.total_bytes as f64) * 100.0
    } else {
        0.0
    };

    // Width for the bar (as percentage of total)
    let bar_width = format!("{}%", percentage.min(100.0));

    html! {
        <div class="flex items-center gap-3 text-sm">
            <span class="w-28 text-neutral-600 dark:text-neutral-400">
                {props.label}
            </span>
            <span class="w-20 text-right font-mono text-neutral-900
                         dark:text-neutral-100">
                {format_bytes(props.bytes)}
            </span>
            <div class="flex-1 h-2 bg-neutral-200 dark:bg-neutral-700
                        rounded-full overflow-hidden">
                <div
                    class="h-full bg-neutral-500 dark:bg-neutral-400
                           rounded-full"
                    style={format!("width: {}", bar_width)}
                />
            </div>
        </div>
    }
}

#[function_component]
pub fn StorageUsageDisplay(props: &Props) -> Html {
    let usage = &props.usage.usage;
    let total = usage.total_bytes();
    let limit = props.usage.limits.storage_bytes;
    let usage_percent = props.usage.usage_percentage();

    // Determine color based on usage percentage
    let bar_color = if usage_percent >= 90.0 {
        "bg-red-500 dark:bg-red-400"
    } else if usage_percent >= 75.0 {
        "bg-amber-500 dark:bg-amber-400"
    } else {
        "bg-neutral-600 dark:bg-neutral-400"
    };

    let tier_label = match props.usage.tier {
        SubscriptionTier::Paid => "Paid tier",
        SubscriptionTier::Free => "Free tier",
    };

    html! {
        <div class="space-y-4">
            // Overall usage header
            <div class="flex items-baseline justify-between">
                <div>
                    <span class="text-2xl font-bold text-neutral-900
                                 dark:text-neutral-100">
                        {format_bytes(total)}
                    </span>
                    <span class="text-neutral-500 dark:text-neutral-400 ml-1">
                        {" / "}
                        {format_bytes(limit)}
                    </span>
                </div>
                <span class="text-sm text-neutral-500 dark:text-neutral-400">
                    {tier_label}
                </span>
            </div>

            // Overall progress bar
            <div class="h-3 bg-neutral-200 dark:bg-neutral-700 rounded-full
                        overflow-hidden">
                <div
                    class={classes!("h-full", "rounded-full", bar_color)}
                    style={format!("width: {}%", usage_percent.min(100.0))}
                />
            </div>

            // Warning if near limit
            {if usage_percent >= 90.0 {
                html! {
                    <div class="p-3 rounded-md bg-red-50 dark:bg-red-900/20
                                border border-red-200 dark:border-red-800">
                        <p class="text-sm text-red-700 dark:text-red-400">
                            {"Storage nearly full. Consider upgrading or \
                              deleting unused data."}
                        </p>
                    </div>
                }
            } else if usage_percent >= 75.0 {
                html! {
                    <div class="p-3 rounded-md bg-amber-50 dark:bg-amber-900/20
                                border border-amber-200 dark:border-amber-800">
                        <p class="text-sm text-amber-700 dark:text-amber-400">
                            {"Approaching storage limit."}
                        </p>
                    </div>
                }
            } else {
                html! {}
            }}

            // Breakdown by category
            <div class="space-y-2 pt-2">
                <StorageRow
                    label="Images"
                    bytes={usage.image_bytes}
                    total_bytes={total}
                />
                <StorageRow
                    label="Auctions"
                    bytes={usage.auction_bytes}
                    total_bytes={total}
                />
                <StorageRow
                    label="Transactions"
                    bytes={usage.transaction_bytes}
                    total_bytes={total}
                />
                <StorageRow
                    label="Members"
                    bytes={usage.member_bytes}
                    total_bytes={total}
                />
                <StorageRow
                    label="Spaces"
                    bytes={usage.space_bytes}
                    total_bytes={total}
                />
            </div>
        </div>
    }
}
