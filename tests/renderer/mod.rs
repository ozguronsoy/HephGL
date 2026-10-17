pub mod flags;
mod test_suites;
pub mod versioned_test_suite;

use std::{marker::PhantomData, process::ExitCode};

use heph_gl::{
    Version,
    graphics_device::Type::{Cpu, DiscreteGpu, IntegratedGpu, VirtualGpu},
    renderers::{
        Renderer,
        concurrency::{RendererHandle, RendererWorkerFactory},
    },
};
use libtest_mimic::{Arguments, Trial};

use crate::heph_expect_success;
use crate::renderer::versioned_test_suite::RendererVersionedTestSuite;

pub struct RendererTests<TestRenderer: Renderer> {
    api_version: Option<Version>,
    _marker: PhantomData<TestRenderer>,
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
                                let panic = heph_expect_success!(
                                    result.err().ok_or("Expected a panic.")
                                );
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
                create_trial!(versioned_test_suite, test_workers_begin_before_main),
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
                create_trial!(versioned_test_suite, test_render_text),
                create_trial!(versioned_test_suite, test_render_camera),
                create_trial!(versioned_test_suite, test_render_meshes),
            ];
            tests.append(&mut test_suite);
        }
        drop(renderer);

        libtest_mimic::run(&args, tests).exit_code()
    }
}
