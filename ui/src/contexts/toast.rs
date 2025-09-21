use std::collections::HashMap;
use uuid::Uuid;
use yew::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum ToastType {
    Error,
    Success,
    #[allow(dead_code)]
    Info,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub id: Uuid,
    pub message: String,
    pub toast_type: ToastType,
    pub duration: Option<u32>, // milliseconds, None for no auto-dismiss
}

impl Toast {
    pub fn new(message: String, toast_type: ToastType) -> Self {
        Self {
            id: Uuid::new_v4(),
            message,
            toast_type,
            duration: Some(5000), // 5 seconds default
        }
    }

    pub fn error(message: String) -> Self {
        Self::new(message, ToastType::Error)
    }

    pub fn success(message: String) -> Self {
        Self::new(message, ToastType::Success)
    }

    #[allow(dead_code)]
    pub fn info(message: String) -> Self {
        Self::new(message, ToastType::Info)
    }

    #[allow(dead_code)]
    pub fn duration(mut self, duration_ms: u32) -> Self {
        self.duration = Some(duration_ms);
        self
    }

    #[allow(dead_code)]
    pub fn no_auto_dismiss(mut self) -> Self {
        self.duration = None;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ToastState {
    pub toasts: HashMap<Uuid, Toast>,
}

pub enum ToastAction {
    Add(Toast),
    Remove(Uuid),
    #[allow(dead_code)]
    Clear,
}

impl Reducible for ToastState {
    type Action = ToastAction;

    fn reduce(
        self: std::rc::Rc<Self>,
        action: Self::Action,
    ) -> std::rc::Rc<Self> {
        let mut toasts = self.toasts.clone();

        match action {
            ToastAction::Add(toast) => {
                toasts.insert(toast.id, toast);
            }
            ToastAction::Remove(id) => {
                toasts.remove(&id);
            }
            ToastAction::Clear => {
                toasts.clear();
            }
        }

        std::rc::Rc::new(ToastState { toasts })
    }
}

pub type ToastContext = UseReducerHandle<ToastState>;

#[derive(Properties, PartialEq)]
pub struct ToastProviderProps {
    pub children: Children,
}

#[function_component]
pub fn ToastProvider(props: &ToastProviderProps) -> Html {
    let toast_state = use_reducer(ToastState::default);

    html! {
        <ContextProvider<ToastContext> context={toast_state}>
            {props.children.clone()}
        </ContextProvider<ToastContext>>
    }
}

#[derive(Clone)]
pub struct ToastHandle {
    context: ToastContext,
}

impl ToastHandle {
    pub fn new(context: ToastContext) -> Self {
        Self { context }
    }

    pub fn add(&self, toast: Toast) {
        let toast_id = toast.id;
        let duration = toast.duration;
        let context = self.context.clone();

        // Add the toast
        self.context.dispatch(ToastAction::Add(toast));

        // Set up auto-dismiss if duration is specified
        if let Some(duration_ms) = duration {
            let context_for_timeout = context.clone();
            yew::platform::spawn_local(async move {
                gloo_timers::future::TimeoutFuture::new(duration_ms).await;
                context_for_timeout.dispatch(ToastAction::Remove(toast_id));
            });
        }
    }

    pub fn error(&self, message: impl Into<String>) {
        self.add(Toast::error(message.into()));
    }

    pub fn success(&self, message: impl Into<String>) {
        self.add(Toast::success(message.into()));
    }

    #[allow(dead_code)]
    pub fn info(&self, message: impl Into<String>) {
        self.add(Toast::info(message.into()));
    }

    pub fn remove(&self, id: Uuid) {
        self.context.dispatch(ToastAction::Remove(id));
    }

    #[allow(dead_code)]
    pub fn clear(&self) {
        self.context.dispatch(ToastAction::Clear);
    }
}

#[hook]
pub fn use_toast() -> ToastHandle {
    let context = use_context::<ToastContext>()
        .expect("use_toast must be used within a ToastProvider");
    ToastHandle::new(context)
}
