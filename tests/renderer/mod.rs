use std::{
    collections::HashSet,
    fmt::Debug,
    marker::PhantomData,
    process::ExitCode,
    sync::{LazyLock, Mutex},
};

use heph_gl::{
    graphics_device::{
        Feature, GraphicsDevice,
        Type::{Cpu, DiscreteGpu, IntegratedGpu, VirtualGpu},
    },
    renderers::{
        BufferUsage, FeatureRequest, GpuBuffer, InitializeOptions, PipelineHandle, Renderer,
        RendererError, ResourceBinding, ResourceBindingType, Settings,
    },
    shader::ShaderSource,
};
use libtest_mimic::{Arguments, Trial};

use crate::utils::{SHADERS_DIR, TestEnv};
use crate::{heph_expect_err, heph_expect_success};

pub static TEST_ENV: LazyLock<Mutex<TestEnv>> = LazyLock::new(|| Mutex::new(TestEnv::default()));
macro_rules! test_env {
    () => {
        TEST_ENV.lock().unwrap()
    };
}

pub struct RendererTests<TestRenderer: Renderer> {
    _marker: PhantomData<TestRenderer>,
}

impl<TestRenderer: Renderer> RendererTests<TestRenderer> {
    // We use nextest and libtest-mimic to run each test on the main thread of its
    // own process. This allows us to avoid "event loop creation in a worker thread"
    // errors.
    pub fn run() -> ExitCode {
        if std::env::var("NEXTEST").is_err() {
            println!("Skipping integration tests, run via `cargo nextest run` instead.");
            return ExitCode::SUCCESS;
        }

        let args = Arguments::from_args();

        let device_type_exists = |device_type: heph_gl::graphics_device::Type| -> bool {
            let renderer = Self::create_renderer();
            let devices = heph_expect_success!(renderer.enumerate_devices());
            devices
                .iter()
                .find(|d| d.device_type == device_type)
                .is_some()
        };
        let skip_discrete_gpu_device_tests = !device_type_exists(DiscreteGpu);
        let skip_integrated_gpu_device_tests = !device_type_exists(IntegratedGpu);
        let skip_cpu_device_tests = !device_type_exists(Cpu);
        let skip_virtual_gpu_device_tests = !device_type_exists(VirtualGpu);
        let skip_other_device_tests = !device_type_exists(heph_gl::graphics_device::Type::Other);

        let tests = vec![
            Trial::test("test_invalid_app_name", move || {
                Self::test_invalid_app_name();
                Ok(())
            }),
            Trial::test("test_initialize_renderer", move || {
                Self::test_initialize_renderer();
                Ok(())
            }),
            Trial::test("test_enumerate_devices", move || {
                Self::test_enumerate_devices();
                Ok(())
            }),
            Trial::test("test_set_device", move || {
                Self::test_set_device();
                Ok(())
            }),
            Trial::test("test_set_settings", move || {
                Self::test_set_settings();
                Ok(())
            }),
            Trial::test("test_uniform_buffer", move || {
                Self::test_uniform_buffer();
                Ok(())
            }),
            Trial::test("test_storage_buffer", move || {
                Self::test_storage_buffer();
                Ok(())
            }),
            Trial::test("test_index_buffer", move || {
                Self::test_index_buffer();
                Ok(())
            }),
            Trial::test("test_vertex_buffer", move || {
                Self::test_vertex_buffer();
                Ok(())
            }),
            Trial::test("test_impossible_buffer_size", move || {
                Self::test_impossible_buffer_size();
                Ok(())
            }),
            Trial::test("test_shader", move || {
                Self::test_shader();
                Ok(())
            }),
            Trial::test(
                "test_calling_main_thread_only_fn_from_worker_thread",
                move || {
                    Self::test_calling_main_thread_only_fn_from_worker_thread();
                    Ok(())
                },
            ),
            Trial::test("test_single_threaded_compute_discrete_gpu", move || {
                Self::test_single_threaded_compute_discrete_gpu();
                Ok(())
            })
            .with_ignored_flag(skip_discrete_gpu_device_tests),
            Trial::test("test_single_threaded_compute_integrated_gpu", move || {
                Self::test_single_threaded_compute_integrated_gpu();
                Ok(())
            })
            .with_ignored_flag(skip_integrated_gpu_device_tests),
            Trial::test("test_single_threaded_compute_cpu", move || {
                Self::test_single_threaded_compute_cpu();
                Ok(())
            })
            .with_ignored_flag(skip_cpu_device_tests),
            Trial::test("test_single_threaded_compute_virtual_gpu", move || {
                Self::test_single_threaded_compute_virtual_gpu();
                Ok(())
            })
            .with_ignored_flag(skip_virtual_gpu_device_tests),
            Trial::test("test_single_threaded_compute_other", move || {
                Self::test_single_threaded_compute_other();
                Ok(())
            })
            .with_ignored_flag(skip_other_device_tests),
            Trial::test("test_multi_threaded_compute_discrete_gpu", move || {
                Self::test_multi_threaded_compute_discrete_gpu();
                Ok(())
            })
            .with_ignored_flag(skip_discrete_gpu_device_tests),
            Trial::test("test_multi_threaded_compute_integrated_gpu", move || {
                Self::test_multi_threaded_compute_integrated_gpu();
                Ok(())
            })
            .with_ignored_flag(skip_integrated_gpu_device_tests),
            Trial::test("test_multi_threaded_compute_cpu", move || {
                Self::test_multi_threaded_compute_cpu();
                Ok(())
            })
            .with_ignored_flag(skip_cpu_device_tests),
            Trial::test("test_multi_threaded_compute_virtual_gpu", move || {
                Self::test_multi_threaded_compute_virtual_gpu();
                Ok(())
            })
            .with_ignored_flag(skip_virtual_gpu_device_tests),
            Trial::test("test_multi_threaded_compute_other", move || {
                Self::test_multi_threaded_compute_other();
                Ok(())
            })
            .with_ignored_flag(skip_other_device_tests),
        ];

        libtest_mimic::run(&args, tests).exit_code()
    }

