use payloads::{SiteId, Space};
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub site_id: SiteId,
    pub on_close: Callback<()>,
    pub on_space_created: Callback<()>,
}

#[function_component]
pub fn CreateSpaceModal(props: &Props) -> Html {
    let name_ref = use_node_ref();
    let description_ref = use_node_ref();
    let eligibility_ref = use_node_ref();
    let available_ref = use_node_ref();

    let is_loading = use_state(|| false);
    let error = use_state(|| None::<String>);
    let success = use_state(|| false);
    let created_space_name = use_state(|| None::<String>);

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
                on_close.emit(());
            }
        })
    };

    let on_reset_form = {
        let success = success.clone();
        let error = error.clone();
        let created_space_name = created_space_name.clone();
        Callback::from(move |_| {
            success.set(false);
            error.set(None);
            created_space_name.set(None);
        })
    };

    let on_submit = {
        let name_ref = name_ref.clone();
        let description_ref = description_ref.clone();
        let eligibility_ref = eligibility_ref.clone();
        let available_ref = available_ref.clone();
        let is_loading = is_loading.clone();
        let error = error.clone();
        let success = success.clone();
        let created_space_name = created_space_name.clone();
        let on_space_created = props.on_space_created.clone();
        let site_id = props.site_id;

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let name_input = name_ref.cast::<HtmlInputElement>().unwrap();
            let name = name_input.value().trim().to_string();

            if name.is_empty() {
                error.set(Some("Name is required".to_string()));
                return;
            }

            let space_name = name.clone();

            let description_input =
                description_ref.cast::<HtmlInputElement>().unwrap();
            let description_value = description_input.value();
            let description = description_value.trim();
            let description = if description.is_empty() {
                None
            } else {
                Some(description.to_string())
            };

            let eligibility_input =
                eligibility_ref.cast::<HtmlInputElement>().unwrap();
            let eligibility_points =
                match eligibility_input.value().parse::<f64>() {
                    Ok(v) if v > 0.0 => v,
                    _ => {
                        error.set(Some(
                            "Eligibility points must be a positive number"
                                .to_string(),
                        ));
                        return;
                    }
                };

            let available_input =
                available_ref.cast::<HtmlInputElement>().unwrap();
            let is_available = available_input.checked();

            let space = Space {
                site_id,
                name,
                description,
                eligibility_points,
                is_available,
                site_image_id: None,
            };

            let is_loading = is_loading.clone();
            let error = error.clone();
            let success = success.clone();
            let created_space_name = created_space_name.clone();
            let on_space_created = on_space_created.clone();
            let name_ref = name_ref.clone();
            let description_ref = description_ref.clone();
            let eligibility_ref = eligibility_ref.clone();
            let available_ref = available_ref.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = crate::get_api_client();
                match api_client.create_space(&space).await {
                    Ok(_) => {
                        success.set(true);
                        created_space_name.set(Some(space_name));
                        on_space_created.emit(());
                        // Clear form inputs
                        if let Some(name_elem) =
                            name_ref.cast::<HtmlInputElement>()
                        {
                            name_elem.set_value("");
                        }
                        if let Some(desc_elem) =
                            description_ref.cast::<HtmlInputElement>()
                        {
                            desc_elem.set_value("");
                        }
                        if let Some(elig_elem) =
                            eligibility_ref.cast::<HtmlInputElement>()
                        {
                            elig_elem.set_value("1.0");
                        }
                        if let Some(avail_elem) =
                            available_ref.cast::<HtmlInputElement>()
                        {
                            avail_elem.set_checked(true);
                        }
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
                        {"Create New Space"}
                    </h3>
                    <button
                        onclick={Callback::from({
                            let on_close = props.on_close.clone();
                            move |_| on_close.emit(())
                        })}
                        class="text-neutral-500 hover:text-neutral-700 dark:text-neutral-400 dark:hover:text-neutral-200 text-2xl leading-none p-1"
                        title="Close"
                    >
                        {"×"}
                    </button>
                </div>

                {if *success {
                    html! {
                        <div class="space-y-4">
                            <div class="p-4 rounded-md bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800">
                                <p class="text-sm text-green-700 dark:text-green-400 font-medium mb-2">
                                    {"Space created successfully!"}
                                </p>
                                {if let Some(ref space_name) = *created_space_name {
                                    html! {
                                        <p class="text-sm text-green-600 dark:text-green-300">
                                            {format!("\"{}\" has been added to this site.", space_name)}
                                        </p>
                                    }
                                } else {
                                    html! {}
                                }}
                            </div>

                            <div class="flex gap-3">
                                <button
                                    type="button"
                                    onclick={on_reset_form}
                                    class="flex-1 py-2 px-4 border border-transparent
                                           rounded-md shadow-sm text-sm font-medium text-white
                                           bg-neutral-900 hover:bg-neutral-800
                                           dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200
                                           focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                                           transition-colors duration-200"
                                >
                                    {"Create Another Space"}
                                </button>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }}

                {if !*success {
                    html! {
                        <>
                            {if let Some(err) = &*error {
                                html! {
                                    <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 mb-4">
                                        <p class="text-sm text-red-700 dark:text-red-400">{err}</p>
                                    </div>
                                }
                            } else {
                                html! {}
                            }}

                <form onsubmit={on_submit} class="space-y-4">
                    <div>
                        <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                            {"Name *"}
                        </label>
                        <input
                            ref={name_ref}
                            type="text"
                            disabled={*is_loading}
                            required={true}
                            class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                   rounded-md shadow-sm bg-white dark:bg-neutral-700
                                   text-neutral-900 dark:text-neutral-100
                                   focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                   disabled:opacity-50 disabled:cursor-not-allowed"
                            placeholder="Enter space name"
                        />
                    </div>

                    <div>
                        <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                            {"Description"}
                        </label>
                        <input
                            ref={description_ref}
                            type="text"
                            disabled={*is_loading}
                            class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                   rounded-md shadow-sm bg-white dark:bg-neutral-700
                                   text-neutral-900 dark:text-neutral-100
                                   focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                   disabled:opacity-50 disabled:cursor-not-allowed"
                            placeholder="Optional description"
                        />
                    </div>

                    <div>
                        <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                            {"Eligibility Points *"}
                        </label>
                        <input
                            ref={eligibility_ref}
                            type="number"
                            step="0.1"
                            min="0.1"
                            value="1.0"
                            disabled={*is_loading}
                            required={true}
                            class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                   rounded-md shadow-sm bg-white dark:bg-neutral-700
                                   text-neutral-900 dark:text-neutral-100
                                   focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                   disabled:opacity-50 disabled:cursor-not-allowed"
                        />
                    </div>

                    <div class="flex items-center">
                        <input
                            ref={available_ref}
                            type="checkbox"
                            checked={true}
                            disabled={*is_loading}
                            class="h-4 w-4 text-neutral-600 focus:ring-neutral-500 border-neutral-300 dark:border-neutral-600 rounded disabled:opacity-50"
                        />
                        <label class="ml-2 text-sm font-medium text-neutral-700 dark:text-neutral-300">
                            {"Available"}
                        </label>
                    </div>

                    <div class="flex gap-3 pt-4">
                        <button
                            type="button"
                            onclick={Callback::from({
                                let on_close = props.on_close.clone();
                                move |_| on_close.emit(())
                            })}
                            disabled={*is_loading}
                            class="flex-1 py-2 px-4 border border-neutral-300 dark:border-neutral-600
                                   rounded-md shadow-sm text-sm font-medium text-neutral-700 dark:text-neutral-300
                                   bg-white dark:bg-neutral-700 hover:bg-neutral-50 dark:hover:bg-neutral-600
                                   focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                                   disabled:opacity-50 disabled:cursor-not-allowed
                                   transition-colors duration-200"
                        >
                            {"Cancel"}
                        </button>
                        <button
                            type="submit"
                            disabled={*is_loading}
                            class="flex-1 py-2 px-4 border border-transparent
                                   rounded-md shadow-sm text-sm font-medium text-white
                                   bg-neutral-900 hover:bg-neutral-800
                                   dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200
                                   focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                                   disabled:opacity-50 disabled:cursor-not-allowed
                                   transition-colors duration-200"
                        >
                            {if *is_loading { "Creating..." } else { "Create Space" }}
                        </button>
                    </div>
                </form>
                        </>
                    }
                } else {
                    html! {}
                }}
            </div>
        </div>
    }
}
