use std::{
    collections::HashMap,
    path::Path,
    time::{Duration, Instant},
};

use heph_gl::{
    camera::{Camera, Projection},
    graphics_device::Feature::AnisotropicFiltering,
    light::Lighting,
    math::{Mat4, Quat, Vec3},
    mesh::{Mesh, MeshVertexAttribute},
    renderers::{
        Renderer,
        resources::{BufferUsage, ResourceBinding, ResourceBindingType, TextureFormat},
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

const MESH_PATH: &str = "assets/ship/ship_pinnace_4k.gltf";
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
    let mut pipeline = renderer
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

    // Find the bounds of the whole mesh so every part uses the same center and scale.
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for part in mesh.parts() {
        let positions = part.vertices(&[MeshVertexAttribute::Position]).unwrap();
        for position in positions.as_chunks::<3>().0 {
            let position = Vec3::from_slice(position);
            min = min.min(position);
            max = max.max(position);
        }
    }
    let center = (min + max) * 0.5;
    let mut transform = Transform::new(
        Vec3::ZERO,
        Quat::IDENTITY,
        Vec3::splat(3.0 / (max - min).max_element()),
    );

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

    let mesh_directory = Path::new(MESH_PATH).parent().unwrap();
    let mut textures = HashMap::new();
    let mut part_buffers = Vec::with_capacity(mesh.part_count());
    renderer.begin_frame().unwrap();
    renderer.clear(RGB::default()).unwrap();
    for part in mesh.parts() {
        // All textures in this ship use the same UV set; the shader expects [px, py, pz, nx, ny,
        // nz, u, v].
        let vertex_attributes = [
            MeshVertexAttribute::Position,
            MeshVertexAttribute::Normal,
            MeshVertexAttribute::TextureCoordinates(part.base_texture_uv_set().unwrap()),
        ];
        let mut vertices = part.vertices(&vertex_attributes).unwrap();
        let component_count = part.vertex_component_count(&vertex_attributes);
        for vertex in vertices.chunks_exact_mut(component_count) {
            vertex[0] -= center.x;
            vertex[1] -= center.y;
            vertex[2] -= center.z;
        }
        let vertex_count = vertices.len() / component_count;
        let mut vertex_buffer = renderer
            .create_buffer(
                vertex_count * part.vertex_size_bytes(&vertex_attributes),
                BufferUsage::Vertex,
            )
            .unwrap();
        renderer
            .write_buffer(vertex_buffer.as_mut(), 0, bytemuck::cast_slice(&vertices))
            .unwrap();

        // Indices reference this part's vertices, so each part is drawn with its own buffers.
        let indices = part.indices();
        let mut index_buffer = renderer
            .create_buffer(std::mem::size_of_val(indices), BufferUsage::Index)
            .unwrap();
        renderer
            .write_buffer(index_buffer.as_mut(), 0, bytemuck::cast_slice(indices))
            .unwrap();

        // Resolve each texture URI relative to the mesh directory, preserving its folder structure.
        // The HashMap avoids decoding and creating shared textures more than once.
        let part_textures = [
            (
                part.base_texture_image_uri().unwrap(),
                TextureFormat::Rgba8Srgb,
            ),
            (
                part.arm_texture_image_uri().unwrap(),
                TextureFormat::Rgba8Unorm,
            ),
            (
                part.normal_texture_image_uri().unwrap(),
                TextureFormat::Rgba8Unorm,
            ),
        ]
        .map(|(uri, format)| {
            let _ = textures.entry(uri.to_owned()).or_insert_with(|| {
                let image = image::open(mesh_directory.join(uri)).unwrap().to_rgba8();
                renderer
                    .create_texture(
                        &TextureOptions {
                            width: image.width(),
                            height: image.height(),
                            format,
                            mip_level_count: TextureOptions::max_mip_level_count(
                                image.width(),
                                image.height(),
                            ),
                        },
                        image.as_raw(),
                    )
                    .unwrap()
            });
            uri.to_owned()
        });
        // Each part uses the material properties loaded from the mesh.
        let material = part.material();
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
            .write_buffer(material_buffer.as_mut(), 0, bytemuck::cast_slice(&uniforms))
            .unwrap();

        part_buffers.push((
            vertex_buffer,
            index_buffer,
            indices.len() as u32,
            part_textures,
            material_buffer,
        ));
    }

    renderer.end_frame().unwrap();

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

    let part_texture_handles: Vec<_> = part_buffers
        .iter()
        .map(|(_, _, _, uris, _)| uris.each_ref().map(|uri| &textures[uri]))
        .collect();

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

        let uniform_buffer = &mut uniform_buffers[frame_index];
        renderer
            .write_buffer(uniform_buffer.as_mut(), 0, bytemuck::cast_slice(&matrices))
            .unwrap();

        for (
            (vertex_buffer, index_buffer, index_count, _, material_buffer),
            [texture, arm_texture, normal_texture],
        ) in part_buffers.iter().zip(&part_texture_handles)
        {
            let bindings = [
                ResourceBinding {
                    binding: 0,
                    resource: ResourceBindingType::Buffer {
                        handle: vertex_buffer.as_ref(),
                        usage: BufferUsage::Vertex,
                        offset: 0,
                        size: vertex_buffer.size(),
                    },
                },
                ResourceBinding {
                    binding: 0,
                    resource: ResourceBindingType::Buffer {
                        handle: index_buffer.as_ref(),
                        usage: BufferUsage::Index,
                        offset: 0,
                        size: index_buffer.size(),
                    },
                },
                ResourceBinding {
                    binding: 0,
                    resource: ResourceBindingType::Texture {
                        handle: texture.as_ref(),
                    },
                },
                ResourceBinding {
                    binding: 1,
                    resource: ResourceBindingType::Sampler {
                        handle: sampler.as_ref(),
                    },
                },
                ResourceBinding {
                    binding: 2,
                    resource: ResourceBindingType::Texture {
                        handle: arm_texture.as_ref(),
                    },
                },
                ResourceBinding {
                    binding: 3,
                    resource: ResourceBindingType::Texture {
                        handle: normal_texture.as_ref(),
                    },
                },
                ResourceBinding {
                    binding: 4,
                    resource: ResourceBindingType::Buffer {
                        handle: material_buffer.as_ref(),
                        usage: BufferUsage::Uniform,
                        offset: 0,
                        size: material_buffer.size(),
                    },
                },
                ResourceBinding {
                    binding: 5,
                    resource: ResourceBindingType::Buffer {
                        handle: uniform_buffer.as_ref(),
                        usage: BufferUsage::Uniform,
                        offset: 0,
                        size: uniform_buffer.size(),
                    },
                },
            ];
            renderer
                .record_graphics_command(pipeline.as_mut(), &[&bindings], *index_count, 1)
                .unwrap();
        }

        renderer.end_frame().unwrap();

        frame_index = (frame_index + 1) % uniform_buffers.len();
        if progress == 1.0 {
            break;
        }
    }

    // Cleanup.
    renderer.wait_idle().unwrap();
    renderer.destroy_sampler(sampler.as_mut()).unwrap();
    for texture in textures.values_mut() {
        renderer.destroy_texture(texture.as_mut()).unwrap();
    }

    for (vertex_buffer, index_buffer, _, _, material_buffer) in &mut part_buffers {
        renderer.destroy_buffer(material_buffer.as_mut()).unwrap();
        renderer.destroy_buffer(vertex_buffer.as_mut()).unwrap();
        renderer.destroy_buffer(index_buffer.as_mut()).unwrap();
    }
    for uniform_buffer in &mut uniform_buffers {
        renderer.destroy_buffer(uniform_buffer.as_mut()).unwrap();
    }
    renderer
        .destroy_graphics_pipeline(pipeline.as_mut())
        .unwrap();

    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Render Meshes";
    const INIT_RENDERER: bool = true;
    utils::run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
