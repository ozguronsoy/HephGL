use std::collections::HashSet;

use heph_gl::{
    Error, Version,
    graphics_device::{
        Feature::{OpticalFlow, RayTracing, VideoDecoding, VideoEncoding},
        GraphicsDevice,
    },
    renderers::{
        Renderer,
        concurrency::{RendererHandle, RendererWorkerFactory},
        settings::{FeatureRequest, InitializeOptions, Settings},
    },
};

use crate::{
    dummy_display_handle, dummy_window_handle, heph_expect_err, heph_expect_success,
    renderer::{RendererTests, test_suites::TEST_ENV},
    test_env,
};

impl<TestRenderer: Renderer> RendererTests<TestRenderer>
where
    RendererHandle<TestRenderer>: RendererWorkerFactory<TestRenderer>,
{
    pub(crate) fn test_invalid_app_name(&self) {
        let test_env = test_env!();
        let mut renderer = TestRenderer::new();
        heph_expect_err!(renderer.initialize(&InitializeOptions {
            app_name: "\0",
            window_handle: test_env.raw_window_handle(),
            display_handle: test_env.raw_display_handle(),
            api_version: self.api_version,
        }));
    }

    pub(crate) fn test_initialize_renderer(&self) {
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
                Error::invalid_operation("")
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
                Error::invalid_argument("")
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
                Error::invalid_argument("")
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

    pub(crate) fn test_enumerate_devices(&self) {
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

    pub(crate) fn test_set_device(&self) {
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
                    max_sampler_anisotropy: 1.0,
                    supported_features: HashSet::default(),
                }),
                &features
            ),
            Error::fail("")
        );

        for feature in &mut features {
            feature.required = true;
        }
        heph_expect_err!(
            renderer.set_device(None, &features),
            Error::UnsupportedRequiredFeature(RayTracing)
        );
    }

    pub(crate) fn test_set_settings(&self) {
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
                let renderer_handle = RendererHandle::from(&mut renderer);
                let _ = s.spawn(move || {
                    let _renderer_worker = heph_expect_success!(renderer_handle.spawn_worker());
                    let _ = b2.wait();
                    let _ = b2.wait();
                });
                let _ = b1.wait();
                heph_expect_err!(
                    renderer.set_settings(settings),
                    Error::invalid_operation("")
                );
                let _ = b1.wait();
            });
        }
    }
}
