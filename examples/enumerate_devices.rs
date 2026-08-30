use heph_gl::renderers::Renderer;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::utils::{ExampleRenderer, run_example};

mod utils;

fn example(renderer: &mut ExampleRenderer, _: RawWindowHandle, _: RawDisplayHandle) {
    let devices = renderer.enumerate_devices().unwrap();
    for device in &devices {
        println!("{}", device);
    }

    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Enumerate Devices";
    const INIT_RENDERER: bool = true;
    run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
