/// Represents a resource binding.
#[derive(Debug, Clone, Copy)]
pub struct ResourceBinding<B> {
    /// The binding number specified in the shader (e.g., `layout(binding =
    /// 0)`).
    pub binding: u32,

    /// The actual resource this slot binds to.
    pub resource: ResourceBindingType<B>,
}

/// Defines the possible usages of a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferUsage {
    /// The buffer is used to store general purpose data.
    Storage,
    /// The buffer is used to pass read-only data, such as transformation
    /// matrices or material properties, to shaders.
    Uniform,
    /// The buffer is used to store vertex data for 3D geometry.
    Vertex,
    /// The buffer is used to store index data for drawing geometry.
    Index,
}

/// Defines the types of resources being bound to a shader slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceBindingType<BufferHandle> {
    /// A block of GPU memory.
    Buffer {
        /// The handle referencing the allocated buffer.
        handle: BufferHandle,
        /// Specifies how the shader intends to use this buffer.
        usage: BufferUsage,
        /// The starting byte offset within the buffer where the binding begins.
        offset: usize,
        /// The size in bytes of the buffer region being bound.
        size: usize,
    },
}

pub enum PipelineHandle<G, C> {
    Graphics(G),
    Compute(C),
}

/// Stores data in a GPU.
pub trait GpuBuffer: std::fmt::Debug + Copy + Clone + Send + Sync {
    fn size(&self) -> usize;
}
