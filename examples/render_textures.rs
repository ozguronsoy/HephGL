use heph_gl::{
    graphics_device::Feature::AnisotropicFiltering,
    renderers::{
        Renderer,
        resources::{BufferUsage, GpuBuffer, ResourceBinding, ResourceBindingType, TextureFormat},
        settings::{FeatureRequest, GraphicsPipelineOptions, SamplerOptions, TextureOptions},
    },
    shader::Shader,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

use crate::utils::{ExampleRenderer, SHADERS_DIR, get_best_device, run_example, set_best_msaa};

mod utils;

// [position.x, position.y, position.z, normal.x, normal.y, normal.z, tex_coord.u, tex_coord.v]
type Vertex = [f32; 8];

const IMAGE_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_diff_1k.jpg";
const LATITUDE_SEGMENTS: usize = 32;
const LONGITUDE_SEGMENTS: usize = 64;

// Calculates the position of a point on a sphere given its radius, x offset, and spherical
// coordinates (u, v).
fn sphere_position(radius: f32, x_offset: f32, u: f32, v: f32) -> [f32; 3] {
    let theta = v * std::f32::consts::PI;
    let phi = u * std::f32::consts::TAU;
    let sin_theta = theta.sin();
    [
        x_offset + radius * sin_theta * phi.cos(),
        radius * theta.cos(),
        radius * sin_theta * phi.sin(),
    ]
}

// Calculates the sphere vertices for a given radius and x offset. The sphere is generated using
// latitude and longitude segments, creating a grid of vertices that form the surface of the sphere.
// Each vertex includes position, normal, and texture coordinates.
fn sphere_vertices(radius: f32, x_offset: f32) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(LATITUDE_SEGMENTS * LONGITUDE_SEGMENTS * 6);

    for y in 0..LATITUDE_SEGMENTS {
        let v0 = y as f32 / LATITUDE_SEGMENTS as f32;
        let v1 = (y + 1) as f32 / LATITUDE_SEGMENTS as f32;

        for x in 0..LONGITUDE_SEGMENTS {
            let u0 = x as f32 / LONGITUDE_SEGMENTS as f32;
            let u1 = (x + 1) as f32 / LONGITUDE_SEGMENTS as f32;

            let p00 = sphere_position(radius, x_offset, u0, v0);
            let p10 = sphere_position(radius, x_offset, u1, v0);
            let p01 = sphere_position(radius, x_offset, u0, v1);
            let p11 = sphere_position(radius, x_offset, u1, v1);

            let n00 = [
                (p00[0] - x_offset) / radius,
                p00[1] / radius,
                p00[2] / radius,
            ];
            let n10 = [
                (p10[0] - x_offset) / radius,
                p10[1] / radius,
                p10[2] / radius,
            ];
            let n01 = [
                (p01[0] - x_offset) / radius,
                p01[1] / radius,
                p01[2] / radius,
            ];
            let n11 = [
                (p11[0] - x_offset) / radius,
                p11[1] / radius,
                p11[2] / radius,
            ];

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

fn example(renderer: &mut ExampleRenderer, _: RawWindowHandle, _: RawDisplayHandle) {
    let device = get_best_device(renderer);
    renderer
        .set_device(
            Some(&device),
            &[FeatureRequest::new(AnisotropicFiltering, false)],
        )
        .unwrap();
    set_best_msaa(renderer);

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
    drop(vert_shader);
    drop(frag_shader);

    let image = image::open(IMAGE_PATH).unwrap().to_rgba8();
    let width = image.width();
    let height = image.height();

    // Generate the sphere vertices for three spheres, write them to a GPU buffer, and then drop the
    // CPU-side vertices to free memory.
    let vertex_count: usize = LATITUDE_SEGMENTS * LONGITUDE_SEGMENTS * 6 * 3;
    let mut vertices = Vec::with_capacity(vertex_count);
    vertices.extend(sphere_vertices(0.1, -1.2));
    vertices.extend(sphere_vertices(0.25, -0.4));
    vertices.extend(sphere_vertices(0.5, 0.8));
    let mut vertex_buffer = renderer
        .create_buffer(
            vertices.len() * std::mem::size_of::<Vertex>(),
            BufferUsage::Vertex,
        )
        .unwrap();
    renderer
        .write_buffer(&vertex_buffer, bytemuck::cast_slice(&vertices))
        .unwrap();
    drop(vertices);

    let mut sampler = renderer
        .create_sampler(&SamplerOptions {
            max_anisotropy: match device.supported_features.contains(&AnisotropicFiltering) {
                true => Some(device.max_sampler_anisotropy),
                false => None,
            },
            ..Default::default()
        })
        .unwrap();

    // Creating a texture creates a GPU command to transfer the raw pixel data from CPU to GPU, thus
    // we must begin a frame first. Once a texture is loaded, we can use it until we destroy it.
    renderer.begin_frame().unwrap();
    renderer.clear(RGB::default()).unwrap();
    let mut texture = renderer
        .create_texture(
            &TextureOptions {
                width,
                height,
                format: TextureFormat::Rgba8Srgb,
                mip_level_count: TextureOptions::max_mip_level_count(width, height),
            },
            image.as_raw(),
        )
        .unwrap();
    renderer.end_frame().unwrap();

    // Render the spheres with the texture applied.
    renderer.begin_frame().unwrap();
    renderer.clear(RGB::default()).unwrap();
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
        .record_graphics_command(&pipeline, &[&bindings], vertex_count as u32, 1)
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
