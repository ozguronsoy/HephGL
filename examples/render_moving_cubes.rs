use std::time::{Duration, Instant};

use heph_gl::{
    renderers::{
        Renderer,
        resources::{BufferUsage, GpuBuffer, ResourceBinding, ResourceBindingType},
        settings::{ColorBlending, GraphicsPipelineOptions},
    },
    shader::Shader,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

use crate::utils::{ExampleRenderer, SHADERS_DIR, get_best_device, run_example};

mod utils;

type Vertex = [f32; 10];

const POSITIONS: [[f32; 3]; 8] = [
    [-0.5, -0.5, -0.5],
    [0.5, -0.5, -0.5],
    [0.5, 0.5, -0.5],
    [-0.5, 0.5, -0.5],
    [-0.5, -0.5, 0.5],
    [0.5, -0.5, 0.5],
    [0.5, 0.5, 0.5],
    [-0.5, 0.5, 0.5],
];
const INDICES: [u32; 36] = [
    0, 1, 2, 2, 3, 0, 4, 6, 5, 6, 4, 7, 0, 3, 7, 7, 4, 0, 1, 5, 6, 6, 2, 1, 0, 4, 5, 5, 1, 0, 3, 2,
    6, 6, 7, 3,
];

fn cube_vertices(offset: [f32; 3], color: [f32; 4]) -> [Vertex; 8] {
    std::array::from_fn(|i| {
        let position = POSITIONS[i];

        [
            position[0],
            position[1],
            position[2],
            color[0],
            color[1],
            color[2],
            color[3],
            offset[0],
            offset[1],
            offset[2],
        ]
    })
}

fn example(renderer: &mut ExampleRenderer, _: RawWindowHandle, _: RawDisplayHandle) {
    let device = get_best_device(renderer);
    renderer.set_device(Some(&device), &[]).unwrap();

    let vert_shader = Shader::from_file(format!("{}/{}", SHADERS_DIR, "cube_vert.spv")).unwrap();
    let frag_shader = Shader::from_file(format!("{}/{}", SHADERS_DIR, "cube_frag.spv")).unwrap();
    let pipeline = renderer
        .create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions {
                blending: ColorBlending::Alpha,
                ..Default::default()
            },
        )
        .unwrap();
    drop(vert_shader);
    drop(frag_shader);

    let vertex_buffer_size = std::mem::size_of::<[Vertex; 8]>();
    let index_buffer_size = std::mem::size_of_val(&INDICES);

    let mut cube_a_buffer = renderer
        .create_buffer(vertex_buffer_size, BufferUsage::Vertex)
        .unwrap();
    let mut cube_b_buffer = renderer
        .create_buffer(vertex_buffer_size, BufferUsage::Vertex)
        .unwrap();
    let mut index_buffer = renderer
        .create_buffer(index_buffer_size, BufferUsage::Index)
        .unwrap();

    renderer
        .write_buffer(&index_buffer, bytemuck::cast_slice(&INDICES))
        .unwrap();

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        renderer.begin_frame().unwrap();
        renderer.clear(RGB::default()).unwrap();

        let time = start.elapsed().as_secs_f32();
        let movement = (time * 2.0).sin() * 0.2;

        let cube_a = cube_vertices([-0.1 + movement, -0.125, 0.5], [1.0, 0.0, 0.0, 1.0]);
        let cube_b = cube_vertices([0.1 - movement, 0.125, 0.2], [0.0, 0.0, 1.0, 0.6]);
        renderer
            .write_buffer(&cube_a_buffer, bytemuck::cast_slice(&cube_a))
            .unwrap();
        renderer
            .write_buffer(&cube_b_buffer, bytemuck::cast_slice(&cube_b))
            .unwrap();

        let cube_a_bindings = [
            ResourceBinding {
                binding: 0,
                resource: ResourceBindingType::Buffer {
                    handle: cube_a_buffer,
                    usage: BufferUsage::Vertex,
                    offset: 0,
                    size: cube_a_buffer.size(),
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
        ];
        let cube_b_bindings = [
            ResourceBinding {
                binding: 0,
                resource: ResourceBindingType::Buffer {
                    handle: cube_b_buffer,
                    usage: BufferUsage::Vertex,
                    offset: 0,
                    size: cube_b_buffer.size(),
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
        ];

        renderer
            .record_graphics_command(&pipeline, &[&cube_a_bindings], INDICES.len() as u32, 1)
            .unwrap();
        renderer
            .record_graphics_command(&pipeline, &[&cube_b_bindings], INDICES.len() as u32, 1)
            .unwrap();

        renderer.end_frame().unwrap();
    }

    renderer.wait_idle().unwrap();

    renderer.destroy_buffer(&mut cube_a_buffer).unwrap();
    renderer.destroy_buffer(&mut cube_b_buffer).unwrap();
    renderer.destroy_buffer(&mut index_buffer).unwrap();
    renderer.destroy_graphics_pipeline(&pipeline).unwrap();

    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Render Moving Cubes";
    const INIT_RENDERER: bool = true;
    run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
