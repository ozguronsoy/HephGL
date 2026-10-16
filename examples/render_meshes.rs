use std::time::{Duration, Instant};

use heph_gl::{
    camera::{Camera, Projection},
    graphics_device::Feature::AnisotropicFiltering,
    light::Lighting,
    material::MaterialProperties,
    math::{Mat4, Quat, Vec3},
    mesh::Mesh,
    renderers::{
        Renderer,
        resources::{BufferUsage, GpuBuffer, ResourceBinding, ResourceBindingType, TextureFormat},
        settings::{
            FeatureRequest, FrontFace, GraphicsPipelineOptions, SamplerAddressMode, SamplerOptions,
            TextureOptions,
        },
    },
    shader::Shader,
    transform::Transform,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

mod utils;

type Vertex = [f32; 8];

const MESH_PATH: &str = "assets/treasure_chest/treasure_chest_4k.gltf";
const IMAGE_PATH: &str = "assets/treasure_chest/treasure_chest_diff_4k.jpg";
const ARM_PATH: &str = "assets/treasure_chest/treasure_chest_arm_4k.jpg";
const NORMAL_PATH: &str = "assets/treasure_chest/treasure_chest_nor_gl_4k.jpg";
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
    settings.frames_in_flight = 2;
    settings.vsync = true;
    renderer.set_settings(settings).unwrap();

    let vert_shader =
        Shader::from_file(format!("{}/{}", utils::SHADERS_DIR, "camera_vert.spv")).unwrap();
    let frag_shader =
        Shader::from_file(format!("{}/{}", utils::SHADERS_DIR, "mesh_frag.spv")).unwrap();
    let pipeline = renderer
        .create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions {
                front_face: FrontFace::Clockwise,
                ..Default::default()
            },
        )
        .unwrap();
    drop(vert_shader);
    drop(frag_shader);

    let mesh = Mesh::from_file(MESH_PATH).unwrap();
    let mut vertices = mesh.vertices;
    let indices = mesh.indices;

    let mut min = Vec3::from_slice(&vertices[0][..3]);
    let mut max = min;
    for vertex in &vertices {
        let position = Vec3::from_slice(&vertex[..3]);
        min = min.min(position);
        max = max.max(position);
    }
    let center = (min + max) * 0.5;
    for vertex in &mut vertices {
        vertex[0] -= center.x;
        vertex[1] -= center.y;
        vertex[2] -= center.z;
    }
    let mut transform = Transform::new(
        Vec3::ZERO,
        Quat::IDENTITY,
        Vec3::splat(2.0 / (max - min).max_element()),
    );

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

    let index_count = indices.len() as u32;
    let mut index_buffer = renderer
        .create_buffer(
            indices.len() * std::mem::size_of::<u32>(),
            BufferUsage::Index,
        )
        .unwrap();
    renderer
        .write_buffer(&index_buffer, bytemuck::cast_slice(&indices))
        .unwrap();
    drop(indices);

    let image = image::open(IMAGE_PATH).unwrap().to_rgba8();
    let arm = image::open(ARM_PATH).unwrap().to_rgba8();
    let normal = image::open(NORMAL_PATH).unwrap().to_rgba8();
    let material = MaterialProperties {
        metallicness: 1.0,
        ..Default::default()
    };
    let lighting = Lighting {
        direction: Vec3::new(0.5, -0.8, 1.0),
        intensity: 3.0,
        ambient: RGB::new(0.15, 0.15, 0.15),
        ..Default::default()
    };
    let mut camera = Camera::new(Projection::Perspective {
        vertical_fov: std::f32::consts::FRAC_PI_3,
        aspect_ratio: 16.0 / 9.0,
        near: 0.1,
        far: 100.0,
    });
    camera.position = Vec3::new(1.6, 1.0, -3.0);
    camera.look_at(&Vec3::ZERO, &Vec3::Y).unwrap();

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
            address_mode_v: SamplerAddressMode::Repeat,
            max_anisotropy: match device.supported_features.contains(&AnisotropicFiltering) {
                true => Some(device.max_sampler_anisotropy),
                false => None,
            },
            ..Default::default()
        })
        .unwrap();

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

    let mut frame_index = 0;
    let start = Instant::now();
    loop {
        renderer.begin_frame().unwrap();
        renderer.clear(RGB::default()).unwrap();

        let progress = (start.elapsed().as_secs_f32() / ANIMATION_DURATION.as_secs_f32()).min(1.0);
        transform.rotation = Quat::from_rotation_y(progress * std::f32::consts::TAU);
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
                resource: ResourceBindingType::Buffer {
                    handle: index_buffer,
                    usage: BufferUsage::Index,
                    offset: 0,
                    size: index_buffer.size(),
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
            .record_graphics_command(&pipeline, &[&bindings], index_count, 1)
            .unwrap();

        renderer.end_frame().unwrap();

        frame_index = (frame_index + 1) % uniform_buffers.len();
        if progress == 1.0 {
            break;
        }
    }

    // Cleanup.
    renderer.wait_idle().unwrap();
    renderer.destroy_sampler(&mut sampler).unwrap();
    renderer.destroy_texture(&mut texture).unwrap();
    renderer.destroy_texture(&mut arm_texture).unwrap();
    renderer.destroy_texture(&mut normal_texture).unwrap();
    renderer.destroy_buffer(&mut material_buffer).unwrap();
    renderer.destroy_buffer(&mut vertex_buffer).unwrap();
    renderer.destroy_buffer(&mut index_buffer).unwrap();
    for uniform_buffer in &mut uniform_buffers {
        renderer.destroy_buffer(uniform_buffer).unwrap();
    }
    renderer.destroy_graphics_pipeline(&pipeline).unwrap();

    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Render Meshes";
    const INIT_RENDERER: bool = true;
    utils::run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
