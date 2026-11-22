use heph_gl::{
    Error,
    renderers::{CanvasOptions, Renderer},
};

use crate::{heph_expect_err, heph_expect_success, renderer::RendererTests};

impl<TestRenderer: Renderer> RendererTests<TestRenderer> {
    pub(crate) fn test_canvas(&self) {
        for depth_only in [false, true] {
            let mut renderer = self.create_renderer_with_any_device(&[]);
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
            let mut renderer = self.create_renderer_with_any_device(&[]);
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
            let mut renderer = self.create_renderer_with_any_device(&[]);
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
}
