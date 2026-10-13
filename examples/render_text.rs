use std::time::{Duration, Instant};

use heph_gl::{
    camera::{Camera, Projection},
    math::{Mat4, Quat, Vec3},
    renderers::{
        Renderer,
        resources::{BufferUsage, GpuBuffer, ResourceBinding, ResourceBindingType, TextureFormat},
        settings::{ColorBlending, GraphicsPipelineOptions, SamplerOptions, TextureOptions},
    },
    shader::Shader,
    text::{
        Font, TextVertex,
        options::{FontOptions, TextType},
    },
    transform::Transform,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

use crate::utils::{ExampleRenderer, SHADERS_DIR, get_best_device, run_example, set_best_msaa};

mod utils;

const FONT_PATH: &str = "assets/liberation_sans/LiberationSans-Regular.ttf";
const FONT_SIZE: f32 = 72.0;
const ANIMATION_DURATION: Duration = Duration::from_secs(10);

fn create_pipeline(
    renderer: &mut ExampleRenderer,
    vertex_shader: &str,
    fragment_shader: &str,
    options: &GraphicsPipelineOptions,
) -> <ExampleRenderer as Renderer>::GraphicsPipeline {
    let vert_shader = Shader::from_file(format!("{}/{}", SHADERS_DIR, vertex_shader)).unwrap();
    let frag_shader = Shader::from_file(format!("{}/{}", SHADERS_DIR, fragment_shader)).unwrap();
    renderer
        .create_graphics_pipeline(&[&vert_shader, &frag_shader], options)
        .unwrap()
}

fn create_vertex_buffer(
    renderer: &mut ExampleRenderer,
    vertices: &[TextVertex],
) -> <ExampleRenderer as Renderer>::Buffer {
    let buffer = renderer
        .create_buffer(std::mem::size_of_val(vertices), BufferUsage::Vertex)
        .unwrap();
    renderer
        .write_buffer(&buffer, bytemuck::cast_slice(vertices))
        .unwrap();
    buffer
}

fn center_vertices(vertices: &mut [TextVertex]) {
    let mut min = Vec3::from_slice(&vertices[0][..3]);
    let mut max = min;
    for vertex in vertices.iter() {
        let position = Vec3::from_slice(&vertex[..3]);
        min = min.min(position);
        max = max.max(position);
    }

    let center = (min + max) * 0.5;
    for vertex in vertices {
        vertex[0] -= center.x;
        vertex[1] -= center.y;
        vertex[2] -= center.z;
    }
}

fn example(renderer: &mut ExampleRenderer, _: RawWindowHandle, _: RawDisplayHandle) {
    let device = get_best_device(renderer);
    renderer.set_device(Some(&device), &[]).unwrap();
    set_best_msaa(renderer);

    let mut settings = *renderer.get_settings();
    settings.frames_in_flight = 3;
    settings.vsync = true;
    renderer.set_settings(settings).unwrap();

    let screen_pipeline = create_pipeline(
        renderer,
        "text_vert.spv",
        "text_frag.spv",
        &GraphicsPipelineOptions {
            blending: ColorBlending::Alpha,
            depth_test: false,
            depth_write: false,
            ..Default::default()
        },
    );
    let world_pipeline = create_pipeline(
        renderer,
        "text_world_vert.spv",
        "text_frag.spv",
        &GraphicsPipelineOptions {
            blending: ColorBlending::Alpha,
            ..Default::default()
        },
    );
    let extruded_pipeline = create_pipeline(
        renderer,
        "text_world_vert.spv",
        "text_extruded_frag.spv",
        &GraphicsPipelineOptions::default(),
    );

    // Load the font and generate vertex data for each text mesh. We only need to calculate the
    // vertices once since we aren't changing the text or font during the animation.
    let mut font = Font::from_file(
        FONT_PATH,
        &FontOptions {
            size: FONT_SIZE,
            enable_screen_text: true,
            enable_world_text: true,
            enable_extruded_text: true,
            ..Default::default()
        },
    )
    .unwrap();
    let screen_vertices = font
        .vertices(
            "Screen Text",
            &Vec3::new(460.0, 36.0, 0.0),
            [1.0, 1.0, 1.0, 1.0],
            TextType::Screen {
                viewport_size: [1280.0, 720.0],
            },
        )
        .unwrap();
    let mut world_vertices = font
        .vertices(
            "World Text",
            &Vec3::ZERO,
            [0.3, 0.8, 1.0, 1.0],
            TextType::World { scale: 0.018 },
        )
        .unwrap();
    let mut extruded_vertices = font
        .vertices(
            "Extruded Text",
            &Vec3::ZERO,
            [1.0, 0.65, 0.2, 1.0],
            TextType::Extruded {
                scale: 0.018,
                depth: 0.35,
                curve_tolerance_divider: 512.0,
            },
        )
        .unwrap();

    // Center each world-space mesh so its Y rotation pivots around the middle of the text.
    center_vertices(&mut world_vertices);
    center_vertices(&mut extruded_vertices);
    let mut screen_vertex_buffer = create_vertex_buffer(renderer, &screen_vertices);
    let mut world_vertex_buffer = create_vertex_buffer(renderer, &world_vertices);
    let mut extruded_vertex_buffer = create_vertex_buffer(renderer, &extruded_vertices);

    // Each world-space object needs its own matrices for every frame in flight.
    let mut uniform_buffers = Vec::with_capacity(settings.frames_in_flight as usize);
    for _ in 0..settings.frames_in_flight {
        uniform_buffers.push([
            renderer
                .create_buffer(std::mem::size_of::<[[f32; 16]; 3]>(), BufferUsage::Uniform)
                .unwrap(),
            renderer
                .create_buffer(std::mem::size_of::<[[f32; 16]; 3]>(), BufferUsage::Uniform)
                .unwrap(),
        ]);
    }

    let mut sampler = renderer.create_sampler(&SamplerOptions::default()).unwrap();

    // Upload the font atlas texture to the GPU. We only need to do this once since we aren't
    // changing the text or font during the animation.
    renderer.begin_frame().unwrap();
    renderer.clear(RGB::default()).unwrap();
    let mut texture = renderer
        .create_texture(
            &TextureOptions {
                width: font.atlas_width(),
                height: font.atlas_height(),
                format: TextureFormat::R8Unorm,
                mip_level_count: 1,
            },
            font.atlas_data(),
        )
        .unwrap();
    renderer.end_frame().unwrap();

    let camera = Camera::new(Projection::Perspective {
        vertical_fov: std::f32::consts::FRAC_PI_3,
        aspect_ratio: 16.0 / 9.0,
        near: 0.1,
        far: 100.0,
    });
    let mut transforms = [
        Transform::new(Vec3::new(0.0, 1.3, 8.0), Quat::IDENTITY, Vec3::ONE),
        Transform::new(Vec3::new(0.0, -1.3, 8.0), Quat::IDENTITY, Vec3::ONE),
    ];

    let mut frame_index = 0;
    let start = Instant::now();
    while start.elapsed() < ANIMATION_DURATION {
        renderer.begin_frame().unwrap();
        renderer.clear(RGB::default()).unwrap();

        // Rotate the world-space text meshes around the Y axis based on the elapsed time. We also
        // update the uniform buffers for each frame in flight with the new matrices.
        let progress = (start.elapsed().as_secs_f32() / ANIMATION_DURATION.as_secs_f32()).min(1.0);
        let angle = progress * std::f32::consts::TAU;
        for (index, transform) in transforms.iter_mut().enumerate() {
            transform.rotation = Quat::from_rotation_y(angle);
            let matrices = [
                transform.matrix().unwrap().to_cols_array(),
                camera.view_projection_matrix().unwrap().to_cols_array(),
                Mat4::from_mat3(transform.normal_matrix().unwrap()).to_cols_array(),
            ];
            renderer
                .write_buffer(
                    &uniform_buffers[frame_index][index],
                    bytemuck::cast_slice(&matrices),
                )
                .unwrap();
        }

        let screen_bindings = [
            ResourceBinding {
                binding: 0,
                resource: ResourceBindingType::Buffer {
                    handle: screen_vertex_buffer,
                    usage: BufferUsage::Vertex,
                    offset: 0,
                    size: screen_vertex_buffer.size(),
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
        let world_bindings = [
            ResourceBinding {
                binding: 0,
                resource: ResourceBindingType::Buffer {
                    handle: world_vertex_buffer,
                    usage: BufferUsage::Vertex,
                    offset: 0,
                    size: world_vertex_buffer.size(),
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
                    handle: uniform_buffers[frame_index][0],
                    usage: BufferUsage::Uniform,
                    offset: 0,
                    size: uniform_buffers[frame_index][0].size(),
                },
            },
        ];
        let extruded_bindings = [
            ResourceBinding {
                binding: 0,
                resource: ResourceBindingType::Buffer {
                    handle: extruded_vertex_buffer,
                    usage: BufferUsage::Vertex,
                    offset: 0,
                    size: extruded_vertex_buffer.size(),
                },
            },
            ResourceBinding {
                binding: 2,
                resource: ResourceBindingType::Buffer {
                    handle: uniform_buffers[frame_index][1],
                    usage: BufferUsage::Uniform,
                    offset: 0,
                    size: uniform_buffers[frame_index][1].size(),
                },
            },
        ];

        renderer
            .record_graphics_command(
                &screen_pipeline,
                &[&screen_bindings],
                screen_vertices.len() as u32,
                1,
            )
            .unwrap();
        renderer
            .record_graphics_command(
                &world_pipeline,
                &[&world_bindings],
                world_vertices.len() as u32,
                1,
            )
            .unwrap();
        renderer
            .record_graphics_command(
                &extruded_pipeline,
                &[&extruded_bindings],
                extruded_vertices.len() as u32,
                1,
            )
            .unwrap();

        renderer.end_frame().unwrap();
        frame_index = (frame_index + 1) % uniform_buffers.len();
    }

    // Cleanup.
    renderer.wait_idle().unwrap();
    renderer.destroy_sampler(&mut sampler).unwrap();
    renderer.destroy_texture(&mut texture).unwrap();
    renderer.destroy_buffer(&mut screen_vertex_buffer).unwrap();
    renderer.destroy_buffer(&mut world_vertex_buffer).unwrap();
    renderer
        .destroy_buffer(&mut extruded_vertex_buffer)
        .unwrap();
    for frame_buffers in &mut uniform_buffers {
        for buffer in frame_buffers {
            renderer.destroy_buffer(buffer).unwrap();
        }
    }
    renderer
        .destroy_graphics_pipeline(&screen_pipeline)
        .unwrap();
    renderer.destroy_graphics_pipeline(&world_pipeline).unwrap();
    renderer
        .destroy_graphics_pipeline(&extruded_pipeline)
        .unwrap();

    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Render Text";
    const INIT_RENDERER: bool = true;
    run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
