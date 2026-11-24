use heph_gl::{
    Error,
    renderers::{DISPLAY_CANVAS, Renderer},
};

use crate::{heph_expect_err, heph_expect_success, renderer::RendererTests};

impl<TestRenderer: Renderer> RendererTests<TestRenderer> {
    pub(crate) fn test_begin_twice(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);
        heph_expect_success!(renderer.begin_frame());
        heph_expect_err!(renderer.begin_frame(), Error::InvalidOperation(_));
    }

    pub(crate) fn test_end_without_begin(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);
        heph_expect_err!(renderer.end_frame(), Error::InvalidOperation(_));
    }

    pub(crate) fn test_calling_frame_only_fn_outside_of_frame(&self) {
        use heph_gl::{
            GraphicsDeviceFeature::ComputeShaders,
            Shader,
            renderers::{
                CanvasOptions, FeatureRequest, GraphicsPipelineOptions, TextureFormat,
                TextureOptions,
            },
        };

        use crate::utils::SHADERS_DIR;

        let mut renderer =
            self.create_renderer_with_any_device(&[FeatureRequest::new(ComputeShaders, true)]);
        let compute_shader =
            heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/addition.spv")));
        let vertex_shader = heph_expect_success!(Shader::from_file(format!(
            "{SHADERS_DIR}/shapes/basic_triangle_vert.spv"
        )));
        let fragment_shader = heph_expect_success!(Shader::from_file(format!(
            "{SHADERS_DIR}/shapes/basic_triangle_frag.spv"
        )));
        let mut compute_pipeline =
            heph_expect_success!(renderer.create_compute_pipeline(&compute_shader));
        let mut graphics_pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
            &[&vertex_shader, &fragment_shader],
            &GraphicsPipelineOptions::default()
        ));

        for frame in 0..2 {
            if frame == 1 {
                heph_expect_success!(renderer.begin_frame());
                heph_expect_success!(renderer.end_frame());
            }
            heph_expect_err!(
                renderer.create_texture(
                    &TextureOptions {
                        width: 1,
                        height: 1,
                        format: TextureFormat::Rgba8Srgb,
                        mip_level_count: 1,
                    },
                    &[0; 4]
                ),
                Error::InvalidOperation(_)
            );
            heph_expect_err!(
                renderer.create_canvas(&CanvasOptions {
                    width: 1,
                    height: 1,
                    depth_only: false,
                    ..Default::default()
                }),
                Error::InvalidOperation(_)
            );
            heph_expect_err!(
                renderer.clear(DISPLAY_CANVAS, [0.0, 0.0, 0.0]),
                Error::InvalidOperation(_)
            );
            heph_expect_err!(
                renderer.record_compute_command(compute_pipeline.as_mut(), &[], (1, 1, 1)),
                Error::InvalidOperation(_)
            );
            heph_expect_err!(
                renderer.record_graphics_command(
                    DISPLAY_CANVAS,
                    graphics_pipeline.as_mut(),
                    None,
                    &[],
                    &[],
                    3,
                    1
                ),
                Error::InvalidOperation(_)
            );
        }

        heph_expect_success!(renderer.wait_idle());
        heph_expect_success!(renderer.destroy_compute_pipeline(compute_pipeline.as_mut()));
        heph_expect_success!(renderer.destroy_graphics_pipeline(graphics_pipeline.as_mut()));
    }

    pub(crate) fn test_resize_window(&self) {
        use heph_gl::{
            Shader,
            renderers::{GraphicsPipelineOptions, Msaa, Settings},
        };

        use crate::{renderer::test_suites::TEST_ENV, test_env, utils::SHADERS_DIR};

        let mut renderer = self.create_renderer_with_any_device(&[]);
        let max_msaa = *heph_expect_success!(
            renderer
                .get_device()
                .and_then(|device| device.supported_msaa_levels().last())
                .ok_or("No supported MSAA sample count.")
        );
        for msaa in [Msaa::X1, max_msaa] {
            heph_expect_success!(renderer.set_settings(Settings {
                frames_in_flight: 2,
                msaa,
                ..Default::default()
            }));
            let vertex_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/shapes/basic_triangle_vert.spv"
            )));
            let fragment_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/shapes/basic_triangle_frag.spv"
            )));
            let mut pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
                &[&vertex_shader, &fragment_shader],
                &GraphicsPipelineOptions::default()
            ));
            for (width, height) in [(640, 480), (960, 540), (320, 240), (1280, 720)] {
                test_env!().resize_window(width, height);
                for frame in 0..6 {
                    heph_expect_success!(renderer.begin_frame());
                    if frame == 0 {
                        test_env!().resize_window(width + 32, height + 32);
                    }
                    heph_expect_success!(renderer.clear(DISPLAY_CANVAS, [0.0, 0.0, 0.0]));
                    heph_expect_success!(renderer.record_graphics_command(
                        DISPLAY_CANVAS,
                        pipeline.as_mut(),
                        None,
                        &[],
                        &[],
                        3,
                        1
                    ));
                    heph_expect_success!(renderer.end_frame());
                }
            }
            heph_expect_success!(renderer.wait_idle());
            heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
        }
    }
}
