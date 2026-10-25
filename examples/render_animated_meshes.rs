use std::time::{Duration, Instant};

use heph_gl::{
    camera::{Camera, Projection},
    graphics_device::GraphicsDeviceFeature::AnisotropicFiltering,
    light::Lighting,
    math::{Mat4, Vec3},
    mesh::{Mesh, MeshVertexAttribute},
    renderers::{
        Renderer,
        resources::{BufferUsage, ResourceBinding, ResourceBindingType, TextureFormat},
        settings::{
            FeatureRequest, FrontFace, GraphicsPipelineOptions, SamplerOptions, TextureOptions,
        },
    },
    shader::Shader,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

mod utils;

const MESH_PATH: &str = "assets/dragon/model.glb";
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
        Shader::from_file(format!("{}/{}", utils::SHADERS_DIR, "animation_vert.spv")).unwrap();
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

    // The dragon has a single skinned part and two animation clips.
    let part = mesh.parts().next().unwrap();
    let skeleton = mesh.skeleton().unwrap();
    let default_transforms = skeleton.transforms();
    let mut transforms = default_transforms.clone();
    let joint_matrices = part
        .joint_matrices(&skeleton.matrices(&transforms).unwrap())
        .unwrap();
    let vertex_attributes = [
        MeshVertexAttribute::Position,
        MeshVertexAttribute::Normal,
        MeshVertexAttribute::TextureCoordinates(part.base_texture_uv_set().unwrap()),
        MeshVertexAttribute::JointIndices(0),
        MeshVertexAttribute::JointWeights(0),
    ];
    let vertices = part.vertices(&vertex_attributes).unwrap();
    let component_count = part.vertex_component_count(&vertex_attributes);

    // Measure the rest pose after skinning, leaving the original vertices unchanged for animation.
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for vertex in vertices.chunks_exact(component_count) {
        let position = Vec3::from_slice(vertex);
        let mut skinned_position = Vec3::ZERO;
        for influence in 0..4 {
            skinned_position += joint_matrices[vertex[8 + influence] as usize]
                .transform_point3(position)
                * vertex[12 + influence];
        }
        min = min.min(skinned_position);
        max = max.max(skinned_position);
    }
    let model = Mat4::from_translation(Vec3::new(0.0, 0.0, 3.0))
        * Mat4::from_scale(Vec3::splat(5.0 / (max - min).max_element()))
        * Mat4::from_translation(-(min + max) * 0.5);

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

    renderer.begin_frame().unwrap();
    renderer.clear(RGB::default()).unwrap();
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

    let indices = part.indices();
    let mut index_buffer = renderer
        .create_buffer(std::mem::size_of_val(indices), BufferUsage::Index)
        .unwrap();
    renderer
        .write_buffer(index_buffer.as_mut(), 0, bytemuck::cast_slice(indices))
        .unwrap();

    let image = image::load_from_memory(part.base_texture_image_data().unwrap())
        .unwrap()
        .to_rgba8();
    let (min_filter, mag_filter, mipmap_filter) = part.base_texture_sampler_filters().unwrap();
    let (address_mode_u, address_mode_v) = part.base_texture_sampler_address_modes().unwrap();

    let mut texture = renderer
        .create_texture(
            &TextureOptions {
                width: image.width(),
                height: image.height(),
                format: TextureFormat::Rgba8Srgb,
                mip_level_count: match mipmap_filter.is_some() {
                    true => TextureOptions::max_mip_level_count(image.width(), image.height()),
                    false => 1,
                },
            },
            image.as_raw(),
        )
        .unwrap();
    // The dragon mesh does not have arm and normal textures.
    let mut arm_texture = renderer
        .create_texture(
            &TextureOptions {
                width: 1,
                height: 1,
                format: TextureFormat::Rgba8Unorm,
                mip_level_count: 1,
            },
            &[255, 255, 255, 255],
        )
        .unwrap();
    let mut normal_texture = renderer
        .create_texture(
            &TextureOptions {
                width: 1,
                height: 1,
                format: TextureFormat::Rgba8Unorm,
                mip_level_count: 1,
            },
            &[128, 128, 255, 255],
        )
        .unwrap();

    let material = part.material();
    let uniforms = [
        [
            material.base_color.r,
            material.base_color.g,
            material.base_color.b,
            material.base_color.a,
        ],
        [material.roughness, material.metallicness, 0.0, 0.0],
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

    renderer.end_frame().unwrap();

    let matrices = [
        model.to_cols_array(),
        camera.view_projection_matrix().unwrap().to_cols_array(),
        model.inverse().transpose().to_cols_array(),
    ];
    let mut uniform_buffer = renderer
        .create_buffer(std::mem::size_of_val(&matrices), BufferUsage::Uniform)
        .unwrap();
    renderer
        .write_buffer(uniform_buffer.as_mut(), 0, bytemuck::cast_slice(&matrices))
        .unwrap();

    let mut joint_buffers = Vec::with_capacity(settings.frames_in_flight);
    for _ in 0..settings.frames_in_flight {
        joint_buffers.push(
            renderer
                .create_buffer(
                    joint_matrices.len() * std::mem::size_of::<[f32; 16]>(),
                    BufferUsage::Storage,
                )
                .unwrap(),
        );
    }
    let mut sampler = renderer
        .create_sampler(&SamplerOptions {
            min_filter,
            mag_filter,
            mipmap_filter: mipmap_filter.unwrap_or_default(),
            address_mode_u,
            address_mode_v,
            max_anisotropy: match device.supported_features.contains(&AnisotropicFiltering) {
                true => Some(device.max_sampler_anisotropy),
                false => None,
            },
            ..Default::default()
        })
        .unwrap();

    let mut frame_index = 0;
    let start = Instant::now();
    loop {
        renderer.begin_frame().unwrap();
        renderer.clear(RGB::default()).unwrap();

        // Play each clip for five seconds, looping its keyframes at their original speed.
        let elapsed = start.elapsed().as_secs_f32();
        let clip_index = (elapsed / ANIMATION_DURATION.as_secs_f32()) as usize;
        if clip_index >= mesh.animations().len() {
            renderer.end_frame().unwrap();
            break;
        }

        let animation = &mesh.animations()[clip_index];
        let clip_time = elapsed % ANIMATION_DURATION.as_secs_f32();
        let time = match animation.duration() > 0.0 {
            true => clip_time % animation.duration(),
            false => 0.0,
        };

        transforms.clone_from_slice(&default_transforms);
        animation.sample(time, &mut transforms).unwrap();

        let matrices = skeleton.matrices(&transforms).unwrap();
        let joints: Vec<[f32; 16]> = part
            .joint_matrices(&matrices)
            .unwrap()
            .iter()
            .map(Mat4::to_cols_array)
            .collect();
        let joint_buffer = &mut joint_buffers[frame_index];
        renderer
            .write_buffer(joint_buffer.as_mut(), 0, bytemuck::cast_slice(&joints))
            .unwrap();

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
            ResourceBinding {
                binding: 6,
                resource: ResourceBindingType::Buffer {
                    handle: joint_buffer.as_ref(),
                    usage: BufferUsage::Storage,
                    offset: 0,
                    size: joint_buffer.size(),
                },
            },
        ];
        renderer
            .record_graphics_command(pipeline.as_mut(), &[&bindings], indices.len() as u32, 1)
            .unwrap();

        renderer.end_frame().unwrap();

        frame_index = (frame_index + 1) % joint_buffers.len();
    }

    // Cleanup.
    renderer.wait_idle().unwrap();
    renderer.destroy_sampler(sampler.as_mut()).unwrap();
    renderer.destroy_texture(texture.as_mut()).unwrap();
    renderer.destroy_texture(arm_texture.as_mut()).unwrap();
    renderer.destroy_texture(normal_texture.as_mut()).unwrap();
    renderer.destroy_buffer(material_buffer.as_mut()).unwrap();
    renderer.destroy_buffer(vertex_buffer.as_mut()).unwrap();
    renderer.destroy_buffer(index_buffer.as_mut()).unwrap();
    renderer.destroy_buffer(uniform_buffer.as_mut()).unwrap();
    for joint_buffer in &mut joint_buffers {
        renderer.destroy_buffer(joint_buffer.as_mut()).unwrap();
    }
    renderer
        .destroy_graphics_pipeline(pipeline.as_mut())
        .unwrap();

    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Render Animation";
    const INIT_RENDERER: bool = true;
    utils::run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
