use heph_gl::renderers::{FeatureRequest, InitializeOptions, Renderer};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::utils::{ExampleRenderer, run_example};

mod utils;

fn example(
    renderer: &mut ExampleRenderer,
    window_handle: RawWindowHandle,
    display_handle: RawDisplayHandle,
) {
    // The renderer is injected in examples so we don't have to create an instance.
    // Normally we would have to create an instance via `ExampleRenderer::new();`.

    // After creating a renderer, we must initialize it and set a target graphics
    // device before we can start using it.

    // Initialize options can only be set during initialization. If you want to
    // change these options, you must call the `uninitialize` method first.
    let init_options = InitializeOptions {
        app_name: "Name of your app",
        // HephGL uses raw handles, so it is not bound to a single window creation library.
        // In examples, HephGL injects these handles, but you can obtain them via the windowing
        // library of your choice (e.g., Winit, SDL).
        window_handle,
        display_handle,
    };
    renderer.initialize(&init_options).unwrap();

    let devices = renderer.enumerate_devices().unwrap();

    // Initializes the graphics device with the requested features. You cannot
    // request or remove features after the device is set, you must call
    // `set_device` again with the new feature list. This will fail if a
    // required feature is not supported.
    let requested_features = [FeatureRequest {
        feature: heph_gl::graphics_device::Feature::ComputeShaders,
        required: false,
    }];
    renderer
        .set_device(devices.first().as_ref().unwrap(), &requested_features)
        .unwrap();

    // Renderer is now ready to use!
}

fn main() {
    const EXAMPLE_NAME: &str = "Creating Renderer";
    const INIT_RENDERER: bool = false;
    run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
