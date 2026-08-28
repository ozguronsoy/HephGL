mod utils;
use std::{
    fmt::Debug,
    sync::{LazyLock, Mutex},
};

use heph_gl::{
    graphics_device::{
        Feature,
        Type::{Cpu, DiscreteGpu, IntegratedGpu},
    },
    renderers::{
        BufferUsage, FeatureRequest, GpuBuffer, InitializeOptions, PipelineHandle, Renderer,
        ResourceBinding, ResourceBindingType, Settings, vulkan_renderer::VulkanRenderer,
    },
    shader::ShaderSource,
};
use libtest_mimic::{Arguments, Trial};

use crate::utils::{SHADERS_DIR, TestEnv};

const APP_NAME: &str = "Vulkan Tests";
pub static TEST_ENV: LazyLock<Mutex<TestEnv>> = LazyLock::new(|| Mutex::new(TestEnv::default()));

#[macro_export]
macro_rules! test_env {
    () => {
        TEST_ENV.lock().unwrap()
    };
}

fn create_renderer() -> VulkanRenderer {
    let test_env = test_env!();
    let mut renderer = VulkanRenderer::new();
    heph_expect_success!(renderer.initialize(&InitializeOptions {
        app_name: APP_NAME,
        window_handle: test_env.raw_window_handle(),
        display_handle: test_env.raw_display_handle(),
    }));
    renderer
}

fn create_renderer_with_target_device(
    target_device_type: heph_gl::graphics_device::Type,
    requested_features: &[FeatureRequest],
    target_device_type_required: bool,
) -> Option<VulkanRenderer> {
    let mut renderer = create_renderer();
    let devices = heph_expect_success!(renderer.enumerate_devices());
    let target_device = devices.iter().find(|d| d.device_type == target_device_type);
    if target_device.is_none() {
        if target_device_type_required {
            panic!("Invalid Test Env: No {} found.", target_device_type);
        } else {
            return None;
        }
    }
    heph_expect_success!(renderer.set_device(target_device.as_ref().unwrap(), requested_features));

    let result = renderer.get_device();
    assert!(
        result.is_some(),
        "Renderer successfully set the graphics device, but `get_device()` returned `None`."
    );
    assert_eq!(target_device.unwrap(), result.unwrap());

    Some(renderer)
}

fn create_renderer_with_any_device(requested_features: &[FeatureRequest]) -> VulkanRenderer {
    let mut renderer = create_renderer();
    let devices = heph_expect_success!(renderer.enumerate_devices());
    let target_device = devices.first();
    assert!(
        target_device.is_some(),
        "Invalid Test Env: No device found.",
    );
    heph_expect_success!(renderer.set_device(target_device.as_ref().unwrap(), requested_features));

    let result = renderer.get_device();
    assert!(
        result.is_some(),
        "Renderer successfully set the graphics device, but `get_device()` returned `None`."
    );
    assert_eq!(target_device.unwrap(), result.unwrap());

    renderer
}

fn test_initialize_renderer() {
    let mut renderer = create_renderer();
    heph_expect_success!(renderer.uninitialize());
}

fn test_enumerate_devices() {
    let mut renderer = create_renderer();
    let devices = heph_expect_success!(renderer.enumerate_devices());
    assert!(
        !devices.is_empty(),
        "Invalid Test Env: No graphics device found to run remaining tests."
    );

    // For debugging CI.
    for device in &devices {
        println!("{}", device);
    }
}

fn test_set_device() {
    create_renderer_with_any_device(&[]);
}

fn test_set_settings() {
    let mut renderer = create_renderer_with_any_device(&[]);
    let settings = Settings {
        frames_in_flight: 10,
    };
    heph_expect_success!(renderer.set_settings(settings));

    let result = renderer.get_settings();
    assert_eq!(settings.frames_in_flight, result.frames_in_flight);
}

fn test_buffer<T>(data: &Vec<T>, usage: heph_gl::renderers::BufferUsage)
where
    T: bytemuck::Pod + PartialEq + Default + Debug,
{
    let renderer = create_renderer_with_any_device(&[]);

    let buffer_size = data.len() * size_of::<T>();
    let mut buffer = heph_expect_success!(renderer.create_buffer(buffer_size, usage));
    assert_eq!(buffer.size(), buffer_size);

    heph_expect_success!(renderer.write_buffer(&buffer, bytemuck::cast_slice(data)));

    let mut output = vec![T::default(); data.len()];
    heph_expect_success!(renderer.read_buffer(&buffer, bytemuck::cast_slice_mut(&mut output)));

    assert_eq!(*data, output);

    heph_expect_success!(renderer.destroy_buffer(&mut buffer));
    assert_eq!(buffer.size(), 0);
}

