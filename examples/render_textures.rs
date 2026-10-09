use heph_gl::{
    renderers::{
        Renderer,
        resources::{BufferUsage, GpuBuffer, ResourceBinding, ResourceBindingType, TextureFormat},
        settings::{GraphicsPipelineOptions, SamplerOptions},
    },
    shader::Shader,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

use crate::utils::{ExampleRenderer, SHADERS_DIR, get_best_device, run_example, set_best_msaa};

mod utils;

type Vertex = [f32; 4];

const IMAGE_PATH: &str = "PATH";
const VERTICES: [Vertex; 6] = [
    [-0.75, -0.75, 0.0, 0.0],
    [0.75, -0.75, 1.0, 0.0],
    [0.75, 0.75, 1.0, 1.0],
    [-0.75, -0.75, 0.0, 0.0],
    [0.75, 0.75, 1.0, 1.0],
    [-0.75, 0.75, 0.0, 1.0],
];

fn example(renderer: &mut ExampleRenderer, _: RawWindowHandle, _: RawDisplayHandle) {
    let device = get_best_device(renderer);
    renderer.set_device(Some(&device), &[]).unwrap();
    set_best_msaa(renderer);

    // These shaders do not use resources.
    let vert_shader =
        Shader::from_file(format!("{}/{}", SHADERS_DIR, "2D_image_vert.spv")).unwrap();
    let frag_shader =
        Shader::from_file(format!("{}/{}", SHADERS_DIR, "2D_image_frag.spv")).unwrap();
    let pipeline = renderer
        .create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions {
                depth_test: false,
                depth_write: false,
                ..Default::default()
            },
        )
        .unwrap();
    drop(vert_shader);
    drop(frag_shader);

    let mut vertex_buffer = renderer
        .create_buffer(std::mem::size_of_val(&VERTICES), BufferUsage::Vertex)
        .unwrap();
    renderer
        .write_buffer(&vertex_buffer, bytemuck::cast_slice(&VERTICES))
        .unwrap();

    let image = image::open(IMAGE_PATH).unwrap().to_rgba8();
    let width = image.width();
    let height = image.height();

    renderer.begin_frame().unwrap();
    renderer.clear(RGB::default()).unwrap();

    let mut texture = renderer
        .create_texture(width, height, TextureFormat::Rgba8Srgb, image.as_raw())
        .unwrap();
    let mut sampler = renderer.create_sampler(&SamplerOptions::default()).unwrap();

    let bindings = [
        ResourceBinding {
            binding: 0,
            resource: ResourceBindingType::Buffer {
                handle: vertex_buffer,
                usage: BufferUsage::Vertex,
                offset: 0,
                size: vertex_buffer.size(),
            },
        },
        ResourceBinding {
            binding: 0,
            resource: ResourceBindingType::Texture { handle: texture },
        },
        ResourceBinding {
            binding: 1,
            resource: ResourceBindingType::Sampler { handle: sampler },
        },
    ];
    renderer
        .record_graphics_command(&pipeline, &[&bindings], VERTICES.len() as u32, 1)
        .unwrap();

    renderer.end_frame().unwrap();

    // Cleanup.
    renderer.wait_idle().unwrap();
    renderer.destroy_sampler(&mut sampler).unwrap();
    renderer.destroy_texture(&mut texture).unwrap();
    renderer.destroy_buffer(&mut vertex_buffer).unwrap();
    renderer.destroy_graphics_pipeline(&pipeline).unwrap();

    std::thread::sleep(std::time::Duration::from_secs(5));
    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Render Textures";
    const INIT_RENDERER: bool = true;
    run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
