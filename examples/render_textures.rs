use heph_gl::{
    renderers::{
        Renderer,
        resources::{BufferUsage, GpuBuffer, ResourceBinding, ResourceBindingType, TextureFormat},
        settings::{ColorBlending, GraphicsPipelineOptions, SamplerOptions},
    },
    shader::Shader,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

use crate::utils::{ExampleRenderer, SHADERS_DIR, get_best_device, run_example, set_best_msaa};

mod utils;

type Vertex = [f32; 8];

const IMAGE_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_diff_1k.jpg";
const LATITUDE_SEGMENTS: usize = 32;
const LONGITUDE_SEGMENTS: usize = 64;

const TEXT: &str = "Rendering Textures!";
const FONT_PATH: &str = "assets/liberation_sans/LiberationSans-Regular.ttf";
const FONT_SIZE: f32 = 64.0;

fn sphere_position(radius: f32, u: f32, v: f32) -> [f32; 3] {
    let theta = v * std::f32::consts::PI;
    let phi = u * std::f32::consts::TAU;
    let sin_theta = theta.sin();
    [
        radius * sin_theta * phi.cos(),
        radius * theta.cos(),
        radius * sin_theta * phi.sin(),
    ]
}

fn sphere_vertices(radius: f32) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(LATITUDE_SEGMENTS * LONGITUDE_SEGMENTS * 6);

    for y in 0..LATITUDE_SEGMENTS {
        let v0 = y as f32 / LATITUDE_SEGMENTS as f32;
        let v1 = (y + 1) as f32 / LATITUDE_SEGMENTS as f32;

        for x in 0..LONGITUDE_SEGMENTS {
            let u0 = x as f32 / LONGITUDE_SEGMENTS as f32;
            let u1 = (x + 1) as f32 / LONGITUDE_SEGMENTS as f32;

            let p00 = sphere_position(radius, u0, v0);
            let p10 = sphere_position(radius, u1, v0);
            let p01 = sphere_position(radius, u0, v1);
            let p11 = sphere_position(radius, u1, v1);

            let n00 = [p00[0] / radius, p00[1] / radius, p00[2] / radius];
            let n10 = [p10[0] / radius, p10[1] / radius, p10[2] / radius];
            let n01 = [p01[0] / radius, p01[1] / radius, p01[2] / radius];
            let n11 = [p11[0] / radius, p11[1] / radius, p11[2] / radius];

            vertices.extend_from_slice(&[
                [p00[0], p00[1], p00[2], n00[0], n00[1], n00[2], u0, v0],
                [p10[0], p10[1], p10[2], n10[0], n10[1], n10[2], u1, v0],
                [p11[0], p11[1], p11[2], n11[0], n11[1], n11[2], u1, v1],
                [p00[0], p00[1], p00[2], n00[0], n00[1], n00[2], u0, v0],
                [p11[0], p11[1], p11[2], n11[0], n11[1], n11[2], u1, v1],
                [p01[0], p01[1], p01[2], n01[0], n01[1], n01[2], u0, v1],
            ]);
        }
    }

    vertices
}

fn render_sphere(
    renderer: &mut ExampleRenderer,
    sampler: <ExampleRenderer as Renderer>::Sampler,
) -> (
    <ExampleRenderer as Renderer>::GraphicsPipeline,
    <ExampleRenderer as Renderer>::Texture,
    <ExampleRenderer as Renderer>::Buffer,
) {
    let vert_shader =
        Shader::from_file(format!("{}/{}", SHADERS_DIR, "broken_brick_wall_vert.spv")).unwrap();
    let frag_shader =
        Shader::from_file(format!("{}/{}", SHADERS_DIR, "broken_brick_wall_frag.spv")).unwrap();
    let pipeline = renderer
        .create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions::default(),
        )
        .unwrap();

    let image = image::open(IMAGE_PATH).unwrap().to_rgba8();
    let width = image.width();
    let height = image.height();

    let vertices = sphere_vertices(0.5);
    let vertex_buffer = renderer
        .create_buffer(
            vertices.len() * std::mem::size_of::<Vertex>(),
            BufferUsage::Vertex,
        )
        .unwrap();
    renderer
        .write_buffer(&vertex_buffer, bytemuck::cast_slice(&vertices))
        .unwrap();

    // Creating a texture creates a GPU command to transfer the raw pixel data from CPU to GPU, thus
    // we must begin a frame first. Once a texture is loaded, we can use it until we destroy it.
    let texture = renderer
        .create_texture(width, height, TextureFormat::Rgba8Srgb, image.as_raw())
        .unwrap();

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
        .record_graphics_command(&pipeline, &[&bindings], vertices.len() as u32, 1)
        .unwrap();

    (pipeline, texture, vertex_buffer)
}

