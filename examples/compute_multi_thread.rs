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

const N_THREADS: usize = 10;
const BUFFER_LEN: usize = 256;
const BUFFER_SIZE: usize = BUFFER_LEN * std::mem::size_of::<f32>();

fn create_data(thread_index: usize) -> (Vec<f32>, Vec<f32>) {
    let mut data_a = vec![0.0f32; BUFFER_LEN];
    let mut data_b = vec![0.0f32; BUFFER_LEN];
    for j in 0..BUFFER_LEN {
        data_a[j] = ((j + 10) * (thread_index + 1)) as f32;
        data_b[j] = ((j + 20) * (thread_index + 1)) as f32;
    }
    (data_a, data_b)
}

// This example processes multiple additions in a single frame.
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

    std::thread::scope(|s| {
        let (tx, rx) = std::sync::mpsc::channel();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(N_THREADS + 1));

        renderer.begin_frame().unwrap();

        // Share the renderer across threads.
        let p_renderer = renderer as *mut ExampleRenderer as usize;
        for thread_index in 0..N_THREADS {
            let tx = tx.clone();
            let barrier = barrier.clone();

            s.spawn(move || {
                let renderer = unsafe { &mut *(p_renderer as *mut ExampleRenderer) };

                // Every thread have its own internal data which must be initialized. This is
                // done internally for the main thread so we don't have to explicitly call
                // these in the main thread.
                renderer.initialize_thread().unwrap();

                // Create buffers in GPU, and fill them.
                let (data_a, data_b) = create_data(thread_index);
                let buffer_a = renderer
                    .create_buffer(BUFFER_SIZE, BufferUsage::Storage)
                    .unwrap();
                let buffer_b = renderer
                    .create_buffer(BUFFER_SIZE, BufferUsage::Storage)
                    .unwrap();
                let buffer_c = renderer
                    .create_buffer(BUFFER_SIZE, BufferUsage::Storage)
                    .unwrap();
                renderer
                    .write_buffer(&buffer_a, bytemuck::cast_slice(&data_a))
                    .unwrap();
                renderer
                    .write_buffer(&buffer_b, bytemuck::cast_slice(&data_b))
                    .unwrap();

                // Bind the GPU buffers to the shader.
                let bindings = [
                    ResourceBinding {
                        binding: 0,
                        resource: ResourceBindingType::Buffer {
                            handle: buffer_a,
                            usage: BufferUsage::Storage,
                            offset: 0,
                            size: buffer_a.size(),
                        },
                    },
                    ResourceBinding {
                        binding: 1,
                        resource: ResourceBindingType::Buffer {
                            handle: buffer_b,
                            usage: BufferUsage::Storage,
                            offset: 0,
                            size: buffer_b.size(),
                        },
                    },
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

                // Record a command. We will submit it to the GPU in the main thread with the
                // other commands.
                let recorded_command = renderer
                    .record_compute_pass(&pipeline, &[&resource_set], (1, 1, 1))
                    .unwrap();

                // Send resources we prepared to the main thread, so we can submit them to the
                // GPU.
                tx.send((
                    thread_index + 1,
                    recorded_command,
                    buffer_a,
                    buffer_b,
                    buffer_c,
                    data_a,
                    data_b,
                ))
                .unwrap();

                // Wait until the main thread receives the recorded commands and resources from
                // all threads, and GPU finishes processing them.
                barrier.wait();

                // We are finished with this thread. Free the resources so another thread can
                // use them later.
                renderer.uninitialize_thread().unwrap();
            });
        }

        // Create command for each thread.
        let mut commands = Vec::with_capacity(N_THREADS);
        let mut cleanup_data = Vec::with_capacity(N_THREADS);
        for _ in 0..N_THREADS {
            let (thread, command, buffer_a, buffer_b, buffer_c, data_a, data_b) =
                rx.recv().unwrap();
            commands.push(command);
            cleanup_data.push((thread, buffer_a, buffer_b, buffer_c, data_a, data_b));
        }

        renderer.submit_commands(&commands).unwrap();

        // Wait for GPU to finish processing.
        renderer.wait_idle().unwrap();
        barrier.wait();

        for (thread, buffer_a, buffer_b, buffer_c, data_a, data_b) in &mut cleanup_data {
            // Read and print all computations.
            let mut data_c = vec![0.0f32; BUFFER_LEN];

            renderer
                .read_buffer(buffer_c, bytemuck::cast_slice_mut(&mut data_c))
                .unwrap();

            let index = 0;
            println!(
                "(thread:{}) {} + {} = {}",
                thread, data_a[index], data_b[index], data_c[index]
            );

            renderer.destroy_buffer(buffer_a).unwrap();
            renderer.destroy_buffer(buffer_b).unwrap();
            renderer.destroy_buffer(buffer_c).unwrap();
        }

        renderer.end_frame().unwrap();
    });

    // Cleanup
    renderer.destroy_compute_pipeline(&pipeline).unwrap();
    renderer.destroy_shader(&shader).unwrap();

    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Compute (Multi Thread)";
    const INIT_RENDERER: bool = true;
    run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