    fn create_renderer() -> TestRenderer {
        let test_env = test_env!();
        let app_name = std::any::type_name::<TestRenderer>().to_string() + " Tests";
        let mut renderer = TestRenderer::new();
        heph_expect_success!(renderer.initialize(&InitializeOptions {
            app_name: app_name.as_str(),
            window_handle: test_env.raw_window_handle(),
            display_handle: test_env.raw_display_handle(),
        }));
        renderer
    }

    fn create_renderer_with_target_device(
        target_device_type: heph_gl::graphics_device::Type,
        requested_features: &[FeatureRequest],
        target_device_type_required: bool,
    ) -> Option<TestRenderer> {
        let mut renderer = Self::create_renderer();
        let devices = heph_expect_success!(renderer.enumerate_devices());
        let target_device = devices.iter().find(|d| d.device_type == target_device_type);
        if target_device.is_none() {
            if target_device_type_required {
                panic!("Invalid Test Env: No {} found.", target_device_type);
            } else {
                return None;
            }
        }
        heph_expect_success!(renderer.set_device(target_device, requested_features));

        let result = renderer.get_device();
        assert!(
            result.is_some(),
            "Renderer successfully set the graphics device, but `get_device()` returned `None`."
        );
        assert_eq!(target_device.unwrap(), result.unwrap());

        Some(renderer)
    }

    fn create_renderer_with_any_device(requested_features: &[FeatureRequest]) -> TestRenderer {
        let mut renderer = Self::create_renderer();
        let devices = heph_expect_success!(renderer.enumerate_devices());
        assert!(!devices.is_empty(), "Invalid Test Env: No device found.",);
        heph_expect_success!(renderer.set_device(None, requested_features));

        let result = renderer.get_device();
        assert!(
            result.is_some(),
            "Renderer successfully set the graphics device, but `get_device()` returned `None`."
        );
        assert_eq!(result.unwrap(), devices.first().unwrap());

        renderer
    }

    fn test_invalid_app_name() {
        let test_env = test_env!();
        let mut renderer = TestRenderer::new();
        heph_expect_err!(renderer.initialize(&InitializeOptions {
            app_name: "\0",
            window_handle: test_env.raw_window_handle(),
            display_handle: test_env.raw_display_handle(),
        }));
    }