fn render_text(
    renderer: &mut ExampleRenderer,
    sampler: <ExampleRenderer as Renderer>::Sampler,
) -> (
    <ExampleRenderer as Renderer>::GraphicsPipeline,
    <ExampleRenderer as Renderer>::Texture,
    <ExampleRenderer as Renderer>::Buffer,
) {
    let vert_shader = Shader::from_file(format!("{}/{}", SHADERS_DIR, "text_vert.spv")).unwrap();
    let frag_shader = Shader::from_file(format!("{}/{}", SHADERS_DIR, "text_frag.spv")).unwrap();
    let pipeline = renderer
        .create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions {
                blending: ColorBlending::Alpha,
                depth_test: false,
                depth_write: false,
                ..Default::default()
            },
        )
        .unwrap();

    let font = heph_gl::text::Font::from_file(FONT_PATH, FONT_SIZE).unwrap();
    let vertices = font.vertices(TEXT, [330.0, 50.0], [1280.0, 720.0], [1.0, 1.0, 1.0, 1.0]);
    let vertex_buffer = renderer
        .create_buffer(
            vertices.len() * std::mem::size_of::<Vertex>(),
            BufferUsage::Vertex,
        )
        .unwrap();
    renderer
        .write_buffer(&vertex_buffer, bytemuck::cast_slice(&vertices))
        .unwrap();

    // Creating a texture creates a GPU command to transfer the raw pixel data from CPU to GPU, thus
    // we must begin a frame first. Once a texture is loaded, we can use it until we destroy it.
    let texture = renderer
        .create_texture(
            font.atlas_width(),
            font.atlas_height(),
            TextureFormat::R8Unorm,
            font.atlas_data(),
        )
        .unwrap();

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
        .record_graphics_command(&pipeline, &[&bindings], vertices.len() as u32, 1)
        .unwrap();

    (pipeline, texture, vertex_buffer)
}

fn example(renderer: &mut ExampleRenderer, _: RawWindowHandle, _: RawDisplayHandle) {
    let device = get_best_device(renderer);
    renderer.set_device(Some(&device), &[]).unwrap();
    set_best_msaa(renderer);

    let mut sampler = renderer.create_sampler(&SamplerOptions::default()).unwrap();

    renderer.begin_frame().unwrap();
    renderer.clear(RGB::default()).unwrap();
    let (
        broken_brick_wall_pipeline,
        mut broken_brick_wall_texture,
        mut broken_brick_wall_vertex_buffer,
    ) = render_sphere(renderer, sampler);
    let (text_pipeline, mut text_texture, mut text_vertex_buffer) = render_text(renderer, sampler);
    renderer.end_frame().unwrap();

    // Cleanup.
    renderer.wait_idle().unwrap();
    renderer.destroy_sampler(&mut sampler).unwrap();
    renderer
        .destroy_texture(&mut broken_brick_wall_texture)
        .unwrap();
    renderer
        .destroy_buffer(&mut broken_brick_wall_vertex_buffer)
        .unwrap();
    renderer
        .destroy_graphics_pipeline(&broken_brick_wall_pipeline)
        .unwrap();
    renderer.destroy_texture(&mut text_texture).unwrap();
    renderer.destroy_buffer(&mut text_vertex_buffer).unwrap();
    renderer.destroy_graphics_pipeline(&text_pipeline).unwrap();

    std::thread::sleep(std::time::Duration::from_secs(5));
    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Render Textures";
    const INIT_RENDERER: bool = true;
    run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
