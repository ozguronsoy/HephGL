use std::time::{Duration, Instant};

use heph_gl::{
    camera::{Camera, Projection},
    graphics_device::Feature::AnisotropicFiltering,
    light::Lighting,
    material::MaterialProperties,
    math::{Mat4, Quat, Vec3},
    renderers::{
        Renderer,
        resources::{BufferUsage, GpuBuffer, ResourceBinding, ResourceBindingType, TextureFormat},
        settings::{
            FeatureRequest, GraphicsPipelineOptions, SamplerAddressMode, SamplerOptions,
            TextureOptions,
        },
    },
    shader::Shader,
    transform::Transform,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

mod utils;

const IMAGE_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_diff_1k.jpg";
const DISPLACEMENT_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_disp_1k.png";
const ARM_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_arm_1k.exr";
const NORMAL_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_nor_gl_1k.exr";
const ANIMATION_DURATION: Duration = Duration::from_secs(5);

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

    let image = image::open(IMAGE_PATH).unwrap().to_rgba8();
    let displacement = image::open(DISPLACEMENT_PATH).unwrap().to_luma16();
    let arm = utils::load_exr(ARM_PATH, ["R", "G", "B"]);
    let normal = utils::load_exr(NORMAL_PATH, ["R", "G", "B"]);
    let material = MaterialProperties {
        // Use the metallicness stored in the ARM map without reducing it.
        metallicness: 1.0,
        displacement_scale: 0.1,
        ..Default::default()
    };
    let lighting = Lighting {
        direction: Vec3::new(0.3, 0.4, 1.0),
        intensity: 2.0,
        ambient: RGB::new(0.45, 0.15, 0.15),
        ..Default::default()
    };
    let camera = Camera::new(Projection::Perspective {
        vertical_fov: std::f32::consts::FRAC_PI_3,
        aspect_ratio: 16.0 / 9.0,
        near: 0.1,
        far: 100.0,
    });

    // Apply displacement in local space before animating the sphere's transform.
    let mut vertices = utils::renderer::sphere_vertices(1.0, 0.0);
    utils::renderer::displace_vertices(&mut vertices, &displacement, &material);
    let vertex_count = vertices.len();
    let mut vertex_buffer = renderer
        .create_buffer(
            vertex_count * std::mem::size_of::<utils::renderer::SphereVertex>(),
            BufferUsage::Vertex,
        )
        .unwrap();
    renderer
        .write_buffer(&vertex_buffer, bytemuck::cast_slice(&vertices))
        .unwrap();
    drop(vertices);

    let uniforms = [
        [
            material.base_color.r,
            material.base_color.g,
            material.base_color.b,
            material.base_color.a,
        ],
        [
            material.roughness,
            material.metallicness,
            material.normal_scale,
            0.0,
        ],
        [
            lighting.direction.x,
            lighting.direction.y,
            lighting.direction.z,
            0.0,
        ],
        [
            lighting.color.r,
            lighting.color.g,
            lighting.color.b,
            lighting.intensity,
        ],
        [
            lighting.ambient.r,
            lighting.ambient.g,
            lighting.ambient.b,
            0.0,
        ],
        [camera.position.x, camera.position.y, camera.position.z, 1.0],
    ];
    let mut material_buffer = renderer
        .create_buffer(std::mem::size_of_val(&uniforms), BufferUsage::Uniform)
        .unwrap();
    renderer
        .write_buffer(&material_buffer, bytemuck::cast_slice(&uniforms))
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

    let mut sampler = renderer
        .create_sampler(&SamplerOptions {
            address_mode_u: SamplerAddressMode::Repeat,
            max_anisotropy: match device.supported_features.contains(&AnisotropicFiltering) {
                true => Some(device.max_sampler_anisotropy),
                false => None,
            },
            ..Default::default()
        })
        .unwrap();

    // Upload the color, ARM, and normal textures to the GPU.
    renderer.begin_frame().unwrap();
    renderer.clear(RGB::default()).unwrap();
    let mut texture = renderer
        .create_texture(
            &TextureOptions {
                width: image.width(),
                height: image.height(),
                format: TextureFormat::Rgba8Srgb,
                mip_level_count: TextureOptions::max_mip_level_count(image.width(), image.height()),
            },
            image.as_raw(),
        )
        .unwrap();
    let mut arm_texture = renderer
        .create_texture(
            &TextureOptions {
                width: arm.width(),
                height: arm.height(),
                format: TextureFormat::Rgba8Unorm,
                mip_level_count: TextureOptions::max_mip_level_count(arm.width(), arm.height()),
            },
            arm.as_raw(),
        )
        .unwrap();
    let mut normal_texture = renderer
        .create_texture(
            &TextureOptions {
                width: normal.width(),
                height: normal.height(),
                format: TextureFormat::Rgba8Unorm,
                mip_level_count: TextureOptions::max_mip_level_count(
                    normal.width(),
                    normal.height(),
                ),
            },
            normal.as_raw(),
        )
        .unwrap();
    renderer.end_frame().unwrap();

    let mut transform = Transform::default();

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
                resource: ResourceBindingType::Texture {
                    handle: arm_texture,
                },
            },
            ResourceBinding {
                binding: 3,
                resource: ResourceBindingType::Texture {
                    handle: normal_texture,
                },
            },
            ResourceBinding {
                binding: 4,
                resource: ResourceBindingType::Buffer {
                    handle: material_buffer,
                    usage: BufferUsage::Uniform,
                    offset: 0,
                    size: material_buffer.size(),
                },
            },
            ResourceBinding {
                binding: 5,
                resource: ResourceBindingType::Buffer {
                    handle: uniform_buffer,
                    usage: BufferUsage::Uniform,
                    offset: 0,
                    size: uniform_buffer.size(),
                },
            },
        ];
        renderer
            .record_graphics_command(&pipeline, &[&bindings], vertex_count as u32, 1)
            .unwrap();

        renderer.end_frame().unwrap();
        frame_index = (frame_index + 1) % uniform_buffers.len();
    }

    // Cleanup.
    renderer.wait_idle().unwrap();
    renderer.destroy_sampler(&mut sampler).unwrap();
    renderer.destroy_texture(&mut texture).unwrap();
    renderer.destroy_texture(&mut arm_texture).unwrap();
    renderer.destroy_texture(&mut normal_texture).unwrap();
    renderer.destroy_buffer(&mut material_buffer).unwrap();
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
