use heph_gl::{
    Error,
    GraphicsDeviceFeature::{
        AnisotropicFiltering, AsyncTransfer, BcTextures, ComputeShaders, DepthBiasClamp,
        DepthClamp, GeometryShaders, RayTracing, SampleRateShading, TessellationShaders,
        VideoDecoding, VideoEncoding, WireframeMode,
    },
    Result, Shader, Version,
    renderers::{
        BufferUsage, CanvasOptions, DISPLAY_CANVAS, FeatureRequest, GraphicsPipelineOptions,
        HdrMode, InitializeOptions, Renderer, RendererCanvas, RendererTaskParam, ResourceBinding,
        SamplerOptions, Settings, TextureFormat, TextureOptions,
        debug::{Debugger, RendererDebugObject, Severity},
    },
};
use renkrs::RGB;

use crate::{
    dummy_display_handle, dummy_window_handle, heph_expect_err, heph_expect_success,
    renderer::{RendererTests, test_suites::TEST_ENV},
    test_env,
    utils::SHADERS_DIR,
};

impl<TestRenderer: Renderer> RendererTests<TestRenderer> {
    pub(crate) fn test_initialize_renderer(&self) {
        {
            let test_env = test_env!();
            let mut renderer = TestRenderer::new();
            let init_options = InitializeOptions {
                app_name: "",
                window_handle: test_env.raw_window_handle(),
                display_handle: test_env.raw_display_handle(),
                api_version: self.api_version,
                debug: false,
            };
            heph_expect_success!(renderer.initialize(&init_options));
            heph_expect_err!(
                renderer.initialize(&init_options),
                Error::InvalidOperation(_)
            );
            heph_expect_success!(renderer.uninitialize());
        }

        {
            let test_env = test_env!();
            let mut renderer = TestRenderer::new();
            heph_expect_err!(
                renderer.initialize(&InitializeOptions {
                    app_name: "\0",
                    window_handle: test_env.raw_window_handle(),
                    display_handle: test_env.raw_display_handle(),
                    api_version: self.api_version,
                    debug: false,
                }),
                Error::InvalidArgument(_)
            );
        }

        {
            let test_env = test_env!();
            let mut renderer = TestRenderer::new();
            let init_options = InitializeOptions {
                app_name: "",
                window_handle: test_env.raw_window_handle(),
                display_handle: test_env.raw_display_handle(),
                api_version: Some(Version::new(u32::MIN, u32::MIN, u32::MIN)),
                debug: false,
            };
            heph_expect_err!(
                renderer.initialize(&init_options),
                Error::InvalidArgument(_)
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
                debug: false,
            };
            heph_expect_err!(
                renderer.initialize(&init_options),
                Error::InvalidArgument(_)
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
                debug: false,
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
                debug: false,
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
                debug: false,
            };
            heph_expect_err!(renderer.initialize(&init_options));
        }
    }

