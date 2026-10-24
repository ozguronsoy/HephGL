use heph_gl::{
    Error,
    renderers::{
        Renderer,
        concurrency::{RendererHandle, RendererWorkerFactory},
    },
};

use crate::{heph_expect_err, heph_expect_success, renderer::RendererTests};

impl<TestRenderer: Renderer> RendererTests<TestRenderer>
where
    RendererHandle<TestRenderer>: RendererWorkerFactory<TestRenderer>,
{
    pub(crate) fn test_begin_twice(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);
        heph_expect_success!(renderer.begin_frame());
        heph_expect_err!(renderer.begin_frame(), Error::invalid_operation(""));
    }

    pub(crate) fn test_end_without_begin(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);
        heph_expect_err!(renderer.end_frame(), Error::invalid_operation(""));
    }

    pub(crate) fn test_resize_window(&self) {
        use heph_gl::{
            renderers::settings::{GraphicsPipelineOptions, Msaa, Settings},
            shader::Shader,
        };
        use renkrs::RGB;

        use crate::{renderer::test_suites::TEST_ENV, test_env, utils::SHADERS_DIR};

        let mut renderer = self.create_renderer_with_any_device(&[]);
        let max_msaa = *heph_expect_success!(
            heph_expect_success!(renderer.supported_msaa_list())
                .last()
                .ok_or("No supported MSAA sample count.")
        );
        for msaa in [Msaa::X1, max_msaa] {
            heph_expect_success!(renderer.set_settings(Settings {
                frames_in_flight: 2,
                msaa,
                ..Default::default()
            }));
            let vertex_shader = heph_expect_success!(Shader::from_file(format!(
                "{}/shapes/basic_triangle_vert.spv",
                SHADERS_DIR
            )));
            let fragment_shader = heph_expect_success!(Shader::from_file(format!(
                "{}/shapes/basic_triangle_frag.spv",
                SHADERS_DIR
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
                    heph_expect_success!(renderer.clear(RGB::default()));
                    heph_expect_success!(renderer.record_graphics_command(
                        pipeline.as_mut(),
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
