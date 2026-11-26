use heph_gl::{
    Error, Shader,
    renderers::{
        BufferUsage, CanvasOptions, DISPLAY_CANVAS, GraphicsPipelineOptions, Renderer,
        RendererCanvas, ResourceBinding, SamplerOptions,
    },
};

use crate::{heph_expect_err, heph_expect_success, renderer::RendererTests, utils::SHADERS_DIR};

impl<TestRenderer: Renderer> RendererTests<TestRenderer> {
    pub(crate) fn test_canvas(&self) {
        for depth_only in [false, true] {
            let renderer = self.create_renderer_with_any_device(&[]);
            let canvas_options = CanvasOptions {
                width: 640,
                height: 360,
                depth_only,
                ..Default::default()
            };

            heph_expect_success!(renderer.begin_frame());
            let mut canvas = heph_expect_success!(renderer.create_canvas(&canvas_options));
            heph_expect_success!(renderer.end_frame());
            heph_expect_success!(renderer.wait_idle());

            assert_eq!(canvas.textures().count(), 1);
            assert!(canvas.textures().all(|texture| texture.is_allocated()));
            assert_eq!(canvas.width(), canvas_options.width);
            assert_eq!(canvas.height(), canvas_options.height);
            assert_eq!(canvas.options().width, canvas_options.width);
            assert_eq!(canvas.options().height, canvas_options.height);
            assert_eq!(canvas.options().depth_only, canvas_options.depth_only);
            for texture in canvas.textures() {
                assert_eq!(texture.width(), canvas_options.width);
                assert_eq!(texture.height(), canvas_options.height);
                assert_eq!(texture.mip_level_count(), 1);
            }

            heph_expect_success!(renderer.destroy_canvas(canvas.as_mut()));
            assert!(canvas.textures().all(|texture| !texture.is_allocated()));
        }

        {
            let renderer = self.create_renderer_with_any_device(&[]);
            let canvas_options = CanvasOptions {
                width: 640,
                height: 360,
                ..Default::default()
            };

            heph_expect_err!(
                renderer.create_canvas(&canvas_options),
                Error::InvalidOperation(_)
            );
            heph_expect_success!(renderer.begin_frame());
            heph_expect_success!(renderer.end_frame());
            heph_expect_err!(
                renderer.create_canvas(&canvas_options),
                Error::InvalidOperation(_)
            );
        }

        {
            let renderer = self.create_renderer_with_any_device(&[]);
            heph_expect_success!(renderer.begin_frame());
            heph_expect_err!(
                renderer.create_canvas(&CanvasOptions {
                    width: 0,
                    height: 360,
                    ..Default::default()
                }),
                Error::InvalidArgument(_)
            );
            heph_expect_err!(
                renderer.create_canvas(&CanvasOptions {
                    width: 640,
                    height: 0,
                    ..Default::default()
                }),
                Error::InvalidArgument(_)
            );
            heph_expect_err!(
                renderer.create_canvas(&CanvasOptions {
                    width: u32::MAX,
                    height: 360,
                    ..Default::default()
                }),
                Error::InvalidArgument(_)
            );
            heph_expect_err!(
                renderer.create_canvas(&CanvasOptions {
                    width: 640,
                    height: u32::MAX,
                    ..Default::default()
                }),
                Error::InvalidArgument(_)
            );
            heph_expect_success!(renderer.end_frame());
        }
    }

    pub(crate) fn test_reject_canvas_use_after_free(&self) {
        let renderer = self.create_renderer_with_any_device(&[]);

        heph_expect_success!(renderer.begin_frame());
        let mut canvas = heph_expect_success!(renderer.create_canvas(&CanvasOptions {
            width: 640,
            height: 360,
            ..Default::default()
        }));
        heph_expect_success!(renderer.end_frame());
        heph_expect_success!(renderer.wait_idle());
        heph_expect_success!(renderer.destroy_canvas(canvas.as_mut()));

        Self::expect_canvas_rejected(&renderer, canvas.as_mut());
    }

