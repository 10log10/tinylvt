use ui::{logs, App};

fn main() {
    logs::init_logging();
    yew::Renderer::<App>::new().render();
}
