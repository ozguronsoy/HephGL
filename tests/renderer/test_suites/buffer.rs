use heph_gl::renderers::{
    Renderer,
    concurrency::{RendererHandle, RendererWorkerFactory},
    resources::{BufferUsage, GpuBuffer},
};

use crate::{heph_expect_err, heph_expect_success, renderer::RendererTests};

const BUFFER_SIZE: usize = 1024;

impl<TestRenderer: Renderer> RendererTests<TestRenderer>
where
    RendererHandle<TestRenderer>: RendererWorkerFactory<TestRenderer>,
{
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

    fn test_buffer<T>(&self, data: &Vec<T>, usage: BufferUsage)
    where
        T: bytemuck::Pod + PartialEq + Default + std::fmt::Debug,
    {
        let renderer = self.create_renderer_with_any_device(&[]);

        let buffer_size = data.len() * size_of::<T>();
        let mut buffer = heph_expect_success!(renderer.create_buffer(buffer_size, usage));
        assert_eq!(buffer.size(), buffer_size);

        heph_expect_success!(renderer.write_buffer(&buffer, bytemuck::cast_slice(data)));

        let mut output = vec![T::default(); data.len()];
        heph_expect_success!(renderer.read_buffer(&buffer, bytemuck::cast_slice_mut(&mut output)));

        assert_eq!(*data, output);

        heph_expect_success!(renderer.destroy_buffer(&mut buffer));
        assert_eq!(buffer.size(), 0);
    }
}