    pub(crate) fn test_reject_canvas_use_in_other_renderer(&self) {
        let renderer = self.create_renderer_with_any_device(&[]);
        let other_renderer = self.create_renderer_with_any_device(&[]);

        heph_expect_success!(renderer.begin_frame());
        let mut canvas = heph_expect_success!(renderer.create_canvas(&CanvasOptions {
            width: 640,
            height: 360,
            ..Default::default()
        }));
        heph_expect_success!(renderer.end_frame());
        heph_expect_success!(renderer.wait_idle());

        Self::expect_canvas_rejected(&other_renderer, canvas.as_mut());
        drop(other_renderer);
        heph_expect_success!(renderer.debugger().ok_or("Debugger is not attached."))
            .logs()
            .append(&mut self.debugger.take().logs());

        heph_expect_success!(renderer.destroy_canvas(canvas.as_mut()));
    }

    fn expect_canvas_rejected(renderer: &TestRenderer, canvas: &mut dyn RendererCanvas) {
        const QUAD_VERTEX_COUNT: u32 = 6;
        const QUAD_VERTEX_BUFFER_SIZE: usize = size_of::<[f32; 4]>() * QUAD_VERTEX_COUNT as usize;

        let triangle_vert_shader = heph_expect_success!(Shader::from_file(format!(
            "{SHADERS_DIR}/shapes/basic_triangle_vert.spv"
        )));
        let triangle_frag_shader = heph_expect_success!(Shader::from_file(format!(
            "{SHADERS_DIR}/shapes/basic_triangle_frag.spv"
        )));
        let canvas_vert_shader =
            heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/canvas_vert.spv")));
        let canvas_frag_shader =
            heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/canvas_frag.spv")));
        let mut triangle_pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
            &[&triangle_vert_shader, &triangle_frag_shader],
            &GraphicsPipelineOptions::default()
        ));
        let mut canvas_pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
            &[&canvas_vert_shader, &canvas_frag_shader],
            &GraphicsPipelineOptions::default()
        ));
        let mut vertex_buffer = heph_expect_success!(
            renderer.create_buffer(QUAD_VERTEX_BUFFER_SIZE, BufferUsage::Vertex)
        );
        let mut sampler = heph_expect_success!(renderer.create_sampler(&SamplerOptions::default()));

        heph_expect_success!(renderer.begin_frame());
        heph_expect_err!(
            renderer.clear(Some(&mut *canvas), [0.0, 0.0, 0.0]),
            Error::InvalidArgument(_)
        );
        heph_expect_err!(
            renderer.record_graphics_command(
                Some(&mut *canvas),
                triangle_pipeline.as_mut(),
                None,
                &[],
                &[],
                3,
                1
            ),
            Error::InvalidArgument(_)
        );
        heph_expect_err!(
            renderer.record_graphics_command(
                DISPLAY_CANVAS,
                canvas_pipeline.as_mut(),
                None,
                &[ResourceBinding::buffer(
                    0,
                    vertex_buffer.as_ref(),
                    0,
                    QUAD_VERTEX_BUFFER_SIZE
                )],
                &[&[
                    ResourceBinding::texture(
                        0,
                        heph_expect_success!(
                            canvas.textures().next().ok_or("Canvas has no texture.")
                        )
                    ),
                    ResourceBinding::sampler(1, sampler.as_ref()),
                ]],
                QUAD_VERTEX_COUNT,
                1
            ),
            Error::InvalidArgument(_)
        );
        heph_expect_success!(renderer.end_frame());

        heph_expect_err!(renderer.destroy_canvas(canvas), Error::InvalidArgument(_));

        heph_expect_success!(renderer.wait_idle());
        heph_expect_success!(renderer.destroy_buffer(vertex_buffer.as_mut()));
        heph_expect_success!(renderer.destroy_sampler(sampler.as_mut()));
        heph_expect_success!(renderer.destroy_graphics_pipeline(triangle_pipeline.as_mut()));
        heph_expect_success!(renderer.destroy_graphics_pipeline(canvas_pipeline.as_mut()));
    }
}
