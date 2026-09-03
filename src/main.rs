mod app;
mod hash;
mod layout;
mod model;

fn main() {
    console_error_panic_hook::set_once();
    yew::Renderer::<app::App>::new().render();
}