fn test_uniform_buffer() {
    const BUFFER_SIZE: usize = 1024;

    let mut data = Vec::with_capacity(BUFFER_SIZE);
    for i in 0..BUFFER_SIZE {
        data.push(i + 1);
    }

    test_buffer(&data, heph_gl::renderers::BufferUsage::Uniform);
}

fn test_storage_buffer() {
    const BUFFER_SIZE: usize = 1024;

    let mut data = Vec::with_capacity(BUFFER_SIZE);
    for i in 0..BUFFER_SIZE {
        data.push(i + 1);
    }

    test_buffer(&data, heph_gl::renderers::BufferUsage::Storage);
}

// TODO: Add tests for other types of buffers.

fn test_shader() {
    let renderer = create_renderer_with_any_device(&[]);

    let shader_source = heph_expect_success!(ShaderSource::from_file(
        SHADERS_DIR.to_owned() + "/addition.spv"
    ));
    let shader = heph_expect_success!(renderer.create_shader(&shader_source));
    heph_expect_success!(renderer.destroy_shader(&shader));
}

fn test_single_threaded_compute(
    target_device_type: heph_gl::graphics_device::Type,
    frames_in_flight: u32,
    n_frames: usize,
) {
    const DATA_COUNT: usize = 256;
    const BYTE_SIZE: usize = DATA_COUNT * std::mem::size_of::<f32>();

    let create_renderer_result = create_renderer_with_target_device(
        target_device_type,
        &[FeatureRequest {
            feature: Feature::ComputeShaders,
            required: false,
        }],
        false,
    );
    assert!(
        create_renderer_result.is_some(),
        "The test should be skipped if device of the target type does not exist."
    );
    let mut renderer = create_renderer_result.unwrap();

    let settings = Settings { frames_in_flight };
    heph_expect_success!(renderer.set_settings(settings));

    let shader_source = heph_expect_success!(ShaderSource::from_file(
        SHADERS_DIR.to_owned() + "/addition.spv"
    ));
    let shader = heph_expect_success!(renderer.create_shader(&shader_source));
    let pipeline = heph_expect_success!(renderer.create_compute_pipeline(&shader));

    let mut buffers = Vec::with_capacity(n_frames);

    for i in 0..n_frames {
        heph_expect_success!(renderer.begin_frame());

        let mut a_data = vec![0.0f32; DATA_COUNT];
        let mut b_data = vec![0.0f32; DATA_COUNT];
        for j in 0..DATA_COUNT {
            a_data[j] = (j * (i + 1)) as f32;
            b_data[j] = ((j * 2) * (i + 1)) as f32;
        }

        let buffer_a =
            heph_expect_success!(renderer.create_buffer(BYTE_SIZE, BufferUsage::Storage));
        let buffer_b =
            heph_expect_success!(renderer.create_buffer(BYTE_SIZE, BufferUsage::Storage));
        let buffer_c =
            heph_expect_success!(renderer.create_buffer(BYTE_SIZE, BufferUsage::Storage));

        heph_expect_success!(renderer.write_buffer(&buffer_a, bytemuck::cast_slice(&a_data)));
        heph_expect_success!(renderer.write_buffer(&buffer_b, bytemuck::cast_slice(&b_data)));

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

        let resource_set = heph_expect_success!(
            renderer.create_resource_set(&PipelineHandle::Compute(pipeline), &bindings)
        );

        let recorded_command = heph_expect_success!(renderer.record_compute_pass(
            &pipeline,
            &[&resource_set],
            (1, 1, 1)
        ));
        heph_expect_success!(renderer.submit_commands(&[recorded_command]));

        heph_expect_success!(renderer.end_frame());

        buffers.push((buffer_a, buffer_b, buffer_c));
    }

    // Wait for all frames to finish.
    heph_expect_success!(renderer.wait_idle());

    for mut buffer in buffers {
        let mut data_a = vec![0.0f32; DATA_COUNT];
        let mut data_b = vec![0.0f32; DATA_COUNT];
        let mut data_c = vec![0.0f32; DATA_COUNT];

        heph_expect_success!(
            renderer.read_buffer(&buffer.0, bytemuck::cast_slice_mut(&mut data_a))
        );
        heph_expect_success!(
            renderer.read_buffer(&buffer.1, bytemuck::cast_slice_mut(&mut data_b))
        );
        heph_expect_success!(
            renderer.read_buffer(&buffer.2, bytemuck::cast_slice_mut(&mut data_c))
        );

        for i in 0..DATA_COUNT {
            assert_eq!(data_a[i] + data_b[i], data_c[i]);
        }

        heph_expect_success!(renderer.destroy_buffer(&mut buffer.0));
        heph_expect_success!(renderer.destroy_buffer(&mut buffer.1));
        heph_expect_success!(renderer.destroy_buffer(&mut buffer.2));
    }

    heph_expect_success!(renderer.destroy_compute_pipeline(&pipeline));
    heph_expect_success!(renderer.destroy_shader(&shader));
}

