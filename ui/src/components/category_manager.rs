use payloads::{CommunityId, SpaceCategory, requests, responses};
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::components::InlineEdit;

/// Coleader management of a community's space categories: add, rename,
/// delete. Categories label spaces for per-bidder bidding caps in capped
/// auctions. The parent owns the categories fetch and passes a refetch
/// callback so sibling consumers (space forms) stay in sync.
#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    pub categories: Vec<responses::SpaceCategory>,
    pub refetch: Callback<()>,
}

#[function_component]
pub fn CategoryManager(props: &Props) -> Html {
    let new_name = use_state(String::new);
    let error = use_state(|| None::<String>);

    let on_add = {
        let community_id = props.community_id;
        let new_name = new_name.clone();
        let error = error.clone();
        let refetch = props.refetch.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let name = new_name.trim().to_string();
            if name.is_empty() {
                return;
            }
            let new_name = new_name.clone();
            let error = error.clone();
            let refetch = refetch.clone();
            yew::platform::spawn_local(async move {
                error.set(None);
                let api_client = crate::get_api_client();
                match api_client
                    .create_space_category(&SpaceCategory {
                        community_id,
                        name,
                    })
                    .await
                {
                    Ok(_) => {
                        new_name.set(String::new());
                        refetch.emit(());
                    }
                    Err(e) => error.set(Some(e.to_string())),
                }
            });
        })
    };

    let on_rename = {
        let error = error.clone();
        let refetch = props.refetch.clone();
        Callback::from(
            move |(category_id, name): (payloads::SpaceCategoryId, String)| {
                let error = error.clone();
                let refetch = refetch.clone();
                yew::platform::spawn_local(async move {
                    error.set(None);
                    let api_client = crate::get_api_client();
                    match api_client
                        .update_space_category(&requests::UpdateSpaceCategory {
                            category_id,
                            name,
                        })
                        .await
                    {
                        Ok(_) => refetch.emit(()),
                        Err(e) => error.set(Some(e.to_string())),
                    }
                });
            },
        )
    };

    let on_delete = {
        let error = error.clone();
        let refetch = props.refetch.clone();
        Callback::from(move |category_id: payloads::SpaceCategoryId| {
            let error = error.clone();
            let refetch = refetch.clone();
            yew::platform::spawn_local(async move {
                error.set(None);
                let api_client = crate::get_api_client();
                match api_client.delete_space_category(&category_id).await {
                    Ok(_) => refetch.emit(()),
                    Err(e) => error.set(Some(e.to_string())),
                }
            });
        })
    };

    let on_new_name_input = {
        let new_name = new_name.clone();
        Callback::from(move |e: InputEvent| {
            let input =
                e.target().unwrap().dyn_into::<HtmlInputElement>().unwrap();
            new_name.set(input.value());
        })
    };

    html! {
        <div class="bg-white dark:bg-neutral-800 p-4 rounded-lg shadow-md
                    border border-neutral-200 dark:border-neutral-700 mb-6">
            <h3 class="text-base font-semibold text-neutral-900
                       dark:text-neutral-100 mb-1">
                {"Space Categories"}
            </h3>
            <p class="text-xs text-neutral-500 dark:text-neutral-400 mb-3">
                {"Categories label spaces for per-bidder caps in capped \
                  auctions. They are shared across the community's sites. A \
                  category can only be deleted while no space or cap \
                  references it."}
            </p>

            {if let Some(err) = &*error {
                html! {
                    <div class="mb-3 p-3 rounded-md bg-red-50
                                dark:bg-red-900/20 border border-red-200
                                dark:border-red-800">
                        <p class="text-sm text-red-700 dark:text-red-400">
                            {err}
                        </p>
                    </div>
                }
            } else {
                html! {}
            }}

            {if props.categories.is_empty() {
                html! {
                    <p class="text-sm text-neutral-500 dark:text-neutral-400
                              mb-3">
                        {"No categories yet."}
                    </p>
                }
            } else {
                html! {
                    <ul class="mb-3 divide-y divide-neutral-200
                               dark:divide-neutral-700">
                        {for props.categories.iter().map(|category| {
                            let category_id = category.id;
                            let on_rename = on_rename.clone();
                            let on_delete = on_delete.clone();
                            html! {
                                <li
                                    key={category_id.to_string()}
                                    class="flex items-center
                                           justify-between gap-3 py-2"
                                >
                                    <div class="flex-1 text-sm
                                                text-neutral-900
                                                dark:text-neutral-100">
                                        <InlineEdit
                                            value={
                                                category.name.clone()
                                            }
                                            on_change={Callback::from(
                                                move |name: String| {
                                                    on_rename.emit(
                                                        (category_id, name),
                                                    );
                                                },
                                            )}
                                        />
                                    </div>
                                    <button
                                        type="button"
                                        onclick={Callback::from(move |_| {
                                            on_delete.emit(category_id)
                                        })}
                                        class="py-1.5 px-3 text-xs
                                               font-medium rounded-md border
                                               border-red-300
                                               dark:border-red-600
                                               text-red-700
                                               dark:text-red-300
                                               bg-red-50 dark:bg-red-900/20
                                               hover:bg-red-100
                                               dark:hover:bg-red-900/30
                                               transition-colors duration-200"
                                    >
                                        {"Delete"}
                                    </button>
                                </li>
                            }
                        })}
                    </ul>
                }
            }}

            <form onsubmit={on_add} class="flex gap-2">
                <input
                    type="text"
                    value={(*new_name).clone()}
                    oninput={on_new_name_input}
                    placeholder="New category name"
                    maxlength="255"
                    class="flex-1 px-3 py-2 border border-neutral-300
                           dark:border-neutral-600 rounded-md shadow-sm
                           bg-white dark:bg-neutral-700 text-neutral-900
                           dark:text-neutral-100 text-sm focus:outline-none
                           focus:ring-2 focus:ring-neutral-500
                           focus:border-neutral-500"
                />
                <button
                    type="submit"
                    disabled={new_name.trim().is_empty()}
                    class="py-2 px-4 rounded-md text-sm font-medium text-white
                           bg-neutral-900 hover:bg-neutral-800
                           dark:bg-neutral-100 dark:text-neutral-900
                           dark:hover:bg-neutral-200
                           disabled:opacity-50 disabled:cursor-not-allowed
                           transition-colors duration-200"
                >
                    {"Add"}
                </button>
            </form>
        </div>
    }
}
