use yew::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum AnnotationLayout {
    /// Arrows pointing to captions beside the image (desktop)
    Arrows,
    /// Numbered markers with list below (mobile)
    NumberedList,
}

/// Represents an annotation with an arrow pointing to a location on the
/// screenshot and an associated caption.
#[derive(Clone, PartialEq)]
pub struct Annotation {
    /// X coordinate on the image where the arrow should point (in image
    /// pixels).
    pub point_x: f64,
    /// Y coordinate on the image where the arrow should point (in image
    /// pixels).
    pub point_y: f64,
    /// X coordinate for the caption (relative to image, can be negative for
    /// left side). Only used in Arrows layout.
    pub caption_x: f64,
    /// Y coordinate for the caption (relative to image). Only used in Arrows
    /// layout.
    pub caption_y: f64,
    /// Width of the caption box in pixels. Only used in Arrows layout.
    pub caption_width: f64,
    /// The caption title.
    pub caption_title: String,
    /// The caption description text.
    pub caption_text: String,
    /// Whether the arrow attach to the left side of the caption box. Only
    /// used in Arrows layout.
    pub arrow_from_left: bool,
}

#[derive(Properties, PartialEq)]
pub struct AnnotatedScreenshotProps {
    /// Path to the screenshot image.
    pub src: AttrValue,
    /// Alt text for the image.
    pub alt: AttrValue,
    /// Width of the actual screenshot image in pixels.
    pub image_width: f64,
    /// Height of the actual screenshot image in pixels.
    pub image_height: f64,
    /// Minimum padding around the image (in image pixel units). Used as
    /// fallback when no annotations, and as minimum padding when annotations
    /// are present.
    #[prop_or(20.0)]
    pub min_padding: f64,
    /// Layout mode for annotations.
    pub layout: AnnotationLayout,
    /// List of annotations to display.
    pub annotations: Vec<Annotation>,
    /// Additional CSS classes for the container.
    #[prop_or_default]
    pub class: Classes,
}