fn test_single_threaded_compute_discrete_gpu() {
    const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = DiscreteGpu;
    test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
    test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
    test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
}

fn test_single_threaded_compute_integrated_gpu() {
    const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = IntegratedGpu;
    test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
    test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
    test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
}

fn test_single_threaded_compute_cpu() {
    const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = Cpu;
    test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
    test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
    test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
}

fn test_multi_threaded_compute(
    target_device_type: heph_gl::graphics_device::Type,
    n_threads: usize,
) {
    const DATA_COUNT: usize = 256;
    const BYTE_SIZE: usize = DATA_COUNT * std::mem::size_of::<f32>();

    let create_renderer_result = create_renderer_with_target_device(
        target_device_type,
        &[FeatureRequest {
            feature: Feature::ComputeShaders,
            required: false,
        }],
        false,
    );
    assert!(
        create_renderer_result.is_some(),
        "The test should be skipped if device of the target type does not exist."
    );
    let mut renderer = create_renderer_result.unwrap();

    let shader_path = SHADERS_DIR.to_owned() + "/addition.spv";
    let shader_module = heph_expect_success!(
        renderer.create_shader(heph_expect_success!(&ShaderSource::from_file(shader_path)))
    );
    let pipeline = heph_expect_success!(renderer.create_compute_pipeline(&shader_module));

    let renderer_ptr = &mut renderer as *mut VulkanRenderer as usize;

    std::thread::scope(|s| {
        let (tx, rx) = std::sync::mpsc::channel();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(n_threads + 1));

        for thread_id in 0..n_threads {
            let tx = tx.clone();
            let barrier = barrier.clone();

            s.spawn(move || {
                let renderer = unsafe { &mut *(renderer_ptr as *mut VulkanRenderer) };

                heph_expect_success!(renderer.initialize_thread());

                barrier.wait();

                let i = thread_id + 1;

                let mut a_data = vec![0.0f32; DATA_COUNT];
                let mut b_data = vec![0.0f32; DATA_COUNT];
                for j in 0..DATA_COUNT {
                    a_data[j] = (j * i) as f32;
                    b_data[j] = ((j * 2) * i) as f32;
                }

                let buffer_a =
                    heph_expect_success!(renderer.create_buffer(BYTE_SIZE, BufferUsage::Storage));
                let buffer_b =
                    heph_expect_success!(renderer.create_buffer(BYTE_SIZE, BufferUsage::Storage));
                let buffer_c =
                    heph_expect_success!(renderer.create_buffer(BYTE_SIZE, BufferUsage::Storage));

                heph_expect_success!(
                    renderer.write_buffer(&buffer_a, bytemuck::cast_slice(&a_data))
                );
                heph_expect_success!(
                    renderer.write_buffer(&buffer_b, bytemuck::cast_slice(&b_data))
                );

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

                let resource_set = heph_expect_success!(
                    renderer.create_resource_set(&PipelineHandle::Compute(pipeline), &bindings)
                );
                let recorded_command = heph_expect_success!(renderer.record_compute_pass(
                    &pipeline,
                    &[&resource_set],
                    (1, 1, 1)
                ));

                heph_expect_success!(tx.send((
                    recorded_command,
                    buffer_a,
                    buffer_b,
                    buffer_c,
                    a_data,
                    b_data,
                )));

                barrier.wait();

                heph_expect_success!(renderer.uninitialize_thread());
            });
        }

        heph_expect_success!(renderer.begin_frame());

        barrier.wait();

        let mut commands = Vec::with_capacity(n_threads);
        let mut cleanup_data = Vec::with_capacity(n_threads);

        for _ in 0..n_threads {
            let (command, buffer_a, buffer_b, buffer_c, a_data, b_data) =
                heph_expect_success!(rx.recv());
            commands.push(command);
            cleanup_data.push((buffer_a, buffer_b, buffer_c, a_data, b_data));
        }

        heph_expect_success!(renderer.submit_commands(&commands));
        heph_expect_success!(renderer.wait_idle());

        for (mut buffer_a, mut buffer_b, mut buffer_c, a_data, b_data) in cleanup_data {
            let mut c_data = vec![0.0f32; DATA_COUNT];
            heph_expect_success!(
                renderer.read_buffer(&buffer_c, bytemuck::cast_slice_mut(&mut c_data))
            );

            for j in 0..DATA_COUNT {
                assert_eq!(a_data[j] + b_data[j], c_data[j]);
            }

            heph_expect_success!(renderer.destroy_buffer(&mut buffer_a));
            heph_expect_success!(renderer.destroy_buffer(&mut buffer_b));
            heph_expect_success!(renderer.destroy_buffer(&mut buffer_c));
        }

        barrier.wait();

        heph_expect_success!(renderer.end_frame());
    });

    heph_expect_success!(renderer.destroy_compute_pipeline(&pipeline));
    heph_expect_success!(renderer.destroy_shader(&shader_module));
}

