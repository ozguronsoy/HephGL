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
}