#[function_component]
pub fn AnnotatedScreenshot(props: &AnnotatedScreenshotProps) -> Html {
    // Calculate actual needed padding based on caption positions
    let use_side_captions = props.layout == AnnotationLayout::Arrows;

    let left_padding = if props.annotations.is_empty() || !use_side_captions {
        props.min_padding
    } else {
        // Find the leftmost caption edge and ensure at least min_padding
        -props
            .annotations
            .iter()
            .map(|a| a.caption_x)
            .fold(0.0_f64, f64::min)
            .min(-props.min_padding)
    };

    let right_padding = if props.annotations.is_empty() || !use_side_captions {
        props.min_padding
    } else {
        // Find the rightmost caption edge and ensure at least min_padding past
        // the right-most edge of the image
        props
            .annotations
            .iter()
            .map(|a| a.caption_x + a.caption_width - props.image_width)
            .fold(0.0_f64, f64::max)
            .max(props.min_padding)
    };

    let total_width = props.image_width + left_padding + right_padding;
    let total_height = props.image_height + 2.0 * props.min_padding;
    let image_x_offset = left_padding;
    let image_y_offset = props.min_padding;

    let viewbox = format!("0 0 {} {}", total_width, total_height);

    html! {
        <div class={classes!("relative", props.class.clone())}>
            // SVG with image and arrows
            <svg
                class="w-full h-auto"
                viewBox={viewbox}
                preserveAspectRatio="xMidYMid meet"
            >
                <defs>
                    // Arrow marker for light mode
                    <marker
                        id="arrowhead-light"
                        markerWidth="20"
                        markerHeight="20"
                        refX="16"
                        refY="10"
                        orient="auto"
                        markerUnits="userSpaceOnUse"
                    >
                        <polygon
                            points="0 0, 20 10, 0 20"
                            fill="#262626"
                        />
                    </marker>
                    // Arrow marker for dark mode
                    <marker
                        id="arrowhead-dark"
                        markerWidth="20"
                        markerHeight="20"
                        refX="16"
                        refY="10"
                        orient="auto"
                        markerUnits="userSpaceOnUse"
                    >
                        <polygon
                            points="0 0, 20 10, 0 20"
                            fill="#e5e5e5"
                        />
                    </marker>
                </defs>

                // Screenshot border and frame
                // Outer shadow/depth
                <rect
                    x={(image_x_offset - 4.0).to_string()}
                    y={(image_y_offset - 4.0).to_string()}
                    width={(props.image_width + 8.0).to_string()}
                    height={(props.image_height + 8.0).to_string()}
                    fill="none"
                    stroke="#a3a3a3"
                    stroke-width="8"
                    rx="12"
                    opacity="0.3"
                />

                // Main border frame (light mode)
                <rect
                    x={image_x_offset.to_string()}
                    y={image_y_offset.to_string()}
                    width={props.image_width.to_string()}
                    height={props.image_height.to_string()}
                    fill="none"
                    stroke="#d4d4d4"
                    stroke-width="3"
                    rx="8"
                    class="dark:hidden"
                />

                // Main border frame (dark mode)
                <rect
                    x={image_x_offset.to_string()}
                    y={image_y_offset.to_string()}
                    width={props.image_width.to_string()}
                    height={props.image_height.to_string()}
                    fill="none"
                    stroke="#525252"
                    stroke-width="3"
                    rx="8"
                    class="hidden dark:block"
                />

                // The actual screenshot image
                <image
                    href={props.src.clone()}
                    x={image_x_offset.to_string()}
                    y={image_y_offset.to_string()}
                    width={props.image_width.to_string()}
                    height={props.image_height.to_string()}
                    clip-path="inset(0 round 8px)"
                />

                // Arrows and numbered markers
                {
                    props.annotations.iter().enumerate().map(|(index, annotation)| {
                        let arrow_x = image_x_offset + annotation.point_x;
                        let arrow_y = image_y_offset + annotation.point_y;

                        let caption_x = image_x_offset + annotation.caption_x;
                        let caption_y = image_y_offset + annotation.caption_y;

                        // Calculate arrow end point based on caption position
                        let arrow_end_x = if annotation.arrow_from_left {
                            caption_x
                        } else {
                            caption_x + annotation.caption_width
                        };
                        let arrow_end_y = caption_y + 40.0; // Offset from top

                        let number = (index + 1).to_string();

                        let marker_content = if use_side_captions {
                            // Arrow lines
                            html! {
                                <>
                                    <line
                                        x1={arrow_end_x.to_string()}
                                        y1={arrow_end_y.to_string()}
                                        x2={arrow_x.to_string()}
                                        y2={arrow_y.to_string()}
                                        stroke="#262626"
                                        stroke-width="4"
                                        marker-end="url(#arrowhead-light)"
                                        class="dark:hidden"
                                    />
                                    <line
                                        x1={arrow_end_x.to_string()}
                                        y1={arrow_end_y.to_string()}
                                        x2={arrow_x.to_string()}
                                        y2={arrow_y.to_string()}
                                        stroke="#e5e5e5"
                                        stroke-width="4"
                                        marker-end="url(#arrowhead-dark)"
                                        class="hidden dark:block"
                                    />
                                </>
                            }
                        } else {
                            // Numbered circle markers
                            html! {
                                <>
                                    <circle
                                        cx={arrow_x.to_string()}
                                        cy={arrow_y.to_string()}
                                        r="40"
                                        fill="white"
                                        stroke="#262626"
                                        stroke-width="4"
                                        class="dark:hidden"
                                    />
                                    <circle
                                        cx={arrow_x.to_string()}
                                        cy={arrow_y.to_string()}
                                        r="40"
                                        fill="#262626"
                                        stroke="#e5e5e5"
                                        stroke-width="4"
                                        class="hidden dark:block"
                                    />
                                    <text
                                        x={arrow_x.to_string()}
                                        y={(arrow_y + 14.0).to_string()}
                                        text-anchor="middle"
                                        font-size="36"
                                        font-weight="600"
                                        fill="#262626"
                                        class="dark:hidden"
                                    >
                                        {&number}
                                    </text>
                                    <text
                                        x={arrow_x.to_string()}
                                        y={(arrow_y + 14.0).to_string()}
                                        text-anchor="middle"
                                        font-size="36"
                                        font-weight="600"
                                        fill="#e5e5e5"
                                        class="hidden dark:block"
                                    >
                                        {&number}
                                    </text>
                                </>
                            }
                        };

                        html! {
                            <g>
                                {marker_content}
                            </g>
                        }
                    }).collect::<Html>()
                }
            </svg>

            // Caption boxes (arrows layout) or numbered list (numbered layout)
            {
                if use_side_captions {
                    // HTML caption boxes positioned absolutely
                    html! {
                        <div>
                            {
                                props.annotations.iter().map(|annotation| {
                                    // Convert from image coordinates to percentages
                                    let left_percent = ((image_x_offset + annotation.caption_x) / total_width) * 100.0;
                                    let top_percent = ((image_y_offset + annotation.caption_y) / total_height) * 100.0;
                                    let width_percent = (annotation.caption_width / total_width) * 100.0;

                                    let style = format!(
                                        "left: {}%; top: {}%; width: {}%;",
                                        left_percent, top_percent, width_percent
                                    );

                                    html! {
                                        <div
                                            class="absolute bg-white dark:bg-neutral-800 p-3 \
                                                rounded-lg shadow-lg border border-neutral-300 \
                                                dark:border-neutral-600"
                                            style={style}
                                        >
                                            <div class="font-semibold text-neutral-900 \
                                                dark:text-neutral-100 mb-1">
                                                {&annotation.caption_title}
                                            </div>
                                            <div class="text-sm text-neutral-600 \
                                                dark:text-neutral-400">
                                                {&annotation.caption_text}
                                            </div>
                                        </div>
                                    }
                                }).collect::<Html>()
                            }
                        </div>
                    }
                } else {
                    // Numbered list below the image
                    html! {
                        <div class="mt-6 space-y-4 px-2">
                            {
                                props.annotations.iter().enumerate().map(|(index, annotation)| {
                                    html! {
                                        <div class="flex gap-3">
                                            <div class="flex-shrink-0 w-8 h-8 rounded-full \
                                                bg-white dark:bg-neutral-800 border-2 \
                                                border-neutral-900 dark:border-neutral-100 \
                                                flex items-center justify-center font-semibold \
                                                text-neutral-900 dark:text-neutral-100">
                                                {index + 1}
                                            </div>
                                            <div class="flex-1">
                                                <div class="font-semibold text-neutral-900 \
                                                    dark:text-neutral-100 mb-1">
                                                    {&annotation.caption_title}
                                                </div>
                                                <div class="text-sm text-neutral-600 \
                                                    dark:text-neutral-400">
                                                    {&annotation.caption_text}
                                                </div>
                                            </div>
                                        </div>
                                    }
                                }).collect::<Html>()
                            }
                        </div>
                    }
                }
            }
        </div>
    }
}
