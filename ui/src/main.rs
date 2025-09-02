use ui::{App, logs};

fn main() {
    logs::init_logging();
    yew::Renderer::<App>::new().render();
}
