use heph_gl::{
    graphics_device::Feature::ComputeShaders,
    renderers::{
        BufferUsage, FeatureRequest, GpuBuffer, PipelineHandle, Renderer, ResourceBinding,
        ResourceBindingType, Settings,
    },
    shader::ShaderSource,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::utils::{ExampleRenderer, SHADERS_DIR, get_best_device, run_example};

mod utils;

// The number of frames we can process in parallel.
const FRAMES_IN_FLIGHT: usize = 2;
// The number of frames we want to execute in total.
const N_FRAMES: usize = FRAMES_IN_FLIGHT * 10;
const BUFFER_LEN: usize = 256;
const BUFFER_SIZE: usize = BUFFER_LEN * std::mem::size_of::<f32>();

fn create_data(frame: usize) -> (Vec<f32>, Vec<f32>) {
    let mut a_data = vec![0.0f32; BUFFER_LEN];
    let mut b_data = vec![0.0f32; BUFFER_LEN];
    for j in 0..BUFFER_LEN {
        a_data[j] = ((j + 10) * (frame + 1)) as f32;
        b_data[j] = ((j + 20) * (frame + 1)) as f32;
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

    // Settings can be set anytime, but you must ensure all processing in GPU is
    // done before changing it.
    renderer.wait_idle().unwrap();
    renderer
        .set_settings(Settings {
            frames_in_flight: FRAMES_IN_FLIGHT as u32,
        })
        .unwrap();

    // This shader takes one set of resources with 3 bindings (2 input buffers, and
    // an output buffer).
    let shader_source = ShaderSource::from_file(SHADERS_DIR.to_owned() + "/addition.spv").unwrap();
    let shader = renderer.create_shader(&shader_source).unwrap();
    let pipeline = renderer.create_compute_pipeline(&shader).unwrap();

    // Create resources for each frame. For this example, we only have general
    // purpose storage buffers in GPU as resources.
    struct FrameResources<T> {
        buffer_a: T,
        buffer_b: T,
        buffer_c: T,
    }
    let mut frame_resources = Vec::with_capacity(FRAMES_IN_FLIGHT);
    for _ in 0..FRAMES_IN_FLIGHT {
        frame_resources.push(FrameResources {
            buffer_a: renderer
                .create_buffer(BUFFER_SIZE, BufferUsage::Storage)
                .unwrap(),
            buffer_b: renderer
                .create_buffer(BUFFER_SIZE, BufferUsage::Storage)
                .unwrap(),
            buffer_c: renderer
                .create_buffer(BUFFER_SIZE, BufferUsage::Storage)
                .unwrap(),
        });
    }

    for i in 0..(N_FRAMES + FRAMES_IN_FLIGHT) {
        renderer.begin_frame().unwrap();
        let frame = &mut frame_resources[i % FRAMES_IN_FLIGHT];

        if i >= FRAMES_IN_FLIGHT {
            // Read and print results from the previous frame.
            let mut data_a = vec![0.0f32; BUFFER_LEN];
            let mut data_b = vec![0.0f32; BUFFER_LEN];
            let mut data_c = vec![0.0f32; BUFFER_LEN];
            renderer
                .read_buffer(&frame.buffer_a, bytemuck::cast_slice_mut(&mut data_a))
                .unwrap();
            renderer
                .read_buffer(&frame.buffer_b, bytemuck::cast_slice_mut(&mut data_b))
                .unwrap();
            renderer
                .read_buffer(&frame.buffer_c, bytemuck::cast_slice_mut(&mut data_c))
                .unwrap();

            let index = 0;
            let current_frame = i - FRAMES_IN_FLIGHT + 1;
            println!(
                "(frame:{}) {} + {} = {}",
                current_frame, data_a[index], data_b[index], data_c[index]
            );

            if i >= N_FRAMES {
                // We are reading the results from final frames now, do not submit new work.
                renderer.end_frame().unwrap();
                continue;
            }
        }

        // Fill the input buffers.
        let (a_data, b_data) = create_data(i);
        renderer
            .write_buffer(&frame.buffer_a, bytemuck::cast_slice(&a_data))
            .unwrap();
        renderer
            .write_buffer(&frame.buffer_b, bytemuck::cast_slice(&b_data))
            .unwrap();

        // Bind the GPU buffers to the shader.
        let bindings = [
            // Input A
            ResourceBinding {
                binding: 0,
                resource: ResourceBindingType::Buffer {
                    handle: frame.buffer_a,
                    usage: BufferUsage::Storage,
                    offset: 0,
                    size: frame.buffer_a.size(),
                },
            },
            // Input B
            ResourceBinding {
                binding: 1,
                resource: ResourceBindingType::Buffer {
                    handle: frame.buffer_b,
                    usage: BufferUsage::Storage,
                    offset: 0,
                    size: frame.buffer_b.size(),
                },
            },
            // Output C
            ResourceBinding {
                binding: 2,
                resource: ResourceBindingType::Buffer {
                    handle: frame.buffer_c,
                    usage: BufferUsage::Storage,
                    offset: 0,
                    size: frame.buffer_c.size(),
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
    }

    // Cleanup
    for frame in &mut frame_resources {
        renderer.destroy_buffer(&mut frame.buffer_a).unwrap();
        renderer.destroy_buffer(&mut frame.buffer_b).unwrap();
        renderer.destroy_buffer(&mut frame.buffer_c).unwrap();
    }
    renderer.destroy_compute_pipeline(&pipeline).unwrap();
    renderer.destroy_shader(&shader).unwrap();

    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Compute (Single Thread / Multi Frame)";
    const INIT_RENDERER: bool = true;
    run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
