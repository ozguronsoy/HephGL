use heph_gl::{
    graphics_device::Feature::ComputeShaders,
    renderers::{
        BufferUsage, FeatureRequest, GpuBuffer, PipelineHandle, Renderer, ResourceBinding,
        ResourceBindingType,
    },
    shader::ShaderSource,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::utils::{ExampleRenderer, SHADERS_DIR, get_best_device, run_example};

mod utils;

const BUFFER_LEN: usize = 256;
const BUFFER_SIZE: usize = BUFFER_LEN * std::mem::size_of::<f32>();

fn create_random_data() -> (Vec<f32>, Vec<f32>) {
    let mut a_data = vec![0.0f32; BUFFER_LEN];
    let mut b_data = vec![0.0f32; BUFFER_LEN];
    for j in 0..BUFFER_LEN {
        a_data[j] = rand::random_range(0.0f32..100.0f32).round();
        b_data[j] = rand::random_range(0.0f32..100.0f32).round();
    }
    (a_data, b_data)
}

fn example(renderer: &mut ExampleRenderer, _: RawWindowHandle, _: RawDisplayHandle) {
    let device = get_best_device(renderer);
    let features = [FeatureRequest {
        feature: ComputeShaders,
        required: true,
    }];
    renderer.set_device(Some(&device), &features).unwrap();

    // This shader takes one set of resources with 3 bindings (2 input buffers, and
    // an output buffer).
    let shader_source = ShaderSource::from_file(SHADERS_DIR.to_owned() + "/addition.spv").unwrap();
    let shader = renderer.create_shader(&shader_source).unwrap();
    let pipeline = renderer.create_compute_pipeline(&shader).unwrap();

    renderer.begin_frame().unwrap();

    // Create buffers in GPU, and fill them.
    let (a_data, b_data) = create_random_data();
    let mut buffer_a = renderer
        .create_buffer(BUFFER_SIZE, BufferUsage::Storage)
        .unwrap();
    let mut buffer_b = renderer
        .create_buffer(BUFFER_SIZE, BufferUsage::Storage)
        .unwrap();
    let mut buffer_c = renderer
        .create_buffer(BUFFER_SIZE, BufferUsage::Storage)
        .unwrap();
    renderer
        .write_buffer(&buffer_a, bytemuck::cast_slice(&a_data))
        .unwrap();
    renderer
        .write_buffer(&buffer_b, bytemuck::cast_slice(&b_data))
        .unwrap();

    // Bind the GPU buffers to the shader.
    let bindings = [
        // Input A
        ResourceBinding {
            binding: 0,
            resource: ResourceBindingType::Buffer {
                handle: buffer_a,
                usage: BufferUsage::Storage,
                offset: 0,
                size: buffer_a.size(),
            },
        },
        // Input B
        ResourceBinding {
            binding: 1,
            resource: ResourceBindingType::Buffer {
                handle: buffer_b,
                usage: BufferUsage::Storage,
                offset: 0,
                size: buffer_b.size(),
            },
        },
        // Output C
        ResourceBinding {
            binding: 2,
            resource: ResourceBindingType::Buffer {
                handle: buffer_c,
                usage: BufferUsage::Storage,
                offset: 0,
                size: buffer_c.size(),
            },
        },
    ];
    let resource_set = renderer
        .create_resource_set(&PipelineHandle::Compute(pipeline), &bindings)
        .unwrap();

    // Record a command and submit it to the GPU.
    let recorded_command = renderer
        .record_compute_pass(&pipeline, &[&resource_set], (1, 1, 1))
        .unwrap();
    renderer.submit_commands(&[recorded_command]).unwrap();

    renderer.end_frame().unwrap();

    // Wait for GPU to finish processing, read the result, and print it. In normal
    // operation, we would keep processing new frames instead.
    renderer.wait_idle().unwrap();
    let mut c_data = vec![0.0f32; BUFFER_LEN];
    renderer
        .read_buffer(&buffer_c, bytemuck::cast_slice_mut(&mut c_data))
        .unwrap();
    println!("Compute Result:");
    for i in 0..BUFFER_LEN {
        println!("{} + {} = {}", a_data[i], b_data[i], c_data[i]);
    }

    // Cleanup
    renderer.destroy_buffer(&mut buffer_a).unwrap();
    renderer.destroy_buffer(&mut buffer_b).unwrap();
    renderer.destroy_buffer(&mut buffer_c).unwrap();
    renderer.destroy_compute_pipeline(&pipeline).unwrap();
    renderer.destroy_shader(&shader).unwrap();

    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Compute (Single Thread / Single Frame)";
    const INIT_RENDERER: bool = true;
    run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