    pub(crate) fn test_debugger(&self) {
        const THREAD_COUNT: usize = 4;

        struct ForeignDebugObject;
        impl RendererDebugObject for ForeignDebugObject {}

        struct WorkerIndex(usize);
        impl RendererTaskParam for WorkerIndex {}

        struct WorkerCanvas {
            canvas: Box<dyn RendererCanvas>,
        }
        impl RendererTaskParam for WorkerCanvas {}

        fn push_label(renderer: &dyn Renderer, label: &str) {
            heph_expect_success!(
                heph_expect_success!(renderer.debugger().ok_or("No debugger is attached."))
                    .push_label(label)
            );
        }

        fn pop_label(renderer: &dyn Renderer) {
            heph_expect_success!(
                heph_expect_success!(renderer.debugger().ok_or("No debugger is attached."))
                    .pop_label()
            );
        }

        fn canvas_task(
            renderer: &mut dyn Renderer,
            _: Option<&mut dyn RendererTaskParam>,
        ) -> Result<Option<Box<dyn RendererTaskParam>>> {
            heph_expect_success!(renderer.begin_frame());
            push_label(renderer, "Worker");
            let mut canvas = heph_expect_success!(renderer.create_canvas(&CanvasOptions {
                width: 64,
                height: 64,
                ..Default::default()
            }));
            push_label(renderer, "Clear Canvas");
            heph_expect_success!(renderer.clear(Some(canvas.as_mut()), RGB::default()));
            pop_label(renderer);
            pop_label(renderer);
            heph_expect_success!(renderer.end_frame());
            Ok(Some(Box::new(WorkerCanvas { canvas })))
        }

        fn buffer_name_task(
            renderer: &mut dyn Renderer,
            param: Option<&mut dyn RendererTaskParam>,
        ) -> Result<Option<Box<dyn RendererTaskParam>>> {
            let index = heph_expect_success!(
                heph_expect_success!(param.ok_or("Missing the worker index."))
                    .downcast_ref::<WorkerIndex>()
            )
            .0;
            let mut buffer =
                heph_expect_success!(renderer.create_buffer(1024, BufferUsage::Storage));
            heph_expect_success!(
                heph_expect_success!(renderer.debugger().ok_or("No debugger is attached."))
                    .set_object_name(buffer.as_ref(), &format!("Worker Buffer {index}"))
            );
            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
            Ok(None)
        }

        fn no_error_logs_task(
            renderer: &mut dyn Renderer,
            _: Option<&mut dyn RendererTaskParam>,
        ) -> Result<Option<Box<dyn RendererTaskParam>>> {
            let debugger =
                heph_expect_success!(renderer.debugger().ok_or("No debugger is attached."));
            assert!(
                !debugger
                    .logs()
                    .iter()
                    .any(|log| matches!(log.severity(), Severity::Error))
            );
            Ok(None)
        }
        let create_renderer = |requested_features: &[FeatureRequest]| {
            let test_env = test_env!();
            let mut renderer = TestRenderer::new();
            heph_expect_success!(renderer.initialize(&InitializeOptions {
                app_name: "",
                window_handle: test_env.raw_window_handle(),
                display_handle: test_env.raw_display_handle(),
                api_version: self.api_version,
                debug: true,
            }));
            heph_expect_success!(
                renderer
                    .attach_debugger(Debugger::default())
                    .map_err(|(e, _)| e)
            );
            heph_expect_success!(renderer.set_device(None, requested_features));
            renderer
        };
        let expect_no_warnings = |renderer: &mut TestRenderer| {
            let debugger = heph_expect_success!(renderer.detach_debugger());
            for log in debugger.logs().iter() {
                assert!(
                    !matches!(log.severity(), Severity::Warning | Severity::Error),
                    "{}",
                    log
                );
            }
        };

        {
            let test_env = test_env!();
            let mut renderer = TestRenderer::new();
            heph_expect_success!(renderer.initialize(&InitializeOptions {
                app_name: "",
                window_handle: test_env.raw_window_handle(),
                display_handle: test_env.raw_display_handle(),
                api_version: self.api_version,
                debug: true,
            }));
            assert!(renderer.debugger().is_none());
            heph_expect_err!(renderer.detach_debugger(), Error::InvalidOperation(_));
            heph_expect_success!(
                renderer
                    .attach_debugger(Debugger::default())
                    .map_err(|(e, _)| e)
            );
            heph_expect_err!(
                renderer
                    .attach_debugger(Debugger::default())
                    .map_err(|(e, _)| e),
                Error::InvalidOperation(_)
            );
            let debugger = heph_expect_success!(renderer.detach_debugger());
            heph_expect_success!(renderer.attach_debugger(debugger).map_err(|(e, _)| e));
        }

        {
            let test_env = test_env!();
            let mut renderer = TestRenderer::new();
            heph_expect_err!(
                renderer
                    .attach_debugger(Debugger::default())
                    .map_err(|(e, _)| e),
                Error::InvalidOperation(_)
            );
            heph_expect_success!(renderer.initialize(&InitializeOptions {
                app_name: "",
                window_handle: test_env.raw_window_handle(),
                display_handle: test_env.raw_display_handle(),
                api_version: self.api_version,
                debug: false,
            }));
            heph_expect_err!(
                renderer
                    .attach_debugger(Debugger::default())
                    .map_err(|(e, _)| e),
                Error::InvalidOperation(_)
            );
            heph_expect_err!(renderer.detach_debugger(), Error::InvalidOperation(_));
        }

        {
            let debugger = Debugger::default();
            heph_expect_err!(
                debugger.set_object_name(&ForeignDebugObject, "Foreign Object"),
                Error::InvalidOperation(_)
            );
        }

        {
            let mut renderer = create_renderer(&[]);
            let mut buffer =
                heph_expect_success!(renderer.create_buffer(1024, BufferUsage::Storage));
            heph_expect_success!(renderer.begin_frame());
            let mut texture = heph_expect_success!(renderer.create_texture(
                &TextureOptions {
                    width: 1,
                    height: 1,
                    format: TextureFormat::Rgba8Unorm,
                    mip_level_count: 1,
                },
                &[0; 4]
            ));
            let mut color_canvas = heph_expect_success!(renderer.create_canvas(&CanvasOptions {
                width: 64,
                height: 64,
                ..Default::default()
            }));
            let mut depth_canvas = heph_expect_success!(renderer.create_canvas(&CanvasOptions {
                width: 64,
                height: 64,
                depth_only: true,
                ..Default::default()
            }));
            heph_expect_success!(renderer.end_frame());
            heph_expect_success!(renderer.wait_idle());
            let mut sampler =
                heph_expect_success!(renderer.create_sampler(&SamplerOptions::default()));
            let compute_shader =
                heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/addition.spv")));
            let mut compute_pipeline =
                heph_expect_success!(renderer.create_compute_pipeline(&compute_shader));
            let frag_shader =
                heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/mesh_frag.spv")));
            let vert_shader =
                heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/camera_vert.spv")));
            let mut graphics_pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
                &[&frag_shader, &vert_shader],
                &GraphicsPipelineOptions::default()
            ));

