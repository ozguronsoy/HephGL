use heph_gl::{
    Error,
    renderers::{
        Renderer,
        concurrency::{RendererHandle, RendererWorkerFactory},
        resources::{
            BufferUsage, RendererBuffer, RendererComputePipeline, RendererGraphicsPipeline,
            RendererResource, RendererSampler, RendererTexture, TextureFormat,
        },
    },
};

use crate::{heph_expect_err, heph_expect_success, renderer::RendererTests};

const BUFFER_SIZE: usize = 1024;

struct ForeignBuffer;
impl RendererResource for ForeignBuffer {
    fn is_allocated(&self) -> bool {
        false
    }
}
impl RendererBuffer for ForeignBuffer {
    fn size(&self) -> usize {
        4
    }
    fn usage(&self) -> BufferUsage {
        BufferUsage::Storage
    }
}

struct ForeignTexture;
impl RendererResource for ForeignTexture {
    fn is_allocated(&self) -> bool {
        false
    }
}
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
impl RendererResource for ForeignSampler {
    fn is_allocated(&self) -> bool {
        false
    }
}
impl RendererSampler for ForeignSampler {}

struct ForeignComputePipeline;
impl RendererResource for ForeignComputePipeline {
    fn is_allocated(&self) -> bool {
        false
    }
}
impl RendererComputePipeline for ForeignComputePipeline {}

struct ForeignGraphicsPipeline;
impl RendererResource for ForeignGraphicsPipeline {
    fn is_allocated(&self) -> bool {
        false
    }
}
impl RendererGraphicsPipeline for ForeignGraphicsPipeline {}

