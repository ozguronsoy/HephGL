use heph_gl::renderers::{
    DEFAULT_DEVICE, InitializeOptions, LATEST_API_VERSION, Renderer, SamplerOptions,
    debug::{Debugger, Severity},
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

mod utils;

fn example(
    renderer: &mut utils::renderer::ExampleRenderer,
    window_handle: RawWindowHandle,
    display_handle: RawDisplayHandle,
) {
    // To use a debugger, the renderer must be initialized with debugging enabled.
    let init_options = InitializeOptions {
        app_name: "Name of your app",
        window_handle,
        display_handle,
        api_version: LATEST_API_VERSION,
        debug: true,
    };
    renderer.initialize(&init_options).unwrap();
    renderer.set_device(DEFAULT_DEVICE, &[]).unwrap();

    if let Err((e, _)) = renderer.attach_debugger(Debugger::default()) {
        // On failure, the renderer returns the debugger along with the error, so its logs are not
        // lost.
        panic!("{}", e);
    }

    // Create an object and set its name.
    let sampler = renderer.create_sampler(&SamplerOptions::default()).unwrap();
    renderer
        .debugger()
        .unwrap()
        .set_object_name(sampler.as_ref(), "Example Sampler")
        .unwrap();

    // Group the commands of a frame under labels so they can be identified in graphics debuggers.
    // Labels can be nested, and every pushed label must be popped before the frame ends.
    renderer.begin_frame().unwrap();
    renderer.debugger().unwrap().push_label("Frame").unwrap();
    renderer.debugger().unwrap().push_label("Clear").unwrap();
    renderer.clear(RGB::new(0.1, 0.1, 0.1)).unwrap();
    renderer.debugger().unwrap().pop_label().unwrap();
    renderer.debugger().unwrap().pop_label().unwrap();
    renderer.end_frame().unwrap();

    // Leak the object so the debugger logs it.
    renderer.uninitialize().unwrap();

    // Detach the debugger once we are done with it. It keeps the recorded logs, and it can be
    // attached to a renderer initialized with debugging again later.
    let debugger = renderer.detach_debugger().unwrap();
    let logs = debugger.logs();
    println!("Number of logs: {}", logs.len());
    // Print the errors and warnings.
    for log in logs
        .iter()
        .filter(|l| matches!(l.severity(), Severity::Warning | Severity::Error))
    {
        println!("{}", log);
    }

    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Debugger";
    const INIT_RENDERER: bool = false;
    utils::run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