    fn test_initialize_renderer() {
        let test_env = test_env!();
        let mut renderer = TestRenderer::new();
        let init_options = InitializeOptions {
            app_name: "",
            window_handle: test_env.raw_window_handle(),
            display_handle: test_env.raw_display_handle(),
        };
        heph_expect_success!(renderer.initialize(&init_options));
        heph_expect_err!(
            renderer.initialize(&init_options),
            RendererError::InvalidOperation("".to_string())
        );
        heph_expect_success!(renderer.uninitialize());
    }

    fn test_enumerate_devices() {
        let renderer = Self::create_renderer();
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
        let mut features = [
            FeatureRequest {
                feature: Feature::RayTracing,
                required: false,
            },
            FeatureRequest {
                feature: Feature::OpticalFlow,
                required: false,
            },
            FeatureRequest {
                feature: Feature::VideoDecoding,
                required: false,
            },
            FeatureRequest {
                feature: Feature::VideoEncoding,
                required: false,
            },
        ];
        let mut renderer = Self::create_renderer_with_any_device(&features);
        assert!(renderer.get_device().is_some());

        // Test invalid device.
        heph_expect_err!(
            renderer.set_device(
                Some(&GraphicsDevice {
                    name: "".to_string(),
                    device_type: heph_gl::graphics_device::Type::Other,
                    vendor_id: u32::MAX,
                    device_id: u32::MAX,
                    api_version: heph_gl::Version {
                        major: 0,
                        minor: 0,
                        patch: 0,
                    },
                    driver_version: heph_gl::Version {
                        major: 0,
                        minor: 0,
                        patch: 0,
                    },
                    vram: 0,
                    supported_features: HashSet::default(),
                }),
                &features
            ),
            RendererError::Fail("".to_string())
        );

        for feature in &mut features {
            feature.required = true;
        }
        heph_expect_err!(
            renderer.set_device(None, &features),
            RendererError::UnsupportedRequiredFeature(Feature::RayTracing)
        );
    }

    fn test_set_settings() {
        let settings = Settings {
            frames_in_flight: 10,
        };

        {
            let mut renderer = Self::create_renderer();
            heph_expect_success!(renderer.set_settings(settings));
            let result = renderer.get_settings();
            assert_eq!(settings.frames_in_flight, result.frames_in_flight);
        }

        {
            let mut renderer = Self::create_renderer_with_any_device(&[]);
            heph_expect_success!(renderer.set_settings(settings));
            let result = renderer.get_settings();
            assert_eq!(settings.frames_in_flight, result.frames_in_flight);
        }

        {
            let mut renderer = Self::create_renderer_with_any_device(&[]);
            std::thread::scope(|s| {
                let p_renderer = &mut renderer as *mut TestRenderer as usize;
                let b1 = std::sync::Arc::new(std::sync::Barrier::new(2));
                let b2 = b1.clone();
                s.spawn(move || {
                    let renderer = unsafe { &mut *(p_renderer as *mut TestRenderer) };
                    heph_expect_success!(renderer.initialize_thread());
                    b2.wait();
                    b2.wait();
                    heph_expect_success!(renderer.uninitialize_thread());
                });
                b1.wait();
                heph_expect_err!(
                    renderer.set_settings(settings),
                    RendererError::InvalidOperation("".to_string())
                );
                b1.wait();
            });
        }
    }

    fn test_buffer<T>(data: &Vec<T>, usage: heph_gl::renderers::BufferUsage)
    where
        T: bytemuck::Pod + PartialEq + Default + Debug,
    {
        let renderer = Self::create_renderer_with_any_device(&[]);

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

        Self::test_buffer(&data, BufferUsage::Uniform);
    }

    fn test_storage_buffer() {
        const BUFFER_SIZE: usize = 1024;

        let mut data = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            data.push(i + 1);
        }