fn test_multi_threaded_compute_discrete_gpu() {
    const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = DiscreteGpu;
    test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
}

fn test_multi_threaded_compute_integrated_gpu() {
    const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = IntegratedGpu;
    test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
}

fn test_multi_threaded_compute_cpu() {
    const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = Cpu;
    test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
}

// We use nextest and libtest-mimic to run each test on the main thread of its
// own process. This allows us to avoid "event loop creation in a worker thread"
// errors.
fn main() {
    let args = Arguments::from_args();

    let device_type_exists = |device_type: heph_gl::graphics_device::Type| -> bool {
        let mut renderer = create_renderer();
        let devices = heph_expect_success!(renderer.enumerate_devices());
        devices
            .iter()
            .find(|d| d.device_type == device_type)
            .is_some()
    };
    let skip_discrete_gpu_device_tests = !device_type_exists(DiscreteGpu);
    let skip_integrated_gpu_device_tests = !device_type_exists(IntegratedGpu);
    let skip_cpu_device_tests = !device_type_exists(Cpu);

    let tests = vec![
        Trial::test("test_initialize_renderer", move || {
            test_initialize_renderer();
            Ok(())
        }),
        Trial::test("test_enumerate_devices", move || {
            test_enumerate_devices();
            Ok(())
        }),
        Trial::test("test_set_device", move || {
            test_set_device();
            Ok(())
        }),
        Trial::test("test_set_settings", move || {
            test_set_settings();
            Ok(())
        }),
        Trial::test("test_uniform_buffer", move || {
            test_uniform_buffer();
            Ok(())
        }),
        Trial::test("test_storage_buffer", move || {
            test_storage_buffer();
            Ok(())
        }),
        Trial::test("test_shader", move || {
            test_shader();
            Ok(())
        }),
        Trial::test("test_single_threaded_compute_discrete_gpu", move || {
            test_single_threaded_compute_discrete_gpu();
            Ok(())
        })
        .with_ignored_flag(skip_discrete_gpu_device_tests),
        Trial::test("test_single_threaded_compute_integrated_gpu", move || {
            test_single_threaded_compute_integrated_gpu();
            Ok(())
        })
        .with_ignored_flag(skip_integrated_gpu_device_tests),
        Trial::test("test_single_threaded_compute_cpu", move || {
            test_single_threaded_compute_cpu();
            Ok(())
        })
        .with_ignored_flag(skip_cpu_device_tests),
        Trial::test("test_multi_threaded_compute_discrete_gpu", move || {
            test_multi_threaded_compute_discrete_gpu();
            Ok(())
        })
        .with_ignored_flag(skip_discrete_gpu_device_tests),
        Trial::test("test_multi_threaded_compute_integrated_gpu", move || {
            test_multi_threaded_compute_integrated_gpu();
            Ok(())
        })
        .with_ignored_flag(skip_integrated_gpu_device_tests),
        Trial::test("test_multi_threaded_compute_cpu", move || {
            test_multi_threaded_compute_cpu();
            Ok(())
        })
        .with_ignored_flag(skip_cpu_device_tests),
    ];

    libtest_mimic::run(&args, tests).exit();
}
