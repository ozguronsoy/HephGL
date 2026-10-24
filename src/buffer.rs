use crate::{
    Result,
    renderers::{
        Renderer,
        resources::{BufferUsage, RendererBuffer},
    },
};

/// An automatically managed buffer allocated by a renderer.
pub struct Buffer<'a> {
    renderer: &'a dyn Renderer,
    renderer_buffer: Box<dyn RendererBuffer>,
}

impl<'a> Buffer<'a> {
    /// Creates an uninitialized buffer with the specified size and usage.
    pub fn new(renderer: &'a dyn Renderer, size: usize, usage: BufferUsage) -> Result<Self> {
        Ok(Self {
            renderer,
            renderer_buffer: renderer.create_buffer(size, usage)?,
        })
    }

    /// Wraps an existing buffer.
    pub fn from_renderer_buffer(
        renderer: &'a dyn Renderer,
        renderer_buffer: Box<dyn RendererBuffer>,
    ) -> Self {
        Self {
            renderer,
            renderer_buffer,
        }
    }

    /// Creates a buffer initialized with the provided bytes.
    pub fn from_slice(renderer: &'a dyn Renderer, data: &[u8], usage: BufferUsage) -> Result<Self> {
        let mut buffer = Buffer::new(renderer, size_of_val(data), usage)?;
        renderer.write_buffer(buffer.renderer_buffer.as_mut(), 0, data)?;
        Ok(buffer)
    }

    /// Returns the renderer that owns this buffer.
    pub fn renderer(&self) -> &'a dyn Renderer {
        self.renderer
    }

    /// Returns the underlying renderer buffer.
    pub fn renderer_buffer(&self) -> &dyn RendererBuffer {
        self.renderer_buffer.as_ref()
    }

    /// Returns the underlying renderer buffer.
    pub fn renderer_buffer_mut(&mut self) -> &mut dyn RendererBuffer {
        self.renderer_buffer.as_mut()
    }

    /// Reads bytes from the buffer at the specified offset.
    pub fn read(&self, offset: usize, dest: &mut [u8]) -> Result<()> {
        self.renderer
            .read_buffer(self.renderer_buffer.as_ref(), offset, dest)
    }

    /// Writes bytes to the buffer at the specified offset.
    pub fn write(&mut self, offset: usize, src: &[u8]) -> Result<()> {
        self.renderer
            .write_buffer(self.renderer_buffer.as_mut(), offset, src)
    }
}

impl<'a> Drop for Buffer<'a> {
    fn drop(&mut self) {
        if self.renderer_buffer.is_allocated() {
            self.renderer
                .destroy_buffer(self.renderer_buffer.as_mut())
                .unwrap();
        }
    }
}
