use heph_gl::{renderers::Renderer, shader::Shader};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::utils::{ExampleRenderer, SHADERS_DIR, get_best_device, run_example};

mod utils;

fn example(renderer: &mut ExampleRenderer, _: RawWindowHandle, _: RawDisplayHandle) {
    let device = get_best_device(renderer);
    renderer.set_device(Some(&device), &[]).unwrap();

    // These shaders do not use resources.
    let mut pipelines = Vec::with_capacity(2);

    {
        const VERTEX_COUNT: u32 = 3;
        let vert_shader =
            Shader::from_file(format!("{}/{}", SHADERS_DIR, "basic_triangle_vert.spv")).unwrap();
        let frag_shader =
            Shader::from_file(format!("{}/{}", SHADERS_DIR, "basic_triangle_frag.spv")).unwrap();
        let pipeline = renderer
            .create_graphics_pipeline(&[&vert_shader, &frag_shader])
            .unwrap();
        pipelines.push((pipeline, VERTEX_COUNT));
    }
    {
        const VERTEX_COUNT: u32 = 36;
        let vert_shader =
            Shader::from_file(format!("{}/{}", SHADERS_DIR, "basic_cube_vert.spv")).unwrap();
        let frag_shader =
            Shader::from_file(format!("{}/{}", SHADERS_DIR, "basic_cube_frag.spv")).unwrap();
        let pipeline = renderer
            .create_graphics_pipeline(&[&vert_shader, &frag_shader])
            .unwrap();
        pipelines.push((pipeline, VERTEX_COUNT));
    }

    renderer.begin_frame().unwrap();

    const INSTANCE_COUNT: u32 = 1;
    for (ref pipeline, vertex_count) in pipelines {
        renderer
            .record_graphics_command(pipeline, &[], vertex_count, INSTANCE_COUNT)
            .unwrap();
    }

    renderer.end_frame().unwrap();

    std::thread::sleep(std::time::Duration::from_secs(5));
    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Render Basic Triangle";
    const INIT_RENDERER: bool = true;
    run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