            {
                let debugger =
                    heph_expect_success!(renderer.debugger().ok_or("No debugger is attached."));
                heph_expect_success!(debugger.set_object_name(buffer.as_ref(), "Buffer"));
                heph_expect_success!(debugger.set_object_name(texture.as_ref(), "Texture"));
                heph_expect_success!(debugger.set_object_name(sampler.as_ref(), "Sampler"));
                heph_expect_success!(
                    debugger.set_object_name(compute_pipeline.as_ref(), "Compute Pipeline")
                );
                heph_expect_success!(
                    debugger.set_object_name(graphics_pipeline.as_ref(), "Graphics Pipeline")
                );
                heph_expect_success!(
                    debugger.set_object_name(color_canvas.as_ref(), "Color Canvas")
                );
                heph_expect_success!(
                    debugger.set_object_name(depth_canvas.as_ref(), "Depth Canvas")
                );
                heph_expect_success!(debugger.set_object_name(buffer.as_ref(), ""));
                heph_expect_err!(
                    debugger.set_object_name(buffer.as_ref(), "Buf\0fer"),
                    Error::InvalidArgument(_)
                );
                heph_expect_err!(
                    debugger.set_object_name(&ForeignDebugObject, "Foreign Object"),
                    Error::InvalidArgument(_)
                );
            }

            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
            heph_expect_success!(renderer.destroy_texture(texture.as_mut()));
            heph_expect_success!(renderer.destroy_sampler(sampler.as_mut()));
            heph_expect_success!(renderer.destroy_compute_pipeline(compute_pipeline.as_mut()));
            heph_expect_success!(renderer.destroy_graphics_pipeline(graphics_pipeline.as_mut()));
            heph_expect_success!(renderer.destroy_canvas(color_canvas.as_mut()));
            heph_expect_success!(renderer.destroy_canvas(depth_canvas.as_mut()));

