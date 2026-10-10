/// Represents a resource binding.
#[derive(Debug, Clone, Copy)]
pub struct ResourceBinding<B, T, S> {
    /// The binding number specified in the shader (e.g., `layout(binding =
    /// 0)`).
    pub binding: u32,
    /// The actual resource this slot binds to.
    pub resource: ResourceBindingType<B, T, S>,
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

/// Defines the pixel format of a texture.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    /// 8-bit single channel linear color value.
    R8Unorm,
    /// 8-bit RGBA using linear color values.
    Rgba8Unorm,
    /// 8-bit RGBA using the sRGB color space.
    #[default]
    Rgba8Srgb,
}

/// Defines the types of resources being bound to a shader slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceBindingType<BufferHandle, TextureHandle, SamplerHandle> {
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
    /// The GPU texture.
    Texture {
        /// The handle referencing the GPU texture.
        handle: TextureHandle,
    },
    /// The GPU texture sampler.
    Sampler {
        /// The handle referencing the GPU texture sampler.
        handle: SamplerHandle,
    },
}

/// Stores data in a GPU.
pub trait GpuBuffer: Copy + Clone + Send + Sync {
    /// Returns the size of the buffer in bytes.
    fn size(&self) -> usize;
}

/// Represents a GPU texture.
pub trait GpuTexture: Copy + Clone + Send + Sync {
    /// Returns the texture width in pixels.
    fn width(&self) -> u32;
    /// Returns the texture height in pixels.
    fn height(&self) -> u32;
    /// Returns the texture format.
    fn format(&self) -> TextureFormat;
}

impl TextureFormat {
    /// Calculates the size of each pixel in bytes.
    pub(super) fn bytes_per_pixel(&self) -> usize {
        match self {
            Self::R8Unorm => 1,
            Self::Rgba8Unorm | Self::Rgba8Srgb => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_format_bytes_per_pixel() {
        assert_eq!(TextureFormat::R8Unorm.bytes_per_pixel(), 1);
        assert_eq!(TextureFormat::Rgba8Unorm.bytes_per_pixel(), 1);
        assert_eq!(TextureFormat::Rgba8Srgb.bytes_per_pixel(), 1);
    }
}
