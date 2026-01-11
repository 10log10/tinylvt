use payloads::responses::UserProfile;
use yew::prelude::*;

use crate::hooks::{login_form, use_require_auth};

/// Component that only renders its children when the user is authenticated.
/// Shows a login form if not authenticated.
///
/// This component ensures that child components and their hooks are only
/// called when the user is logged in, avoiding conditional hook violations
/// and unnecessary data fetching.
///
/// Supports two modes:
/// 1. Simple children mode: Just wraps content that doesn't need the profile
/// 2. Render prop mode: Provides UserProfile to children that need it
#[derive(Properties, PartialEq)]
pub struct RequireAuthProps {
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub render: Option<Callback<UserProfile, Html>>,
}

#[function_component]
pub fn RequireAuth(props: &RequireAuthProps) -> Html {
    let user_profile = use_require_auth();

    if user_profile.is_none() {
        return login_form();
    }

    // If render prop is provided, use it and pass the profile
    if let Some(render) = &props.render {
        return render.emit(user_profile.unwrap());
    }

    // Otherwise, just render children
    html! {
        <>
            {for props.children.iter()}
        </>
    }
}