impl<TestRenderer: Renderer> RendererTests<TestRenderer>
where
    RendererHandle<TestRenderer>: RendererWorkerFactory<TestRenderer>,
{
    pub(crate) fn test_buffer_io(&self) {
        let renderer = self.create_renderer_with_any_device(&[]);

        let data = [1u32, 2, 3, 4, 5];
        let buffer_size = data.len() * size_of::<u32>();
        let create_buffer = || {
            let mut buffer =
                heph_expect_success!(renderer.create_buffer(buffer_size, BufferUsage::Storage));
            heph_expect_success!(renderer.write_buffer(
                buffer.as_mut(),
                0,
                bytemuck::cast_slice(&data)
            ));
            buffer
        };

        {
            let mut result = [0u32; 5];
            let mut buffer = create_buffer();
            heph_expect_success!(renderer.read_buffer(
                buffer.as_ref(),
                0,
                bytemuck::cast_slice_mut(&mut result)
            ));
            assert_eq!(data, result);
            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
        }

        {
            let mut result = [0u32; 2];
            let mut buffer = create_buffer();
            heph_expect_success!(renderer.read_buffer(
                buffer.as_ref(),
                2 * size_of::<u32>(),
                bytemuck::cast_slice_mut(&mut result)
            ));
            assert_eq!(result, [3, 4]);
            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
        }

        {
            let new_data = [5u32, 4, 3, 2, 1];
            let mut result = [0u32; 5];
            let mut buffer = create_buffer();
            heph_expect_success!(renderer.write_buffer(
                buffer.as_mut(),
                0,
                bytemuck::cast_slice(&new_data)
            ));
            heph_expect_success!(renderer.read_buffer(
                buffer.as_ref(),
                0,
                bytemuck::cast_slice_mut(&mut result)
            ));
            assert_eq!(result, new_data);
            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
        }

        {
            let new_data = [32u32, 86];
            let mut result = [0u32; 5];
            let mut buffer = create_buffer();
            heph_expect_success!(renderer.write_buffer(
                buffer.as_mut(),
                3 * size_of::<u32>(),
                bytemuck::cast_slice(&new_data)
            ));
            heph_expect_success!(renderer.read_buffer(
                buffer.as_ref(),
                0,
                bytemuck::cast_slice_mut(&mut result)
            ));
            assert_eq!(result, [1u32, 2, 3, 32, 86]);
            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
        }

        {
            let mut result = [0u32; 10];
            let mut buffer = create_buffer();
            heph_expect_err!(
                renderer.read_buffer(buffer.as_ref(), 0, bytemuck::cast_slice_mut(&mut result)),
                Error::fail("")
            );
            heph_expect_err!(
                renderer.read_buffer(
                    buffer.as_ref(),
                    2 * size_of::<u32>(),
                    bytemuck::cast_slice_mut(&mut result[..5])
                ),
                Error::fail("")
            );
            heph_expect_err!(
                renderer.read_buffer(
                    buffer.as_ref(),
                    5 * size_of::<u32>(),
                    bytemuck::cast_slice_mut(&mut result[..5])
                ),
                Error::invalid_argument("")
            );
            heph_expect_err!(
                renderer.read_buffer(
                    buffer.as_ref(),
                    6 * size_of::<u32>(),
                    bytemuck::cast_slice_mut(&mut result[..5])
                ),
                Error::invalid_argument("")
            );
            heph_expect_err!(
                renderer.read_buffer(
                    buffer.as_ref(),
                    2 * size_of::<u32>(),
                    bytemuck::cast_slice_mut(&mut result[..0])
                ),
                Error::invalid_argument("")
            );
            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
        }

        {
            let new_data = [0u32; 10];
            let mut buffer = create_buffer();
            heph_expect_err!(
                renderer.write_buffer(buffer.as_mut(), 0, bytemuck::cast_slice(&new_data)),
                Error::fail("")
            );
            heph_expect_err!(
                renderer.write_buffer(
                    buffer.as_mut(),
                    2 * size_of::<u32>(),
                    bytemuck::cast_slice(&new_data[..5])
                ),
                Error::fail("")
            );
            heph_expect_err!(
                renderer.write_buffer(
                    buffer.as_mut(),
                    5 * size_of::<u32>(),
                    bytemuck::cast_slice(&new_data[..1])
                ),
                Error::invalid_argument("")
            );
            heph_expect_err!(
                renderer.write_buffer(
                    buffer.as_mut(),
                    6 * size_of::<u32>(),
                    bytemuck::cast_slice(&new_data[..1])
                ),
                Error::invalid_argument("")
            );
            heph_expect_err!(
                renderer.write_buffer(
                    buffer.as_mut(),
                    2 * size_of::<u32>(),
                    bytemuck::cast_slice(&new_data[..0])
                ),
                Error::invalid_argument("")
            );
            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
        }
    }

    pub(crate) fn test_uniform_buffer(&self) {
        let mut data = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            data.push(i + 1);
        }
        self.test_buffer(&data, BufferUsage::Uniform);
    }

    pub(crate) fn test_storage_buffer(&self) {
        let mut data = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            data.push(i + 1);
        }
        self.test_buffer(&data, BufferUsage::Storage);
    }

    pub(crate) fn test_index_buffer(&self) {
        let mut data = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            data.push(i + 1);
        }
        self.test_buffer(&data, BufferUsage::Index);
    }

    pub(crate) fn test_vertex_buffer(&self) {
        let mut data = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            data.push(i + 1);
        }
        self.test_buffer(&data, BufferUsage::Vertex);
    }

    pub(crate) fn test_impossible_buffer_size(&self) {
        let renderer = self.create_renderer_with_any_device(&[]);
        let buffer_size = 1024 * 1024 * 1024 * 1024; // 1 TB
        heph_expect_err!(renderer.create_buffer(buffer_size, BufferUsage::Storage));
    }

    pub(crate) fn test_reject_foreign_resources(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);
        let mut buffer: Box<dyn RendererBuffer> = Box::new(ForeignBuffer);
        let mut texture: Box<dyn RendererTexture> = Box::new(ForeignTexture);
        let mut sampler: Box<dyn RendererSampler> = Box::new(ForeignSampler);
        let mut compute_pipeline: Box<dyn RendererComputePipeline> =
            Box::new(ForeignComputePipeline);
        let mut graphics_pipeline: Box<dyn RendererGraphicsPipeline> =
            Box::new(ForeignGraphicsPipeline);

        heph_expect_err!(
            renderer.write_buffer(buffer.as_mut(), 0, &[0; 4]),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            renderer.read_buffer(buffer.as_ref(), 0, &mut [0; 4]),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            renderer.destroy_buffer(buffer.as_mut()),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            renderer.destroy_texture(texture.as_mut()),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            renderer.destroy_sampler(sampler.as_mut()),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            renderer.record_compute_command(compute_pipeline.as_mut(), &[], (1, 1, 1)),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            renderer.destroy_compute_pipeline(compute_pipeline.as_mut()),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            renderer.record_graphics_command(graphics_pipeline.as_mut(), &[], 3, 1),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            renderer.destroy_graphics_pipeline(graphics_pipeline.as_mut()),
            Error::invalid_argument("")
        );
    }

    fn test_buffer<T>(&self, data: &Vec<T>, usage: BufferUsage)
    where
        T: bytemuck::Pod + PartialEq + Default + std::fmt::Debug,
    {
        let renderer = self.create_renderer_with_any_device(&[]);

        let buffer_size = data.len() * size_of::<T>();
        let mut buffer = heph_expect_success!(renderer.create_buffer(buffer_size, usage));
        assert!(buffer.is_allocated());
        assert_eq!(buffer.size(), buffer_size);
        assert_eq!(buffer.usage(), usage);

        heph_expect_success!(renderer.write_buffer(buffer.as_mut(), 0, bytemuck::cast_slice(data)));

        let mut output = vec![T::default(); data.len()];
        heph_expect_success!(renderer.read_buffer(
            buffer.as_ref(),
            0,
            bytemuck::cast_slice_mut(&mut output)
        ));

        assert_eq!(*data, output);

        heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
        assert_eq!(buffer.size(), 0);
        assert!(!buffer.is_allocated());
    }
}
