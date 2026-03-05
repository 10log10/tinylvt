use yew::prelude::*;

use super::{AnnotatedScreenshot, Annotation, AnnotationLayout};

const IMAGE_WIDTH: f64 = 1600.0;
const IMAGE_HEIGHT: f64 = 3467.0;

const CAPTION_WIDTH: f64 = 500.0;
const CAPTION_EDGE_OFFSET: f64 = 70.0;

#[derive(Properties, PartialEq)]
pub struct AuctionInterfaceWalkthroughProps {
    /// Whether to use dark mode screenshot.
    pub dark_mode: bool,
    /// Additional CSS classes for the container.
    #[prop_or_default]
    pub class: Classes,
}

#[function_component]
pub fn AuctionInterfaceWalkthrough(
    props: &AuctionInterfaceWalkthroughProps,
) -> Html {
    let annotations = vec![
        Annotation {
            point_x: 30.0,
            point_y: 720.0,
            caption_x: -CAPTION_WIDTH - CAPTION_EDGE_OFFSET,
            caption_y: 300.0,
            caption_width: CAPTION_WIDTH,
            arrow_from_left: true,
            caption_title: "Possession period".to_string(),
            caption_text:
                "What time period of resource usage the auction is for."
                    .to_string(),
        },
        Annotation {
            point_x: 1430.0,
            point_y: 720.0,
            caption_x: IMAGE_WIDTH + 70.0,
            caption_y: 500.0,
            caption_width: CAPTION_WIDTH,
            arrow_from_left: false,
            caption_title: "Start time".to_string(),
            caption_text:
                "Auctions run automatically at the scheduled start time."
                    .to_string(),
        },
        Annotation {
            point_x: 30.0,
            point_y: 1150.0,
            caption_x: -CAPTION_WIDTH - CAPTION_EDGE_OFFSET,
            caption_y: 900.0,
            caption_width: CAPTION_WIDTH,
            arrow_from_left: true,
            caption_title: "Current round".to_string(),
            caption_text:
                "Each round people place bids, and one bidder for each space is randomly selected as the standing high bidder."
                    .to_string(),
        },
        Annotation {
            point_x: 1450.0,
            point_y: 1380.0,
            caption_x: IMAGE_WIDTH + 70.0,
            caption_y: 1200.0,
            caption_width: CAPTION_WIDTH,
            arrow_from_left: false,
            caption_title: "Eligibility".to_string(),
            caption_text:
                "Ensures bidders actively participate throughout the auction."
                    .to_string(),
        },
        Annotation {
            point_x: 30.0,
            point_y: 1930.0,
            caption_x: -CAPTION_WIDTH - CAPTION_EDGE_OFFSET,
            caption_y: 1700.0,
            caption_width: CAPTION_WIDTH,
            arrow_from_left: true,
            caption_title: "Proxy bidding".to_string(),
            caption_text:
                "The system bids for you automatically. Enable it before the auction starts."
                    .to_string(),
        },
        Annotation {
            point_x: 30.0,
            point_y: 2500.0,
            caption_x: -CAPTION_WIDTH - CAPTION_EDGE_OFFSET,
            caption_y: 2200.0,
            caption_width: CAPTION_WIDTH,
            arrow_from_left: true,
            caption_title: "Space list".to_string(),
            caption_text:
                "The spaces being auctioned."
                    .to_string(),
        },
        Annotation {
            point_x: 280.0,
            point_y: 2730.0,
            caption_x: -CAPTION_WIDTH - CAPTION_EDGE_OFFSET,
            caption_y: 2500.0,
            caption_width: CAPTION_WIDTH,
            arrow_from_left: true,
            caption_title: "Eligibility points".to_string(),
            caption_text:
                "The number of eligibility points a space has. New bids and standing high bids count towards round activity."
                    .to_string(),
        },
        Annotation {
            point_x: 510.0,
            point_y: 2770.0,
            caption_x: -CAPTION_WIDTH - CAPTION_EDGE_OFFSET,
            caption_y: 3050.0,
            caption_width: CAPTION_WIDTH,
            arrow_from_left: false,
            caption_title: "Current price".to_string(),
            caption_text:
                "The current price for a space based on the results of the last round. \"C\" is for \"credits\"."
                    .to_string(),
        },
        Annotation {
            point_x: 1000.0,
            point_y: 2700.0,
            caption_x: IMAGE_WIDTH + 70.0,
            caption_y: 1850.0,
            caption_width: CAPTION_WIDTH,
            arrow_from_left: false,
            caption_title: "Your value".to_string(),
            caption_text:
                "The maximum amount you'd pay for a space. Set these to complete proxy bidding setup."
                    .to_string(),
        },
        Annotation {
            point_x: 1200.0,
            point_y: 2750.0,
            caption_x: IMAGE_WIDTH + 70.0,
            caption_y: 2250.0,
            caption_width: CAPTION_WIDTH,
            arrow_from_left: false,
            caption_title: "Surplus".to_string(),
            caption_text:
                "The difference between your value and the current price. This is what the proxy bidding maximizes."
                    .to_string(),
        },
        Annotation {
            point_x: IMAGE_WIDTH - 30.0,
            point_y: 2960.0,
            caption_x: IMAGE_WIDTH + 70.0,
            caption_y: 2700.0,
            caption_width: CAPTION_WIDTH,
            arrow_from_left: false,
            caption_title: "Current bid".to_string(),
            caption_text:
                "The proxy bidding has already bid for the space with the maximum surplus."
                    .to_string(),
        },
        Annotation {
            point_x: IMAGE_WIDTH - 30.0,
            point_y: 3170.0,
            caption_x: IMAGE_WIDTH + 70.0,
            caption_y: 3100.0,
            caption_width: CAPTION_WIDTH,
            arrow_from_left: false,
            caption_title: "Eligibility limit".to_string(),
            caption_text:
                "Not enough eligibility to bid for more than 1 point of spaces."
                    .to_string(),
        },
    ];

    html! {
        <>
            // Desktop: Arrows layout
            <div class="hidden lg:block">
                <AnnotatedScreenshot
                    src={if props.dark_mode {
                        "/auction-detail-dark.jpg"
                    } else {
                        "/auction-detail-light.jpg"
                    }}
                    alt="TinyLVT auction interface walkthrough"
                    image_width={IMAGE_WIDTH}
                    image_height={IMAGE_HEIGHT}
                    min_padding={20.0}
                    layout={AnnotationLayout::Arrows}
                    annotations={annotations.clone()}
                    class={props.class.clone()}
                />
            </div>

            // Mobile: Numbered list layout
            <div class="lg:hidden">
                <AnnotatedScreenshot
                    src={if props.dark_mode {
                        "/auction-detail-dark.jpg"
                    } else {
                        "/auction-detail-light.jpg"
                    }}
                    alt="TinyLVT auction interface walkthrough"
                    image_width={IMAGE_WIDTH}
                    image_height={IMAGE_HEIGHT}
                    min_padding={20.0}
                    layout={AnnotationLayout::NumberedList}
                    annotations={annotations}
                    class={props.class.clone()}
                />
            </div>
        </>
    }
}
