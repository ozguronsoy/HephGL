use heph_gl::{
    renderers::{Renderer, settings::GraphicsPipelineOptions},
    shader::Shader,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

use crate::utils::{ExampleRenderer, SHADERS_DIR, get_best_device, run_example};

mod utils;

fn example(renderer: &mut ExampleRenderer, _: RawWindowHandle, _: RawDisplayHandle) {
    let device = get_best_device(renderer);
    renderer.set_device(Some(&device), &[]).unwrap();

    // These shaders do not use resources.
    let vert_shader = Shader::from_file(format!(
        "{}/shapes/{}",
        SHADERS_DIR, "basic_triangle_vert.spv"
    ))
    .unwrap();
    let frag_shader = Shader::from_file(format!(
        "{}/shapes/{}",
        SHADERS_DIR, "basic_triangle_frag.spv"
    ))
    .unwrap();
    let pipeline = renderer
        .create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions::default(),
        )
        .unwrap();
    drop(vert_shader);
    drop(frag_shader);

    renderer.begin_frame().unwrap();
    renderer.clear(RGB::default()).unwrap();

    const VERTEX_COUNT: u32 = 3;
    const INSTANCE_COUNT: u32 = 1;
    renderer
        .record_graphics_command(&pipeline, &[], VERTEX_COUNT, INSTANCE_COUNT)
        .unwrap();

    renderer.end_frame().unwrap();

    // Cleanup.
    renderer.wait_idle().unwrap();
    renderer.destroy_graphics_pipeline(&pipeline).unwrap();

    std::thread::sleep(std::time::Duration::from_secs(5));
    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Render Basic Triangle";
    const INIT_RENDERER: bool = true;
    run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
