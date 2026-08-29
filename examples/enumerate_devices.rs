use heph_gl::renderers::Renderer;

use crate::utils::{ExampleRenderer, run_example};

mod utils;

fn example(renderer: &mut ExampleRenderer) {
    let devices = renderer.enumerate_devices().unwrap();
    for device in &devices {
        println!("{}", device);
    }
}

fn main() {
    const EXAMPLE_NAME: &str = "Enumerate Devices";
    run_example(EXAMPLE_NAME, example);
}
