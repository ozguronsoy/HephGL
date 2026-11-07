use std::time::{Duration, Instant};

use heph_gl::{
    Shader,
    renderers::{
        BufferUsage, GraphicsPipelineOptions, Renderer, RendererGraphicsPipeline, ResourceBinding,
        ResourceBindingType, SamplerOptions,
    },
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

mod utils;

// [position.x, position.y, tex_coord.u, tex_coord.v]
type Vertex = [f32; 4];

/// Creates the two triangles of a quad that shows the whole canvas, mirroring it if requested.
fn quad_vertices(min: [f32; 2], max: [f32; 2], mirror_x: bool, mirror_y: bool) -> [Vertex; 6] {
    let (left_u, right_u) = if mirror_x { (1.0, 0.0) } else { (0.0, 1.0) };
    let (top_v, bottom_v) = if mirror_y { (1.0, 0.0) } else { (0.0, 1.0) };
    let top_left = [min[0], min[1], left_u, top_v];
    let top_right = [max[0], min[1], right_u, top_v];
    let bottom_right = [max[0], max[1], right_u, bottom_v];
    let bottom_left = [min[0], max[1], left_u, bottom_v];
    [
        top_left,
        top_right,
        bottom_right,
        top_left,
        bottom_right,
        bottom_left,
    ]
}

fn create_pipeline(
    renderer: &mut utils::renderer::ExampleRenderer,
    vertex_shader: &str,
    fragment_shader: &str,
) -> Box<dyn RendererGraphicsPipeline> {
    let vertex_shader =
        Shader::from_file(format!("{}/{}", utils::SHADERS_DIR, vertex_shader)).unwrap();
    let fragment_shader =
        Shader::from_file(format!("{}/{}", utils::SHADERS_DIR, fragment_shader)).unwrap();
    renderer
        .create_graphics_pipeline(
            &[&vertex_shader, &fragment_shader],
            &GraphicsPipelineOptions::default(),
        )
        .unwrap()
}

fn example(
    renderer: &mut utils::renderer::ExampleRenderer,
    _: RawWindowHandle,
    _: RawDisplayHandle,
) {
    let device = utils::renderer::get_best_device(renderer);
    renderer.set_device(Some(&device), &[]).unwrap();
    utils::renderer::set_best_msaa(renderer);

    let mut triangle_pipeline = create_pipeline(
        renderer,
        "shapes/basic_triangle_vert.spv",
        "shapes/basic_triangle_frag.spv",
    );
    let mut canvas_pipeline = create_pipeline(renderer, "canvas_vert.spv", "canvas_frag.spv");

    let mut canvas = renderer.create_canvas(640, 360).unwrap();
    let mut sampler = renderer.create_sampler(&SamplerOptions::default()).unwrap();

    // Show the canvas four times, mirrored differently in each quad.
    let mut vertices = Vec::with_capacity(4 * 6);
    vertices.extend(quad_vertices([-0.95, -0.95], [-0.05, -0.05], false, false));
    vertices.extend(quad_vertices([0.05, -0.95], [0.95, -0.05], true, false));
    vertices.extend(quad_vertices([-0.95, 0.05], [-0.05, 0.95], false, true));
    vertices.extend(quad_vertices([0.05, 0.05], [0.95, 0.95], true, true));
    let mut vertex_buffer = renderer
        .create_buffer(
            std::mem::size_of_val(vertices.as_slice()),
            BufferUsage::Vertex,
        )
        .unwrap();
    renderer
        .write_buffer(vertex_buffer.as_mut(), 0, bytemuck::cast_slice(&vertices))
        .unwrap();

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        renderer.begin_frame().unwrap();
        renderer.clear(RGB::default()).unwrap();

        // Render into the canvas before sampling it.
        let time = start.elapsed().as_secs_f32();
        renderer
            .clear_canvas(
                canvas.as_mut(),
                RGB::new(0.2 + 0.15 * time.sin(), 0.1, 0.2 + 0.15 * time.cos()),
            )
            .unwrap();
        renderer
            .record_canvas_command(
                canvas.as_mut(),
                triangle_pipeline.as_mut(),
                None,
                &[],
                &[],
                3,
                1,
            )
            .unwrap();

        // Render the canvas into the display.
        let vertex_bindings = [ResourceBinding {
            binding: 0,
            resource: ResourceBindingType::Buffer {
                buffer: vertex_buffer.as_ref(),
                offset: 0,
                size: vertex_buffer.size(),
            },
        }];
        let resource_bindings = [
            ResourceBinding {
                binding: 0,
                resource: ResourceBindingType::Texture {
                    texture: canvas.texture(),
                },
            },
            ResourceBinding {
                binding: 1,
                resource: ResourceBindingType::Sampler {
                    sampler: sampler.as_ref(),
                },
            },
        ];
        renderer
            .record_graphics_command(
                canvas_pipeline.as_mut(),
                None,
                &vertex_bindings,
                &[&resource_bindings],
                vertices.len() as u32,
                1,
            )
            .unwrap();

        renderer.end_frame().unwrap();
    }

    // Cleanup.
    renderer.wait_idle().unwrap();
    renderer.destroy_buffer(vertex_buffer.as_mut()).unwrap();
    renderer.destroy_sampler(sampler.as_mut()).unwrap();
    renderer.destroy_canvas(canvas.as_mut()).unwrap();
    renderer
        .destroy_graphics_pipeline(canvas_pipeline.as_mut())
        .unwrap();
    renderer
        .destroy_graphics_pipeline(triangle_pipeline.as_mut())
        .unwrap();

    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Render Canvas";
    const INIT_RENDERER: bool = true;
    utils::run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
