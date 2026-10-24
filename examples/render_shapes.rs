use heph_gl::{
    renderers::{
        Renderer,
        settings::{GraphicsPipelineOptions, PrimitiveTopology},
    },
    shader::Shader,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

mod utils;

fn example(
    renderer: &mut utils::renderer::ExampleRenderer,
    _: RawWindowHandle,
    _: RawDisplayHandle,
) {
    let device = utils::renderer::get_best_device(renderer);
    renderer.set_device(Some(&device), &[]).unwrap();
    utils::renderer::set_best_msaa(renderer);

    // These shaders do not have any input.
    let vertex_shader_infos = [
        ("dot", PrimitiveTopology::PointList, 1),
        ("line", PrimitiveTopology::LineList, 2),
        ("ellipse", PrimitiveTopology::LineStrip, 257),
        ("pentagon", PrimitiveTopology::TriangleStrip, 5),
    ];
    let mut pipelines = Vec::with_capacity(vertex_shader_infos.len());
    {
        let frag_shader = Shader::from_file(format!(
            "{}/shapes/{}",
            utils::SHADERS_DIR,
            "shape_frag.spv"
        ))
        .unwrap();
        for (vertex_shader_name, topology, draw_count) in vertex_shader_infos {
            let vertex_shader = Shader::from_file(format!(
                "{}/shapes/{}_vert.spv",
                utils::SHADERS_DIR,
                vertex_shader_name
            ))
            .unwrap();
            pipelines.push((
                renderer
                    .create_graphics_pipeline(
                        &[&vertex_shader, &frag_shader],
                        &GraphicsPipelineOptions {
                            topology,
                            ..Default::default()
                        },
                    )
                    .unwrap(),
                draw_count,
            ));
        }
    }

    renderer.begin_frame().unwrap();
    renderer.clear(RGB::default()).unwrap();

    for (pipeline, draw_count) in pipelines.iter_mut() {
        const INSTANCE_COUNT: u32 = 1;
        renderer
            .record_graphics_command(pipeline.as_mut(), &[], *draw_count, INSTANCE_COUNT)
            .unwrap();
    }

    renderer.end_frame().unwrap();

    // Cleanup.
    renderer.wait_idle().unwrap();
    for (ref mut pipeline, _) in pipelines {
        renderer
            .destroy_graphics_pipeline(pipeline.as_mut())
            .unwrap();
    }

    std::thread::sleep(std::time::Duration::from_secs(5));
    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Render Shapes";
    const INIT_RENDERER: bool = true;
    utils::run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