            {
                let debugger =
                    heph_expect_success!(renderer.debugger().ok_or("No debugger is attached."));
                heph_expect_err!(
                    debugger.set_object_name(buffer.as_ref(), "Buffer"),
                    Error::InvalidArgument(_)
                );
                heph_expect_err!(
                    debugger.set_object_name(texture.as_ref(), "Texture"),
                    Error::InvalidArgument(_)
                );
                heph_expect_err!(
                    debugger.set_object_name(sampler.as_ref(), "Sampler"),
                    Error::InvalidArgument(_)
                );
                heph_expect_err!(
                    debugger.set_object_name(compute_pipeline.as_ref(), "Compute Pipeline"),
                    Error::InvalidArgument(_)
                );
                heph_expect_err!(
                    debugger.set_object_name(graphics_pipeline.as_ref(), "Graphics Pipeline"),
                    Error::InvalidArgument(_)
                );
                heph_expect_err!(
                    debugger.set_object_name(color_canvas.as_ref(), "Color Canvas"),
                    Error::InvalidArgument(_)
                );
                heph_expect_err!(
                    debugger.set_object_name(depth_canvas.as_ref(), "Depth Canvas"),
                    Error::InvalidArgument(_)
                );
            }

            let mut buffer =
                heph_expect_success!(renderer.create_buffer(1024, BufferUsage::Storage));
            let debugger = heph_expect_success!(renderer.detach_debugger());
            heph_expect_err!(
                debugger.set_object_name(buffer.as_ref(), "Buffer"),
                Error::InvalidOperation(_)
            );
            heph_expect_success!(renderer.attach_debugger(debugger).map_err(|(e, _)| e));
            heph_expect_success!(
                heph_expect_success!(renderer.debugger().ok_or("No debugger is attached."))
                    .set_object_name(buffer.as_ref(), "Buffer")
            );
            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
            expect_no_warnings(&mut renderer);
        }

        {
            let debugger = Debugger::default();
            heph_expect_err!(debugger.pop_label(), Error::InvalidOperation(_));
            heph_expect_err!(debugger.push_label("La\0bel"), Error::InvalidArgument(_));
            heph_expect_success!(debugger.push_label("Label"));
            heph_expect_success!(debugger.push_label("Nested Label"));
            heph_expect_success!(debugger.pop_label());
            heph_expect_success!(debugger.pop_label());
            heph_expect_err!(debugger.pop_label(), Error::InvalidOperation(_));
        }

        {
            let mut renderer = create_renderer(&[
                FeatureRequest::new(AsyncTransfer, false),
                FeatureRequest::new(ComputeShaders, false),
            ]);
            let compute_supported = renderer
                .get_device()
                .unwrap()
                .supported_features()
                .contains(&ComputeShaders);
            let mut compute_pipeline = if compute_supported {
                let compute_shader =
                    heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/addition.spv")));
                Some(heph_expect_success!(
                    renderer.create_compute_pipeline(&compute_shader)
                ))
            } else {
                None
            };
            let mut buffers = (0..3)
                .map(|_| heph_expect_success!(renderer.create_buffer(1024, BufferUsage::Storage)))
                .collect::<Vec<_>>();

            heph_expect_success!(renderer.begin_frame());
            push_label(&renderer, "Frame");
            push_label(&renderer, "Resources");
            let mut texture = heph_expect_success!(renderer.create_texture(
                &TextureOptions {
                    width: 4,
                    height: 4,
                    format: TextureFormat::Rgba8Unorm,
                    mip_level_count: 3,
                },
                &[0; 64]
            ));
            let mut canvas = heph_expect_success!(renderer.create_canvas(&CanvasOptions {
                width: 64,
                height: 64,
                ..Default::default()
            }));
            pop_label(&renderer);
            push_label(&renderer, "Clear Canvas");
            heph_expect_success!(renderer.clear(Some(canvas.as_mut()), RGB::default()));
            pop_label(&renderer);
            if let Some(compute_pipeline) = &mut compute_pipeline {
                push_label(&renderer, "Compute");
                let bindings = [
                    ResourceBinding::buffer(0, buffers[0].as_ref(), 0, buffers[0].size()),
                    ResourceBinding::buffer(1, buffers[1].as_ref(), 0, buffers[1].size()),
                    ResourceBinding::buffer(2, buffers[2].as_ref(), 0, buffers[2].size()),
                ];
                heph_expect_success!(renderer.record_compute_command(
                    compute_pipeline.as_mut(),
                    &[&bindings],
                    (1, 1, 1)
                ));
                pop_label(&renderer);
            }
            push_label(&renderer, "Clear");
            heph_expect_success!(renderer.clear(DISPLAY_CANVAS, RGB::default()));
            pop_label(&renderer);
            pop_label(&renderer);
            heph_expect_success!(renderer.end_frame());

            heph_expect_success!(renderer.begin_frame());
            push_label(&renderer, "Empty");
            pop_label(&renderer);
            push_label(&renderer, "Clear");
            heph_expect_success!(renderer.clear(DISPLAY_CANVAS, RGB::default()));
            pop_label(&renderer);
            heph_expect_success!(renderer.end_frame());
            heph_expect_success!(renderer.wait_idle());

            heph_expect_success!(renderer.destroy_texture(texture.as_mut()));
            heph_expect_success!(renderer.destroy_canvas(canvas.as_mut()));
            for buffer in &mut buffers {
                heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
            }
            if let Some(compute_pipeline) = &mut compute_pipeline {
                heph_expect_success!(renderer.destroy_compute_pipeline(compute_pipeline.as_mut()));
            }
            expect_no_warnings(&mut renderer);
        }

        {
            let mut renderer = create_renderer(&[]);
            let mut workers = (0..THREAD_COUNT)
                .map(|_| {
                    let mut worker = renderer.spawn_worker();
                    heph_expect_success!(
                        worker.work(&[canvas_task], None, true).map_err(|(e, _)| e)
                    );
                    worker
                })
                .collect::<Vec<_>>();

            heph_expect_success!(renderer.begin_frame());
            std::thread::sleep(std::time::Duration::from_millis(100));
            push_label(&renderer, "Main");
            heph_expect_success!(renderer.clear(DISPLAY_CANVAS, RGB::default()));
            pop_label(&renderer);
            heph_expect_success!(renderer.end_frame());
            heph_expect_success!(renderer.wait_idle());

            let mut results = workers
                .iter_mut()
                .map(|worker| {
                    heph_expect_success!(
                        heph_expect_success!(worker.finish())
                            .ok_or("The worker returned no param.")
                    )
                })
                .collect::<Vec<_>>();
            drop(workers);
            for result in &mut results {
                let worker_canvas = heph_expect_success!(result.downcast_mut::<WorkerCanvas>());
                heph_expect_success!(renderer.destroy_canvas(worker_canvas.canvas.as_mut()));
            }
            expect_no_warnings(&mut renderer);
        }

        {
            let mut renderer = create_renderer(&[]);
            let mut workers = (0..THREAD_COUNT)
                .map(|i| {
                    let mut worker = renderer.spawn_worker();
                    heph_expect_success!(
                        worker
                            .work(&[buffer_name_task], Some(Box::new(WorkerIndex(i))), true)
                            .map_err(|(e, _)| e)
                    );
                    worker
                })
                .collect::<Vec<_>>();

            let mut buffer =
                heph_expect_success!(renderer.create_buffer(1024, BufferUsage::Storage));
            heph_expect_success!(
                heph_expect_success!(renderer.debugger().ok_or("No debugger is attached."))
                    .set_object_name(buffer.as_ref(), "Main Buffer")
            );
            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));

            for worker in &mut workers {
                let _ = heph_expect_success!(worker.finish());
            }
            expect_no_warnings(&mut renderer);
        }

        {
            let mut renderer = create_renderer(&[]);
            let _ = heph_expect_success!(renderer.create_sampler(&SamplerOptions::default()));
            let mut workers = (0..THREAD_COUNT)
                .map(|_| {
                    let mut worker = renderer.spawn_worker();
                    heph_expect_success!(
                        worker
                            .work(&[no_error_logs_task], None, true)
                            .map_err(|(e, _)| e)
                    );
                    worker
                })
                .collect::<Vec<_>>();

            let debugger =
                heph_expect_success!(renderer.debugger().ok_or("No debugger is attached."));
            assert!(
                !debugger
                    .logs()
                    .iter()
                    .any(|log| matches!(log.severity(), Severity::Error))
            );

            for worker in &mut workers {
                let _ = heph_expect_success!(worker.finish());
            }
            heph_expect_success!(renderer.uninitialize());
            let debugger = heph_expect_success!(renderer.detach_debugger());
            assert!(
                debugger
                    .logs()
                    .iter()
                    .any(|log| matches!(log.severity(), Severity::Error))
            );
        }
    }

    pub(crate) fn test_enumerate_devices(&self) {
        let renderer = self.create_renderer();
        let devices = heph_expect_success!(renderer.enumerate_devices());
        assert!(
            !devices.is_empty(),
            "Invalid Test Env: No graphics device found to run remaining tests."
        );
        assert!(devices.is_sorted_by_key(|d| (d.device_type(), std::cmp::Reverse(d.vram()))));

        // For debugging in CI.
        for device in &devices {
            println!("{device}");
        }
    }

    pub(crate) fn test_set_device(&self) {
        let mut features = [
            FeatureRequest::new(AnisotropicFiltering, false),
            FeatureRequest::new(AsyncTransfer, false),
            FeatureRequest::new(BcTextures, false),
            FeatureRequest::new(ComputeShaders, false),
            FeatureRequest::new(DepthBiasClamp, false),
            FeatureRequest::new(DepthClamp, false),
            FeatureRequest::new(GeometryShaders, false),
            FeatureRequest::new(RayTracing, false),
            FeatureRequest::new(SampleRateShading, false),
            FeatureRequest::new(TessellationShaders, false),
            FeatureRequest::new(VideoDecoding, false),
            FeatureRequest::new(VideoEncoding, false),
            FeatureRequest::new(WireframeMode, false),
        ];
        let mut renderer = self.create_renderer_with_any_device(&features);
        assert!(renderer.get_device().is_some());

        let devices = heph_expect_success!(renderer.enumerate_devices());
        if let Some(device) = devices.iter().find(|d| {
            features
                .iter()
                .any(|f| !d.supported_features().contains(&f.feature))
        }) {
            for feature in &mut features {
                feature.required = true;
            }
            heph_expect_err!(
                renderer.set_device(Some(device), &features),
                Error::UnsupportedRequiredFeature(_)
            );
        }
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
            fn idle_task(
                _: &mut dyn Renderer,
                _: Option<&mut dyn RendererTaskParam>,
            ) -> Result<Option<Box<dyn RendererTaskParam>>> {
                Ok(None)
            }

            let mut renderer = self.create_renderer_with_any_device(&[]);
            let mut worker = renderer.spawn_worker();
            heph_expect_success!(worker.work(&[idle_task], None, true).map_err(|(e, _)| e));
            heph_expect_err!(renderer.set_settings(settings), Error::InvalidOperation(_));
            let _ = heph_expect_success!(worker.finish());
        }

        {
            let mut renderer = self.create_renderer_with_any_device(&[]);
            heph_expect_err!(
                renderer.set_settings(Settings {
                    max_fps: Some(0),
                    ..Default::default()
                }),
                Error::InvalidArgument(_)
            );
        }
    }

    pub(crate) fn test_is_hdr_mode_supported(&self) {
        {
            let renderer = self.create_renderer();
            heph_expect_err!(
                renderer.is_hdr_mode_supported(HdrMode::Disabled),
                Error::InvalidOperation(_)
            );
        }

        {
            let mut renderer = self.create_renderer_with_any_device(&[]);
            assert!(heph_expect_success!(
                renderer.is_hdr_mode_supported(HdrMode::Disabled)
            ));
            for hdr_mode in [HdrMode::Hdr10, HdrMode::Scrgb] {
                let supported = heph_expect_success!(renderer.is_hdr_mode_supported(hdr_mode));
                heph_expect_success!(renderer.set_settings(Settings {
                    hdr_mode,
                    ..Default::default()
                }));
                if supported {
                    assert_eq!(renderer.get_settings().hdr_mode, hdr_mode);
                } else {
                    assert_eq!(renderer.get_settings().hdr_mode, HdrMode::Disabled);
                    let debugger =
                        heph_expect_success!(renderer.debugger().ok_or("No debugger is attached."));
                    let mut logs = debugger.logs();
                    let index = heph_expect_success!(
                        logs.iter()
                            .position(|log| matches!(log.severity(), Severity::Warning))
                            .ok_or("No fallback warning found.")
                    );
                    let _ = logs.remove(index);
                }
            }
        }
    }
}