        Self::test_buffer(&data, BufferUsage::Storage);
    }

    fn test_index_buffer() {
        const BUFFER_SIZE: usize = 1024;

        let mut data = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            data.push(i + 1);
        }

        Self::test_buffer(&data, BufferUsage::Index);
    }

    fn test_vertex_buffer() {
        const BUFFER_SIZE: usize = 1024;

        let mut data = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            data.push(i + 1);
        }

        Self::test_buffer(&data, BufferUsage::Vertex);
    }

    fn test_impossible_buffer_size() {
        let renderer = Self::create_renderer_with_any_device(&[]);
        let buffer_size = 1024 * 1024 * 1024 * 1024; // 1 TB
        heph_expect_err!(renderer.create_buffer(buffer_size, BufferUsage::Storage));
    }

    fn test_shader() {
        let renderer = Self::create_renderer_with_any_device(&[]);

        let shader_source = heph_expect_success!(ShaderSource::from_file(
            SHADERS_DIR.to_owned() + "/addition.spv"
        ));
        let shader = heph_expect_success!(renderer.create_shader(&shader_source));
        heph_expect_success!(renderer.destroy_shader(&shader));
    }

    fn test_calling_main_thread_only_fn_from_worker_thread() {
        let mut renderer = Self::create_renderer_with_any_device(&[]);
        std::thread::scope(|s| {
            let p_renderer = &mut renderer as *mut TestRenderer as usize;
            s.spawn(move || {
                let renderer = unsafe { &mut *(p_renderer as *mut TestRenderer) };
                heph_expect_err!(
                    renderer.set_device(None, &[]),
                    RendererError::InvalidOperation("".to_string())
                );
            });
        });
    }

    fn test_single_threaded_compute(
        target_device_type: heph_gl::graphics_device::Type,
        frames_in_flight: u32,
        n_frames: usize,
    ) {
        const DATA_COUNT: usize = 256;
        const BYTE_SIZE: usize = DATA_COUNT * std::mem::size_of::<f32>();

        let create_renderer_result = Self::create_renderer_with_target_device(
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
        Self::test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
        Self::test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
        Self::test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
    }

    fn test_single_threaded_compute_integrated_gpu() {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = IntegratedGpu;
        Self::test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
        Self::test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
        Self::test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
    }

    fn test_single_threaded_compute_cpu() {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = Cpu;
        Self::test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
        Self::test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
        Self::test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
    }

    fn test_single_threaded_compute_virtual_gpu() {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = VirtualGpu;
        Self::test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
        Self::test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
        Self::test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
    }

    fn test_single_threaded_compute_other() {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type =
            heph_gl::graphics_device::Type::Other;
        Self::test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
        Self::test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
        Self::test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
    }

    fn test_multi_threaded_compute(
        target_device_type: heph_gl::graphics_device::Type,
        n_threads: usize,
    ) {
        const DATA_COUNT: usize = 256;
        const BYTE_SIZE: usize = DATA_COUNT * std::mem::size_of::<f32>();

        let create_renderer_result = Self::create_renderer_with_target_device(
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

        let renderer_ptr = &mut renderer as *mut TestRenderer as usize;

        std::thread::scope(|s| {
            let (tx, rx) = std::sync::mpsc::channel();
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(n_threads + 1));

            for thread_id in 0..n_threads {
                let tx = tx.clone();
                let barrier = barrier.clone();

                s.spawn(move || {
                    let renderer = unsafe { &mut *(renderer_ptr as *mut TestRenderer) };

                    heph_expect_success!(renderer.initialize_thread());

                    barrier.wait();

                    let i = thread_id + 1;

                    let mut a_data = vec![0.0f32; DATA_COUNT];
                    let mut b_data = vec![0.0f32; DATA_COUNT];
                    for j in 0..DATA_COUNT {
                        a_data[j] = (j * i) as f32;
                        b_data[j] = ((j * 2) * i) as f32;
                    }

                    let buffer_a = heph_expect_success!(
                        renderer.create_buffer(BYTE_SIZE, BufferUsage::Storage)
                    );
                    let buffer_b = heph_expect_success!(
                        renderer.create_buffer(BYTE_SIZE, BufferUsage::Storage)
                    );
                    let buffer_c = heph_expect_success!(
                        renderer.create_buffer(BYTE_SIZE, BufferUsage::Storage)
                    );

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
        Self::test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
    }

    fn test_multi_threaded_compute_integrated_gpu() {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = IntegratedGpu;
        Self::test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
    }

    fn test_multi_threaded_compute_cpu() {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = Cpu;
        Self::test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
    }

    fn test_multi_threaded_compute_virtual_gpu() {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = VirtualGpu;
        Self::test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
    }

    fn test_multi_threaded_compute_other() {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type =
            heph_gl::graphics_device::Type::Other;
        Self::test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
    }
}
