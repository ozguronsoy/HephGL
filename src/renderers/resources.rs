use std::any::{Any, type_name};

use crate::{Error, Result};

/// Defines the possible usages of a buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
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
#[derive(Debug, Default, Clone, Copy, PartialEq)]
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
#[derive(Clone, Copy)]
pub enum ResourceBindingType<'a> {
    /// A block of GPU memory.
    Buffer {
        /// The handle referencing the allocated buffer.
        handle: &'a dyn RendererBuffer,
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
        handle: &'a dyn RendererTexture,
    },
    /// The GPU texture sampler.
    Sampler {
        /// The handle referencing the GPU texture sampler.
        handle: &'a dyn RendererSampler,
    },
}

/// Represents a resource binding.
#[derive(Clone, Copy)]
pub struct ResourceBinding<'a> {
    /// The binding number specified in the shader (e.g., `layout(binding =
    /// 0)`).
    pub binding: u32,
    /// The actual resource this slot binds to.
    pub resource: ResourceBindingType<'a>,
}

/// Represents a resource managed by a renderer.
pub trait RendererResource: Any + Send + Sync {
    /// Indicates whether the resource currently has an allocation.
    fn is_allocated(&self) -> bool;
}

/// Stores data in a GPU.
pub trait RendererBuffer: RendererResource {
    /// Returns the size of the buffer in bytes.
    fn size(&self) -> usize;
    /// Returns the usage type of the buffer.
    fn usage(&self) -> BufferUsage;
}

/// Represents a GPU texture.
pub trait RendererTexture: RendererResource {
    /// Returns the texture width in pixels.
    fn width(&self) -> u32;
    /// Returns the texture height in pixels.
    fn height(&self) -> u32;
    /// Returns the texture format.
    fn format(&self) -> TextureFormat;
    /// Returns the number of mip levels.
    fn mip_level_count(&self) -> u32;
}

/// Represents a texture sampler.
pub trait RendererSampler: RendererResource {}

/// Represents a compiled compute pipeline.
pub trait RendererComputePipeline: RendererResource {}

/// Represents a compiled graphics pipeline.
pub trait RendererGraphicsPipeline: RendererResource {}

impl TextureFormat {
    /// Calculates the size of each pixel in bytes.
    pub(super) fn bytes_per_pixel(&self) -> usize {
        match self {
            Self::R8Unorm => 1,
            Self::Rgba8Unorm | Self::Rgba8Srgb => 4,
        }
    }
}

/// Downcasts the reference to the provided type.
pub(super) fn resource_ref<T: Any>(resource: &dyn Any) -> Result<&T> {
    resource.downcast_ref::<T>().ok_or_else(|| {
        Error::invalid_argument(format!(
            "Resource type mismatch: expected {}.",
            type_name::<T>()
        ))
    })
}

/// Downcasts the mutable reference to the provided type.
pub(super) fn resource_mut<T: Any>(resource: &mut dyn Any) -> Result<&mut T> {
    resource.downcast_mut::<T>().ok_or_else(|| {
        Error::invalid_argument(format!(
            "Resource type mismatch: expected {}.",
            type_name::<T>()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_format_bytes_per_pixel() {
        assert_eq!(TextureFormat::R8Unorm.bytes_per_pixel(), 1);
        assert_eq!(TextureFormat::Rgba8Unorm.bytes_per_pixel(), 4);
        assert_eq!(TextureFormat::Rgba8Srgb.bytes_per_pixel(), 4);
    }
}
