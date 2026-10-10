use std::{
    collections::HashSet,
    fmt::Debug,
    marker::PhantomData,
    process::ExitCode,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use heph_gl::{
    Version,
    graphics_device::{
        Feature::{self, ComputeShaders, OpticalFlow, RayTracing, VideoDecoding, VideoEncoding},
        GraphicsDevice,
        Type::{Cpu, DiscreteGpu, IntegratedGpu, VirtualGpu},
    },
    renderers::{
        Renderer,
        concurrency::{RendererHandle, RendererWorkerFactory},
        error::RendererError,
        resources::{BufferUsage, GpuBuffer, ResourceBinding, ResourceBindingType, TextureFormat},
        settings::{
            ColorBlending, CullingMode, FeatureRequest, FrontFace, GraphicsPipelineOptions,
            InitializeOptions, PrimitiveTopology, SamplerOptions, Settings, TextureOptions,
        },
    },
    shader::Shader,
};
use libtest_mimic::{Arguments, Trial};
use renkrs::RGB;

use crate::utils::{SHADERS_DIR, TestEnv};
use crate::{heph_expect_err, heph_expect_success};

pub static TEST_ENV: LazyLock<Mutex<TestEnv>> = LazyLock::new(|| Mutex::new(TestEnv::default()));
macro_rules! test_env {
    () => {
        TEST_ENV.lock().unwrap()
    };
}

macro_rules! dummy_window_handle {
    () => {
        raw_window_handle::RawWindowHandle::Orbital(raw_window_handle::OrbitalWindowHandle::new(
            std::ptr::NonNull::<std::ffi::c_void>::dangling(),
        ))
    };
}
macro_rules! dummy_display_handle {
    () => {
        raw_window_handle::RawDisplayHandle::Orbital(raw_window_handle::OrbitalDisplayHandle::new())
    };
}

macro_rules! define_renderer_test_flags {
    (
        $($tfn:ident),* $(,)?
    ) => {
        paste::paste! {
            #[derive(Default, Clone, Copy)]
            pub struct RendererTestFlags {
                /// Skips all tests.
                pub skip_all_tests: bool,

                $(
                    /// Skips testing this feature.
                    pub [<skip_ $tfn>]: bool,
                    /// Indicates whether the feature is not yet implemented, and tests should fail
                    /// with a `todo!()`.
                    pub [<todo_ $tfn>]: bool,
                    /// Indicates whether the feature is not supported by this renderer, and tests
                    /// should fail with an `unimplemented!()`.
                    pub [<unimplemented_ $tfn>]: bool,
                )*
            }
        }
    };
}

define_renderer_test_flags!(
    test_invalid_app_name,
    test_initialize_renderer,
    test_enumerate_devices,
    test_set_device,
    test_set_settings,
    test_uniform_buffer,
    test_storage_buffer,
    test_index_buffer,
    test_vertex_buffer,
    test_impossible_buffer_size,
    test_begin_twice,
    test_end_without_begin,
    test_calling_main_thread_only_fn_from_worker_thread,
    test_multiple_workers_per_thread,
    test_excess_threads,
    test_single_threaded_compute_discrete_gpu,
    test_single_threaded_compute_integrated_gpu,
    test_single_threaded_compute_cpu,
    test_single_threaded_compute_virtual_gpu,
    test_single_threaded_compute_other,
    test_multi_threaded_compute_discrete_gpu,
    test_multi_threaded_compute_integrated_gpu,
    test_multi_threaded_compute_cpu,
    test_multi_threaded_compute_virtual_gpu,
    test_multi_threaded_compute_other,
    test_render_static_shapes,
    test_render_dynamic_cubes,
    test_render_textures
);

#[derive(Clone, Copy)]
pub struct RendererVersionedTestSuite {
    /// The API version the renderer will use for the current test suite.
    pub api_version: Option<Version>,
    /// The flags of the current test suite.
    pub flags: RendererTestFlags,
}

pub struct RendererTests<TestRenderer: Renderer> {
    api_version: Option<Version>,
    _marker: PhantomData<TestRenderer>,
}

impl RendererVersionedTestSuite {
    fn version_string(&self) -> String {
        if let Some(api_version) = self.api_version {
            api_version.to_string().to_uppercase()
        } else {
            "LATEST VERSION".to_string()
        }
    }
}

impl<TestRenderer: Renderer> RendererTests<TestRenderer>
where
    RendererHandle<TestRenderer>: RendererWorkerFactory<TestRenderer>,
{
    fn new(api_version: Option<Version>) -> Self {
        Self {
            api_version,
            _marker: PhantomData,
        }
    }

    // We use nextest and libtest-mimic to run each test on the main thread of its
    // own process. This allows us to avoid "event loop creation in a worker thread"
    // errors.
    pub fn run(versioned_test_suites: &[RendererVersionedTestSuite]) -> ExitCode {
        if std::env::var("NEXTEST").is_err() {
            println!("Skipping integration tests, run via `cargo nextest run` instead.");
            return ExitCode::SUCCESS;
        }

        let args = Arguments::from_args();

        let device_type_exists = |device_type: heph_gl::graphics_device::Type| -> bool {
            let renderer = Self::new(TestRenderer::LATEST_API_VERSION).create_renderer();
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

        macro_rules! create_trial {
            ($versioned_test_suite:ident, $tfn:ident) => {
                create_trial!($versioned_test_suite, $tfn, false)
            };
            ($versioned_test_suite:ident, $tfn:ident, $skip:expr) => {
                paste::paste! {
                    {
                        let versioned_test_suite = $versioned_test_suite.clone();
                        let test_name = format!(
                            "[{}] {}",
                            versioned_test_suite.version_string(),
                            stringify!($tfn)
                        );
                        Trial::test(test_name, move || {
                            let result = std::panic::catch_unwind(|| {
                                Self::new(versioned_test_suite.api_version).$tfn();
                            });

                            let todo = versioned_test_suite.flags.[<todo_ $tfn>];
                            let unimplemented = versioned_test_suite.flags.[<unimplemented_ $tfn>];
                            if todo || unimplemented {
                                assert!(
                                    result.is_err(),
                                    "{} was expected to panic",
                                    stringify!($tfn)
                                );
                                let panic = result.err().unwrap();
                                let panic_msg = panic.downcast_ref::<&str>()
                                                    .copied()
                                                    .or_else(||
                                                        panic
                                                        .downcast_ref::<String>()
                                                        .map(String::as_str)
                                                    ).unwrap_or("");
                                if todo {
                                    assert!(
                                        panic_msg.contains("not yet implemented"),
                                        "unexpected panic: {panic_msg}"
                                    );
                                }
                                else if unimplemented {
                                    assert!(
                                        panic_msg.contains("not implemented"),
                                        "unexpected panic: {panic_msg}"
                                    );
                                } else {
                                    panic!("Unhandled renderer test setting flag");
                                }
                            } else {
                                if let Err(err) = result {
                                    std::panic::resume_unwind(err);
                                }
                            }

                            Ok(())
                        })
                        .with_ignored_flag(
                            versioned_test_suite.flags.skip_all_tests ||
                            versioned_test_suite.flags.[<skip_ $tfn>] ||
                            $skip
                        )
                    }
                }
            };
        }

        let renderer = TestRenderer::new();
        let latest_api_version = renderer
            .latest_api_version()
            .unwrap_or(TestRenderer::MIN_SUPPORTED_API_VERSION);
        let mut tests = Vec::new();
        let mut api_versions = Vec::new();
        for mut versioned_test_suite in versioned_test_suites.iter().copied() {
            let api_version = match versioned_test_suite.api_version {
                Some(api_version) => Version::new(api_version.major, api_version.minor, 0),
                None => Version::new(latest_api_version.major, latest_api_version.minor, 0),
            };
            if api_versions.contains(&api_version) {
                continue;
            }
            api_versions.push(api_version);
            if !versioned_test_suite.flags.skip_all_tests {
                versioned_test_suite.flags.skip_all_tests = !renderer
                    .is_api_version_supported(api_version)
                    .unwrap_or(false);
            }

            let mut test_suite = vec![
                create_trial!(versioned_test_suite, test_invalid_app_name),
                create_trial!(versioned_test_suite, test_initialize_renderer),
                create_trial!(versioned_test_suite, test_enumerate_devices),
                create_trial!(versioned_test_suite, test_set_device),
                create_trial!(versioned_test_suite, test_set_settings),
                create_trial!(versioned_test_suite, test_uniform_buffer),
                create_trial!(versioned_test_suite, test_storage_buffer),
                create_trial!(versioned_test_suite, test_index_buffer),
                create_trial!(versioned_test_suite, test_vertex_buffer),
                create_trial!(versioned_test_suite, test_impossible_buffer_size),
                create_trial!(versioned_test_suite, test_begin_twice),
                create_trial!(versioned_test_suite, test_end_without_begin),
                create_trial!(
                    versioned_test_suite,
                    test_calling_main_thread_only_fn_from_worker_thread
                ),
                create_trial!(versioned_test_suite, test_multiple_workers_per_thread),
                create_trial!(versioned_test_suite, test_excess_threads),
                create_trial!(
                    versioned_test_suite,
                    test_single_threaded_compute_discrete_gpu,
                    skip_discrete_gpu_device_tests
                ),
                create_trial!(
                    versioned_test_suite,
                    test_single_threaded_compute_integrated_gpu,
                    skip_integrated_gpu_device_tests
                ),
                create_trial!(
                    versioned_test_suite,
                    test_single_threaded_compute_cpu,
                    skip_cpu_device_tests
                ),
                create_trial!(
                    versioned_test_suite,
                    test_single_threaded_compute_virtual_gpu,
                    skip_virtual_gpu_device_tests
                ),
                create_trial!(
                    versioned_test_suite,
                    test_single_threaded_compute_other,
                    skip_other_device_tests
                ),
                create_trial!(
                    versioned_test_suite,
                    test_multi_threaded_compute_discrete_gpu,
                    skip_discrete_gpu_device_tests
                ),
                create_trial!(
                    versioned_test_suite,
                    test_multi_threaded_compute_integrated_gpu,
                    skip_integrated_gpu_device_tests
                ),
                create_trial!(
                    versioned_test_suite,
                    test_multi_threaded_compute_cpu,
                    skip_cpu_device_tests
                ),
                create_trial!(
                    versioned_test_suite,
                    test_multi_threaded_compute_virtual_gpu,
                    skip_virtual_gpu_device_tests
                ),
                create_trial!(
                    versioned_test_suite,
                    test_multi_threaded_compute_other,
                    skip_other_device_tests
                ),
                create_trial!(versioned_test_suite, test_render_static_shapes),
                create_trial!(versioned_test_suite, test_render_dynamic_cubes),
                create_trial!(versioned_test_suite, test_render_textures),
            ];
            tests.append(&mut test_suite);
        }
        drop(renderer);

        libtest_mimic::run(&args, tests).exit_code()
    }

    fn create_renderer(&self) -> TestRenderer {
        let test_env = test_env!();
        let app_name = format!("{} Tests", std::any::type_name::<TestRenderer>());
        let mut renderer = TestRenderer::new();
        heph_expect_success!(renderer.initialize(&InitializeOptions {
            app_name: app_name.as_str(),
            window_handle: test_env.raw_window_handle(),
            display_handle: test_env.raw_display_handle(),
            api_version: self.api_version,
        }));
        renderer
    }

    fn create_renderer_with_target_device(
        &self,
        target_device_type: heph_gl::graphics_device::Type,
        requested_features: &[FeatureRequest],
        target_device_type_required: bool,
    ) -> Option<TestRenderer> {
        let mut renderer = self.create_renderer();
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

    fn create_renderer_with_any_device(
        &self,
        requested_features: &[FeatureRequest],
    ) -> TestRenderer {
        let mut renderer = self.create_renderer();
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

    fn test_invalid_app_name(&self) {
        let test_env = test_env!();
        let mut renderer = TestRenderer::new();
        heph_expect_err!(renderer.initialize(&InitializeOptions {
            app_name: "\0",
            window_handle: test_env.raw_window_handle(),
            display_handle: test_env.raw_display_handle(),
            api_version: self.api_version,
        }));
    }

    fn test_initialize_renderer(&self) {
        {
            let test_env = test_env!();
            let mut renderer = TestRenderer::new();
            let init_options = InitializeOptions {
                app_name: "",
                window_handle: test_env.raw_window_handle(),
                display_handle: test_env.raw_display_handle(),
                api_version: self.api_version,
            };
            heph_expect_success!(renderer.initialize(&init_options));
            heph_expect_err!(
                renderer.initialize(&init_options),
                RendererError::invalid_operation("")
            );
            heph_expect_success!(renderer.uninitialize());
        }

        {
            let test_env = test_env!();
            let mut renderer = TestRenderer::new();
            let init_options = InitializeOptions {
                app_name: "",
                window_handle: test_env.raw_window_handle(),
                display_handle: test_env.raw_display_handle(),
                api_version: Some(Version::new(u32::MIN, u32::MIN, u32::MIN)),
            };
            heph_expect_err!(
                renderer.initialize(&init_options),
                RendererError::invalid_argument("")
            );
        }

        {
            let test_env = test_env!();
            let mut renderer = TestRenderer::new();
            let init_options = InitializeOptions {
                app_name: "",
                window_handle: test_env.raw_window_handle(),
                display_handle: test_env.raw_display_handle(),
                api_version: Some(Version::new(u32::MAX, u32::MAX, u32::MAX)),
            };
            heph_expect_err!(
                renderer.initialize(&init_options),
                RendererError::invalid_argument("")
            );
        }

        {
            let test_env = test_env!();
            let mut renderer = TestRenderer::new();
            let init_options = InitializeOptions {
                app_name: "",
                window_handle: test_env.raw_window_handle(),
                display_handle: dummy_display_handle!(),
                api_version: self.api_version,
            };
            heph_expect_err!(renderer.initialize(&init_options));
        }

        {
            let test_env = test_env!();
            let mut renderer = TestRenderer::new();
            let init_options = InitializeOptions {
                app_name: "",
                window_handle: dummy_window_handle!(),
                display_handle: test_env.raw_display_handle(),
                api_version: self.api_version,
            };
            heph_expect_err!(renderer.initialize(&init_options));
        }

        {
            let mut renderer = TestRenderer::new();
            let init_options = InitializeOptions {
                app_name: "",
                window_handle: dummy_window_handle!(),
                display_handle: dummy_display_handle!(),
                api_version: self.api_version,
            };
            heph_expect_err!(renderer.initialize(&init_options));
        }
    }

    fn test_enumerate_devices(&self) {
        let renderer = self.create_renderer();
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

    fn test_set_device(&self) {
        let mut features = [
            FeatureRequest::new(RayTracing, false),
            FeatureRequest::new(OpticalFlow, false),
            FeatureRequest::new(VideoDecoding, false),
            FeatureRequest::new(VideoEncoding, false),
        ];
        let mut renderer = self.create_renderer_with_any_device(&features);
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
            RendererError::fail("")
        );

        for feature in &mut features {
            feature.required = true;
        }
        heph_expect_err!(
            renderer.set_device(None, &features),
            RendererError::UnsupportedRequiredFeature(Feature::RayTracing)
        );
    }

    fn test_set_settings(&self) {
        let settings = Settings {
            frames_in_flight: 2,
            ..Default::default()
        };

        {
            let mut renderer = self.create_renderer();
            heph_expect_success!(renderer.set_settings(settings));
            let result = renderer.get_settings();
            assert_eq!(settings.frames_in_flight, result.frames_in_flight);
        }

        {
            let mut renderer = self.create_renderer_with_any_device(&[]);
            heph_expect_success!(renderer.set_settings(settings));
            let result = renderer.get_settings();
            assert_eq!(settings.frames_in_flight, result.frames_in_flight);
        }

        {
            let mut renderer = self.create_renderer_with_any_device(&[]);
            std::thread::scope(|s| {
                let b1 = std::sync::Arc::new(std::sync::Barrier::new(2));
                let b2 = b1.clone();
                let renderer_handle = RendererHandle::<TestRenderer>::from(&mut renderer);
                s.spawn(move || {
                    let _renderer_worker = heph_expect_success!(renderer_handle.spawn_worker());
                    b2.wait();
                    b2.wait();
                });
                b1.wait();
                heph_expect_err!(
                    renderer.set_settings(settings),
                    RendererError::invalid_operation("")
                );
                b1.wait();
            });
        }
    }

    fn test_buffer<T>(&self, data: &Vec<T>, usage: BufferUsage)
    where
        T: bytemuck::Pod + PartialEq + Default + Debug,
    {
        let renderer = self.create_renderer_with_any_device(&[]);

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

    fn test_uniform_buffer(&self) {
        const BUFFER_SIZE: usize = 1024;

        let mut data = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            data.push(i + 1);
        }

        self.test_buffer(&data, BufferUsage::Uniform);
    }

    fn test_storage_buffer(&self) {
        const BUFFER_SIZE: usize = 1024;

        let mut data = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            data.push(i + 1);
        }

        self.test_buffer(&data, BufferUsage::Storage);
    }

    fn test_index_buffer(&self) {
        const BUFFER_SIZE: usize = 1024;

        let mut data = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            data.push(i + 1);
        }

        self.test_buffer(&data, BufferUsage::Index);
    }

    fn test_vertex_buffer(&self) {
        const BUFFER_SIZE: usize = 1024;

        let mut data = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            data.push(i + 1);
        }

        self.test_buffer(&data, BufferUsage::Vertex);
    }

    fn test_impossible_buffer_size(&self) {
        let renderer = self.create_renderer_with_any_device(&[]);
        let buffer_size = 1024 * 1024 * 1024 * 1024; // 1 TB
        heph_expect_err!(renderer.create_buffer(buffer_size, BufferUsage::Storage));
    }

    fn test_begin_twice(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);
        heph_expect_success!(renderer.begin_frame());
        heph_expect_err!(renderer.begin_frame(), RendererError::invalid_operation(""));
    }

    fn test_end_without_begin(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);
        heph_expect_err!(renderer.end_frame(), RendererError::invalid_operation(""));
    }

    fn test_calling_main_thread_only_fn_from_worker_thread(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);
        let renderer_handle = RendererHandle::<TestRenderer>::from(&mut renderer);
        std::thread::scope(|s| {
            s.spawn(move || {
                let mut renderer_worker = heph_expect_success!(renderer_handle.spawn_worker());
                heph_expect_err!(
                    renderer_worker.set_settings(Settings::default()),
                    RendererError::invalid_operation("")
                );
                heph_expect_err!(
                    renderer_worker.initialize(&InitializeOptions {
                        app_name: "",
                        window_handle: dummy_window_handle!(),
                        display_handle: dummy_display_handle!(),
                        api_version: TestRenderer::LATEST_API_VERSION,
                    }),
                    RendererError::invalid_operation("")
                );
                heph_expect_err!(
                    renderer_worker.uninitialize(),
                    RendererError::invalid_operation("")
                );
                heph_expect_err!(
                    renderer_worker.set_device(None, &[]),
                    RendererError::invalid_operation("")
                );
                heph_expect_err!(
                    renderer_worker.wait_idle(),
                    RendererError::invalid_operation("")
                );
            });
        });
    }

    fn test_multiple_workers_per_thread(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);
        let renderer_handle = RendererHandle::<TestRenderer>::from(&mut renderer);

        std::thread::scope(|s| {
            s.spawn(move || {
                // We can spawn another worker after the first one is dropped.
                heph_expect_success!(renderer_handle.spawn_worker());
                heph_expect_success!(renderer_handle.spawn_worker());
            });
        });

        std::thread::scope(|s| {
            s.spawn(move || {
                let _w1 = heph_expect_success!(renderer_handle.spawn_worker());
                heph_expect_err!(
                    renderer_handle.spawn_worker(),
                    RendererError::invalid_operation("")
                );
            });
        });
    }

    fn test_excess_threads(&self) {
        // This should exceed `RENDERER_MAX_CONCURRENT_THREADS`.
        let thread_count = heph_gl::renderers::max_concurrent_threads() + 1;

        let mut renderer = self.create_renderer_with_any_device(&[]);
        let renderer_handle = RendererHandle::<TestRenderer>::from(&mut renderer);
        let fail_count = AtomicUsize::new(0);
        std::thread::scope(|s| {
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(thread_count));
            for _ in 1..thread_count {
                let barrier = barrier.clone();
                let fail_count_ref = &fail_count;
                s.spawn(move || {
                    let result = renderer_handle.spawn_worker();
                    if let Err(e) = result {
                        if e == RendererError::fail("") {
                            fail_count_ref.fetch_add(1, Ordering::Relaxed);
                        }
                        barrier.wait();
                    } else {
                        let _worker = result.unwrap();
                        barrier.wait();
                    }
                });
            }
            barrier.wait();
            assert_ne!(fail_count.load(Ordering::Relaxed), 0);
        });
    }

    fn test_single_threaded_compute(
        &self,
        target_device_type: heph_gl::graphics_device::Type,
        frames_in_flight: u32,
        n_frames: usize,
    ) {
        const DATA_COUNT: usize = 256;
        const BYTE_SIZE: usize = DATA_COUNT * std::mem::size_of::<f32>();

        let create_renderer_result = self.create_renderer_with_target_device(
            target_device_type,
            &[FeatureRequest::new(ComputeShaders, true)],
            false,
        );
        assert!(
            create_renderer_result.is_some(),
            "The test should be skipped if device of the target type does not exist."
        );
        let mut renderer = create_renderer_result.unwrap();

        let settings = Settings {
            frames_in_flight,
            ..Default::default()
        };
        heph_expect_success!(renderer.set_settings(settings));

        let shader =
            heph_expect_success!(Shader::from_file(SHADERS_DIR.to_owned() + "/addition.spv"));
        let pipeline = heph_expect_success!(renderer.create_compute_pipeline(&shader));
        drop(shader);

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

            heph_expect_success!(renderer.record_compute_command(
                &pipeline,
                &[&bindings],
                (1, 1, 1)
            ));

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
    }

    fn test_single_threaded_compute_discrete_gpu(&self) {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = DiscreteGpu;
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
    }

    fn test_single_threaded_compute_integrated_gpu(&self) {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = IntegratedGpu;
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
    }

    fn test_single_threaded_compute_cpu(&self) {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = Cpu;
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
    }

    fn test_single_threaded_compute_virtual_gpu(&self) {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = VirtualGpu;
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
    }

    fn test_single_threaded_compute_other(&self) {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type =
            heph_gl::graphics_device::Type::Other;
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
    }

    fn test_multi_threaded_compute(
        &self,
        target_device_type: heph_gl::graphics_device::Type,
        n_threads: usize,
    ) {
        const DATA_COUNT: usize = 256;
        const BYTE_SIZE: usize = DATA_COUNT * std::mem::size_of::<f32>();

        let create_renderer_result = self.create_renderer_with_target_device(
            target_device_type,
            &[FeatureRequest::new(ComputeShaders, true)],
            false,
        );
        assert!(
            create_renderer_result.is_some(),
            "The test should be skipped if device of the target type does not exist."
        );
        let mut renderer = create_renderer_result.unwrap();

        let shader_path = SHADERS_DIR.to_owned() + "/addition.spv";
        let shader = heph_expect_success!(Shader::from_file(shader_path));
        let pipeline = heph_expect_success!(renderer.create_compute_pipeline(&shader));
        drop(shader);

        std::thread::scope(|s| {
            let (tx, rx) = std::sync::mpsc::channel();
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(n_threads + 1));
            let renderer_handle = RendererHandle::<TestRenderer>::from(&mut renderer);

            for thread_id in 0..n_threads {
                let tx = tx.clone();
                let barrier = barrier.clone();
                let pipeline = pipeline.clone();

                s.spawn(move || {
                    let mut renderer_worker = heph_expect_success!(renderer_handle.spawn_worker());
                    heph_expect_success!(renderer_worker.begin_frame());

                    let i = thread_id + 1;

                    let mut a_data = vec![0.0f32; DATA_COUNT];
                    let mut b_data = vec![0.0f32; DATA_COUNT];
                    for j in 0..DATA_COUNT {
                        a_data[j] = (j * i) as f32;
                        b_data[j] = ((j * 2) * i) as f32;
                    }

                    let buffer_a = heph_expect_success!(
                        renderer_worker.create_buffer(BYTE_SIZE, BufferUsage::Storage)
                    );
                    let buffer_b = heph_expect_success!(
                        renderer_worker.create_buffer(BYTE_SIZE, BufferUsage::Storage)
                    );
                    let buffer_c = heph_expect_success!(
                        renderer_worker.create_buffer(BYTE_SIZE, BufferUsage::Storage)
                    );

                    heph_expect_success!(
                        renderer_worker.write_buffer(&buffer_a, bytemuck::cast_slice(&a_data))
                    );
                    heph_expect_success!(
                        renderer_worker.write_buffer(&buffer_b, bytemuck::cast_slice(&b_data))
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
                    heph_expect_success!(renderer_worker.record_compute_command(
                        &pipeline,
                        &[&bindings],
                        (1, 1, 1)
                    ));

                    heph_expect_success!(renderer_worker.end_frame());

                    heph_expect_success!(tx.send((buffer_a, buffer_b, buffer_c, a_data, b_data,)));
                    barrier.wait();
                });
            }

            let mut cleanup_data = Vec::with_capacity(n_threads);
            for _ in 0..n_threads {
                let (buffer_a, buffer_b, buffer_c, a_data, b_data) =
                    heph_expect_success!(rx.recv());
                cleanup_data.push((buffer_a, buffer_b, buffer_c, a_data, b_data));
            }

            heph_expect_success!(renderer.begin_frame());
            heph_expect_success!(renderer.end_frame());
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
        });

        heph_expect_success!(renderer.destroy_compute_pipeline(&pipeline));
    }

    fn test_multi_threaded_compute_discrete_gpu(&self) {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = DiscreteGpu;
        self.test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
    }

    fn test_multi_threaded_compute_integrated_gpu(&self) {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = IntegratedGpu;
        self.test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
    }

    fn test_multi_threaded_compute_cpu(&self) {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = Cpu;
        self.test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
    }

    fn test_multi_threaded_compute_virtual_gpu(&self) {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type = VirtualGpu;
        self.test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
    }

    fn test_multi_threaded_compute_other(&self) {
        const TARGET_DEVICE_TYPE: heph_gl::graphics_device::Type =
            heph_gl::graphics_device::Type::Other;
        self.test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
    }

    fn test_render_static_shapes(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);

        {
            let msaa = heph_expect_success!(renderer.supported_msaa_list()).last();
            assert!(msaa.is_some());
            heph_expect_success!(renderer.set_settings(Settings {
                msaa: *msaa.unwrap(),
                ..Default::default()
            }));
        }

        let shader_infos = [
            ("dot", "shape", PrimitiveTopology::PointList, 1),
            ("line", "shape", PrimitiveTopology::LineList, 2),
            ("ellipse", "shape", PrimitiveTopology::LineStrip, 257),
            ("pentagon", "shape", PrimitiveTopology::TriangleStrip, 5),
            (
                "basic_triangle",
                "basic_triangle",
                PrimitiveTopology::TriangleList,
                3,
            ),
        ];
        let mut pipelines = Vec::with_capacity(shader_infos.len());
        {
            for (vertex_shader_name, frag_shader_name, topology, draw_count) in shader_infos {
                let vertex_shader = heph_expect_success!(Shader::from_file(format!(
                    "{}/shapes/{}_vert.spv",
                    SHADERS_DIR, vertex_shader_name
                )));
                let frag_shader = heph_expect_success!(Shader::from_file(format!(
                    "{}/shapes/{}_frag.spv",
                    SHADERS_DIR, frag_shader_name
                )));
                pipelines.push((
                    heph_expect_success!(renderer.create_graphics_pipeline(
                        &[&vertex_shader, &frag_shader],
                        &GraphicsPipelineOptions {
                            topology,
                            ..Default::default()
                        },
                    )),
                    draw_count,
                ));
            }
        }

        heph_expect_success!(renderer.begin_frame());
        heph_expect_success!(renderer.clear(RGB::default()));
        for (pipeline, draw_count) in pipelines.iter() {
            heph_expect_success!(renderer.record_graphics_command(pipeline, &[], *draw_count, 1));
        }
        heph_expect_success!(renderer.end_frame());

        heph_expect_success!(renderer.wait_idle());
        for (ref pipeline, _) in pipelines {
            heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline));
        }
    }

    fn test_render_dynamic_cubes(&self) {
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
            0, 1, 2, 2, 3, 0, 4, 6, 5, 6, 4, 7, 0, 3, 7, 7, 4, 0, 1, 5, 6, 6, 2, 1, 0, 4, 5, 5, 1,
            0, 3, 2, 6, 6, 7, 3,
        ];

        let mut renderer = self.create_renderer_with_any_device(&[]);
        let vert_shader = heph_expect_success!(Shader::from_file(format!(
            "{}/shapes/{}",
            SHADERS_DIR, "cube_vert.spv"
        )));
        let frag_shader = heph_expect_success!(Shader::from_file(format!(
            "{}/shapes/{}",
            SHADERS_DIR, "cube_frag.spv"
        )));
        let pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions {
                blending: ColorBlending::Alpha,
                culling: CullingMode::Back,
                front_face: FrontFace::Clockwise,
                ..Default::default()
            }
        ));
        drop(vert_shader);
        drop(frag_shader);

        let vertex_buffer_size = std::mem::size_of::<[Vertex; 8]>();
        let index_buffer_size = std::mem::size_of_val(&INDICES);
        let mut cube_a_buffer =
            heph_expect_success!(renderer.create_buffer(vertex_buffer_size, BufferUsage::Vertex));
        let mut cube_b_buffer =
            heph_expect_success!(renderer.create_buffer(vertex_buffer_size, BufferUsage::Vertex));
        let mut index_buffer =
            heph_expect_success!(renderer.create_buffer(index_buffer_size, BufferUsage::Index));

        heph_expect_success!(renderer.write_buffer(&index_buffer, bytemuck::cast_slice(&INDICES)));

        let cube_vertices = |offset: [f32; 3], color: [f32; 4]| -> [Vertex; 8] {
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
        };

        heph_expect_success!(renderer.begin_frame());
        heph_expect_success!(renderer.clear(RGB::default()));

        let cube_a = cube_vertices([-0.5, -0.125, 0.5], [1.0, 0.0, 0.0, 1.0]);
        let cube_b = cube_vertices([0.5, 0.125, 0.2], [0.0, 0.0, 1.0, 0.5]);
        heph_expect_success!(renderer.write_buffer(&cube_a_buffer, bytemuck::cast_slice(&cube_a)));
        heph_expect_success!(renderer.write_buffer(&cube_b_buffer, bytemuck::cast_slice(&cube_b)));

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
        heph_expect_success!(renderer.record_graphics_command(
            &pipeline,
            &[&cube_a_bindings],
            INDICES.len() as u32,
            1
        ));
        heph_expect_success!(renderer.record_graphics_command(
            &pipeline,
            &[&cube_b_bindings],
            INDICES.len() as u32,
            1
        ));

        heph_expect_success!(renderer.end_frame());

        heph_expect_success!(renderer.wait_idle());
        heph_expect_success!(renderer.destroy_buffer(&mut cube_a_buffer));
        heph_expect_success!(renderer.destroy_buffer(&mut cube_b_buffer));
        heph_expect_success!(renderer.destroy_buffer(&mut index_buffer));
        heph_expect_success!(renderer.destroy_graphics_pipeline(&pipeline));
    }

    fn test_render_textures(&self) {
        type Vertex = [f32; 8];
        const IMAGE_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_diff_1k.jpg";
        const LATITUDE_SEGMENTS: usize = 32;
        const LONGITUDE_SEGMENTS: usize = 64;
        const TEXT: &str = "Rendering Textures!";
        const FONT_PATH: &str = "assets/liberation_sans/LiberationSans-Regular.ttf";
        const FONT_SIZE: f32 = 64.0;

        let sphere_position = |radius: f32, x_offset: f32, u: f32, v: f32| -> [f32; 3] {
            let theta = v * std::f32::consts::PI;
            let phi = u * std::f32::consts::TAU;
            let sin_theta = theta.sin();
            [
                x_offset + radius * sin_theta * phi.cos(),
                radius * theta.cos(),
                radius * sin_theta * phi.sin(),
            ]
        };
        let sphere_vertices = |radius: f32, x_offset: f32| -> Vec<Vertex> {
            let mut vertices = Vec::with_capacity(LATITUDE_SEGMENTS * LONGITUDE_SEGMENTS * 6);

            for y in 0..LATITUDE_SEGMENTS {
                let v0 = y as f32 / LATITUDE_SEGMENTS as f32;
                let v1 = (y + 1) as f32 / LATITUDE_SEGMENTS as f32;

                for x in 0..LONGITUDE_SEGMENTS {
                    let u0 = x as f32 / LONGITUDE_SEGMENTS as f32;
                    let u1 = (x + 1) as f32 / LONGITUDE_SEGMENTS as f32;

                    let p00 = sphere_position(radius, x_offset, u0, v0);
                    let p10 = sphere_position(radius, x_offset, u1, v0);
                    let p01 = sphere_position(radius, x_offset, u0, v1);
                    let p11 = sphere_position(radius, x_offset, u1, v1);

                    let n00 = [
                        (p00[0] - x_offset) / radius,
                        p00[1] / radius,
                        p00[2] / radius,
                    ];
                    let n10 = [
                        (p10[0] - x_offset) / radius,
                        p10[1] / radius,
                        p10[2] / radius,
                    ];
                    let n01 = [
                        (p01[0] - x_offset) / radius,
                        p01[1] / radius,
                        p01[2] / radius,
                    ];
                    let n11 = [
                        (p11[0] - x_offset) / radius,
                        p11[1] / radius,
                        p11[2] / radius,
                    ];

                    vertices.extend_from_slice(&[
                        [p00[0], p00[1], p00[2], n00[0], n00[1], n00[2], u0, v0],
                        [p10[0], p10[1], p10[2], n10[0], n10[1], n10[2], u1, v0],
                        [p11[0], p11[1], p11[2], n11[0], n11[1], n11[2], u1, v1],
                        [p00[0], p00[1], p00[2], n00[0], n00[1], n00[2], u0, v0],
                        [p11[0], p11[1], p11[2], n11[0], n11[1], n11[2], u1, v1],
                        [p01[0], p01[1], p01[2], n01[0], n01[1], n01[2], u0, v1],
                    ]);
                }
            }

            vertices
        };
        let render_sphere = |renderer: &mut TestRenderer, sampler: TestRenderer::Sampler| {
            let vert_shader = heph_expect_success!(Shader::from_file(format!(
                "{}/{}",
                SHADERS_DIR, "broken_brick_wall_vert.spv"
            )));
            let frag_shader = heph_expect_success!(Shader::from_file(format!(
                "{}/{}",
                SHADERS_DIR, "broken_brick_wall_frag.spv"
            )));
            let pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
                &[&vert_shader, &frag_shader],
                &GraphicsPipelineOptions::default(),
            ));

            let image = heph_expect_success!(image::open(IMAGE_PATH)).to_rgba8();
            let width = image.width();
            let height = image.height();

            let mut vertices = Vec::with_capacity(LATITUDE_SEGMENTS * LONGITUDE_SEGMENTS * 6 * 3);
            vertices.extend(sphere_vertices(0.1, -1.2));
            vertices.extend(sphere_vertices(0.25, -0.4));
            vertices.extend(sphere_vertices(0.5, 0.8));
            let vertex_buffer = heph_expect_success!(renderer.create_buffer(
                vertices.len() * std::mem::size_of::<Vertex>(),
                BufferUsage::Vertex,
            ));
            heph_expect_success!(
                renderer.write_buffer(&vertex_buffer, bytemuck::cast_slice(&vertices))
            );

            let texture = heph_expect_success!(renderer.create_texture(
                &TextureOptions {
                    width,
                    height,
                    format: TextureFormat::Rgba8Srgb,
                    mip_level_count: TextureOptions::max_mip_level_count(width, height),
                },
                image.as_raw()
            ));

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
                    resource: ResourceBindingType::Texture { handle: texture },
                },
                ResourceBinding {
                    binding: 1,
                    resource: ResourceBindingType::Sampler { handle: sampler },
                },
            ];
            heph_expect_success!(renderer.record_graphics_command(
                &pipeline,
                &[&bindings],
                vertices.len() as u32,
                1
            ));

            (pipeline, texture, vertex_buffer)
        };

        let render_text = |renderer: &mut TestRenderer, sampler: TestRenderer::Sampler| {
            let vert_shader = heph_expect_success!(Shader::from_file(format!(
                "{}/{}",
                SHADERS_DIR, "text_vert.spv"
            )));
            let frag_shader = heph_expect_success!(Shader::from_file(format!(
                "{}/{}",
                SHADERS_DIR, "text_frag.spv"
            )));
            let pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
                &[&vert_shader, &frag_shader],
                &GraphicsPipelineOptions {
                    blending: ColorBlending::Alpha,
                    depth_test: false,
                    depth_write: false,
                    ..Default::default()
                },
            ));

            let font = heph_expect_success!(heph_gl::text::Font::from_file(FONT_PATH, FONT_SIZE));
            let vertices =
                font.vertices(TEXT, [330.0, 50.0], [1280.0, 720.0], [1.0, 1.0, 1.0, 1.0]);
            let vertex_buffer = heph_expect_success!(renderer.create_buffer(
                vertices.len() * std::mem::size_of::<Vertex>(),
                BufferUsage::Vertex,
            ));
            heph_expect_success!(
                renderer.write_buffer(&vertex_buffer, bytemuck::cast_slice(&vertices))
            );

            let texture = heph_expect_success!(renderer.create_texture(
                &TextureOptions {
                    width: font.atlas_width(),
                    height: font.atlas_height(),
                    format: TextureFormat::R8Unorm,
                    mip_level_count: 1,
                },
                font.atlas_data(),
            ));

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
                    resource: ResourceBindingType::Texture { handle: texture },
                },
                ResourceBinding {
                    binding: 1,
                    resource: ResourceBindingType::Sampler { handle: sampler },
                },
            ];
            heph_expect_success!(renderer.record_graphics_command(
                &pipeline,
                &[&bindings],
                vertices.len() as u32,
                1
            ));

            (pipeline, texture, vertex_buffer)
        };

        let mut renderer = self.create_renderer_with_any_device(&[]);

        let mut sampler = heph_expect_success!(renderer.create_sampler(&SamplerOptions::default()));

        heph_expect_success!(renderer.begin_frame());
        heph_expect_success!(renderer.clear(RGB::default()));
        let (
            broken_brick_wall_pipeline,
            mut broken_brick_wall_texture,
            mut broken_brick_wall_vertex_buffer,
        ) = render_sphere(&mut renderer, sampler);
        let (text_pipeline, mut text_texture, mut text_vertex_buffer) =
            render_text(&mut renderer, sampler);
        heph_expect_success!(renderer.end_frame());

        heph_expect_success!(renderer.wait_idle());
        heph_expect_success!(renderer.destroy_sampler(&mut sampler));
        heph_expect_success!(renderer.destroy_texture(&mut broken_brick_wall_texture));
        heph_expect_success!(renderer.destroy_buffer(&mut broken_brick_wall_vertex_buffer));
        heph_expect_success!(renderer.destroy_graphics_pipeline(&broken_brick_wall_pipeline));
        heph_expect_success!(renderer.destroy_texture(&mut text_texture));
        heph_expect_success!(renderer.destroy_buffer(&mut text_vertex_buffer));
        heph_expect_success!(renderer.destroy_graphics_pipeline(&text_pipeline));
    }
}
