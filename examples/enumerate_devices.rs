use heph_gl::renderers::Renderer;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

mod utils;

fn example(
    renderer: &mut utils::renderer::ExampleRenderer,
    _: RawWindowHandle,
    _: RawDisplayHandle,
) {
    let devices = renderer.enumerate_devices().unwrap();
    for device in &devices {
        println!("{}", device);
    }

    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Enumerate Devices";
    const INIT_RENDERER: bool = true;
    utils::run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
