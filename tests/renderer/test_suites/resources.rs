use heph_gl::renderers::{
    Renderer,
    concurrency::{RendererHandle, RendererWorkerFactory},
    resources::{
        RendererBuffer, RendererComputePipeline, RendererGraphicsPipeline, RendererSampler,
        RendererTexture, TextureFormat,
    },
};

use crate::{heph_expect_err, renderer::RendererTests};

struct ForeignBuffer;
impl RendererBuffer for ForeignBuffer {
    fn size(&self) -> usize {
        4
    }
}

struct ForeignTexture;
impl RendererTexture for ForeignTexture {
    fn width(&self) -> u32 {
        1
    }
    fn height(&self) -> u32 {
        1
    }
    fn format(&self) -> TextureFormat {
        TextureFormat::Rgba8Unorm
    }
    fn mip_level_count(&self) -> u32 {
        1
    }
}

struct ForeignSampler;
impl RendererSampler for ForeignSampler {}

struct ForeignComputePipeline;
impl RendererComputePipeline for ForeignComputePipeline {}

struct ForeignGraphicsPipeline;
impl RendererGraphicsPipeline for ForeignGraphicsPipeline {}

impl<TestRenderer: Renderer> RendererTests<TestRenderer>
where
    RendererHandle<TestRenderer>: RendererWorkerFactory<TestRenderer>,
{
    pub(crate) fn test_reject_foreign_resources(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);
        let mut buffer: Box<dyn RendererBuffer> = Box::new(ForeignBuffer);
        let mut texture: Box<dyn RendererTexture> = Box::new(ForeignTexture);
        let mut sampler: Box<dyn RendererSampler> = Box::new(ForeignSampler);
        let mut compute_pipeline: Box<dyn RendererComputePipeline> =
            Box::new(ForeignComputePipeline);
        let mut graphics_pipeline: Box<dyn RendererGraphicsPipeline> =
            Box::new(ForeignGraphicsPipeline);

        heph_expect_err!(renderer.write_buffer(buffer.as_mut(), &[0; 4]));
        heph_expect_err!(renderer.read_buffer(buffer.as_ref(), &mut [0; 4]));
        heph_expect_err!(renderer.destroy_buffer(buffer.as_mut()));
        heph_expect_err!(renderer.destroy_texture(texture.as_mut()));
        heph_expect_err!(renderer.destroy_sampler(sampler.as_mut()));
        heph_expect_err!(renderer.record_compute_command(
            compute_pipeline.as_mut(),
            &[],
            (1, 1, 1)
        ));
        heph_expect_err!(renderer.destroy_compute_pipeline(compute_pipeline.as_mut()));
        heph_expect_err!(renderer.record_graphics_command(graphics_pipeline.as_mut(), &[], 3, 1));
        heph_expect_err!(renderer.destroy_graphics_pipeline(graphics_pipeline.as_mut()));
    }
}
