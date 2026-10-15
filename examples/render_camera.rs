use std::time::{Duration, Instant};

use heph_gl::{
    camera::{Camera, Projection},
    graphics_device::Feature::AnisotropicFiltering,
    math::{Mat4, Quat, Vec3},
    renderers::{
        Renderer,
        resources::{BufferUsage, GpuBuffer, ResourceBinding, ResourceBindingType, TextureFormat},
        settings::{FeatureRequest, GraphicsPipelineOptions, SamplerOptions, TextureOptions},
    },
    shader::Shader,
    transform::Transform,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

mod utils;

type Vertex = [f32; 8];

const IMAGE_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_diff_1k.jpg";
const LATITUDE_SEGMENTS: usize = 32;
const LONGITUDE_SEGMENTS: usize = 64;
const ANIMATION_DURATION: Duration = Duration::from_secs(5);

// Calculates the position of a point on a sphere given its radius, x offset, and spherical
// coordinates (u, v).
fn sphere_vertex(u: f32, v: f32) -> Vertex {
    let theta = v * std::f32::consts::PI;
    let phi = u * std::f32::consts::TAU;
    let x = theta.sin() * phi.cos();
    let y = theta.cos();
    let z = theta.sin() * phi.sin();

    // On a unit sphere, each position is also its outward normal.
    [x, y, z, x, y, z, u, v]
}

// Calculates the sphere vertices for a given radius and x offset. The sphere is generated using
// latitude and longitude segments, creating a grid of vertices that form the surface of the sphere.
// Each vertex includes position, normal, and texture coordinates.
fn sphere_vertices() -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(LATITUDE_SEGMENTS * LONGITUDE_SEGMENTS * 6);
    for y in 0..LATITUDE_SEGMENTS {
        let v0 = y as f32 / LATITUDE_SEGMENTS as f32;
        let v1 = (y + 1) as f32 / LATITUDE_SEGMENTS as f32;
        for x in 0..LONGITUDE_SEGMENTS {
            let u0 = x as f32 / LONGITUDE_SEGMENTS as f32;
            let u1 = (x + 1) as f32 / LONGITUDE_SEGMENTS as f32;

            let p00 = sphere_vertex(u0, v0);
            let p10 = sphere_vertex(u1, v0);
            let p01 = sphere_vertex(u0, v1);
            let p11 = sphere_vertex(u1, v1);

            vertices.extend_from_slice(&[p00, p10, p11, p00, p11, p01]);
        }
    }
    vertices
}

fn example(
    renderer: &mut utils::renderer::ExampleRenderer,
    _: RawWindowHandle,
    _: RawDisplayHandle,
) {
    let device = utils::renderer::get_best_device(renderer);
    renderer
        .set_device(
            Some(&device),
            &[FeatureRequest::new(AnisotropicFiltering, false)],
        )
        .unwrap();
    utils::renderer::set_best_msaa(renderer);

    let mut settings = *renderer.get_settings();
    settings.frames_in_flight = 3;
    settings.vsync = true;
    renderer.set_settings(settings).unwrap();

    let vert_shader =
        Shader::from_file(format!("{}/{}", utils::SHADERS_DIR, "camera_vert.spv")).unwrap();
    let frag_shader = Shader::from_file(format!(
        "{}/{}",
        utils::SHADERS_DIR,
        "broken_brick_wall_frag.spv"
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

    let vertices = sphere_vertices();
    let mut vertex_buffer = renderer
        .create_buffer(
            vertices.len() * std::mem::size_of::<Vertex>(),
            BufferUsage::Vertex,
        )
        .unwrap();
    renderer
        .write_buffer(&vertex_buffer, bytemuck::cast_slice(&vertices))
        .unwrap();

    // The shader reads three mat4 values: model, view-projection, and normal.
    // Keep one buffer per frame in flight so updates do not overwrite data the GPU still reads.
    let mut uniform_buffers = Vec::with_capacity(settings.frames_in_flight);
    for _ in 0..settings.frames_in_flight {
        uniform_buffers.push(
            renderer
                .create_buffer(std::mem::size_of::<[[f32; 16]; 3]>(), BufferUsage::Uniform)
                .unwrap(),
        );
    }

    let image = image::open(IMAGE_PATH).unwrap().to_rgba8();
    let width = image.width();
    let height = image.height();
    let mut sampler = renderer
        .create_sampler(&SamplerOptions {
            max_anisotropy: match device.supported_features.contains(&AnisotropicFiltering) {
                true => Some(device.max_sampler_anisotropy),
                false => None,
            },
            ..Default::default()
        })
        .unwrap();

    // Upload the texture to the GPU.
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

    let mut transform = Transform::default();
    let camera = Camera::new(Projection::Perspective {
        vertical_fov: std::f32::consts::FRAC_PI_3,
        aspect_ratio: 16.0 / 9.0,
        near: 0.1,
        far: 100.0,
    });

    let mut frame_index = 0;
    let start = Instant::now();
    while start.elapsed() < ANIMATION_DURATION {
        renderer.begin_frame().unwrap();
        renderer.clear(RGB::default()).unwrap();

        let progress = (start.elapsed().as_secs_f32() / ANIMATION_DURATION.as_secs_f32()).min(1.0);

        // Move right (+X), up (+Y), and forward (+Z).
        transform.translation = Vec3::new(
            -1.2 + progress * 5.6,
            -0.5 + progress * 3.2,
            3.5 + progress * 21.5,
        );

        let angle = progress * 1.5 * std::f32::consts::TAU;
        transform.rotation = Quat::from_rotation_x(angle) * Quat::from_rotation_y(angle);

        // Matrices are uploaded by columns, in the same order as the shader's uniform block.
        let matrices = [
            transform.matrix().unwrap().to_cols_array(),
            camera.view_projection_matrix().unwrap().to_cols_array(),
            Mat4::from_mat3(transform.normal_matrix().unwrap()).to_cols_array(),
        ];
        let uniform_buffer = uniform_buffers[frame_index];
        renderer
            .write_buffer(&uniform_buffer, bytemuck::cast_slice(&matrices))
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
            ResourceBinding {
                binding: 2,
                resource: ResourceBindingType::Buffer {
                    handle: uniform_buffer,
                    usage: BufferUsage::Uniform,
                    offset: 0,
                    size: uniform_buffer.size(),
                },
            },
        ];
        renderer
            .record_graphics_command(&pipeline, &[&bindings], vertices.len() as u32, 1)
            .unwrap();

        renderer.end_frame().unwrap();
        frame_index = (frame_index + 1) % uniform_buffers.len();
    }

    // Cleanup.
    renderer.wait_idle().unwrap();
    renderer.destroy_sampler(&mut sampler).unwrap();
    renderer.destroy_texture(&mut texture).unwrap();
    renderer.destroy_buffer(&mut vertex_buffer).unwrap();
    for uniform_buffer in &mut uniform_buffers {
        renderer.destroy_buffer(uniform_buffer).unwrap();
    }
    renderer.destroy_graphics_pipeline(&pipeline).unwrap();

    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Render Camera";
    const INIT_RENDERER: bool = true;
    utils::run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
