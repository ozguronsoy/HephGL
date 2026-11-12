use std::any::{Any, type_name};

use crate::{Error, Result};

/// Defines the possible usages of a buffer.
#[derive(Debug, Clone, Copy, Hash, PartialEq)]
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
    /// 16-bit single channel linear color value.
    R16Unorm,
    /// 8-bit RGBA using linear color values.
    Rgba8Unorm,
    /// 8-bit RGBA using the sRGB color space.
    #[default]
    Rgba8Srgb,
    /// 8-bit BGRA using the sRGB color space.
    Bgra8Srgb,
    /// BC1 block-compressed RGB with 1-bit alpha using linear color values.
    Bc1Unorm,
    /// BC1 block-compressed RGB with 1-bit alpha using the sRGB color space.
    Bc1Srgb,
    /// BC2 block-compressed RGBA using linear color values.
    Bc2Unorm,
    /// BC2 block-compressed RGBA using the sRGB color space.
    Bc2Srgb,
    /// BC3 block-compressed RGBA using linear color values.
    Bc3Unorm,
    /// BC3 block-compressed RGBA using the sRGB color space.
    Bc3Srgb,
    /// BC4 block-compressed single channel linear color value.
    Bc4Unorm,
    /// BC5 block-compressed two channel linear color values.
    Bc5Unorm,
    /// BC6H block-compressed RGB using unsigned floating-point color values.
    Bc6hUfloat,
    /// BC7 block-compressed RGBA using linear color values.
    Bc7Unorm,
    /// BC7 block-compressed RGBA using the sRGB color space.
    Bc7Srgb,
}

/// Defines the types of resources being bound to a shader slot.
#[derive(Clone, Copy)]
pub enum ResourceBindingType<'a> {
    /// A block of GPU memory.
    Buffer {
        buffer: &'a dyn RendererBuffer,
        offset: usize,
        size: usize,
    },
    /// The GPU texture.
    Texture { texture: &'a dyn RendererTexture },
    /// The GPU texture sampler.
    Sampler { sampler: &'a dyn RendererSampler },
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

impl<'a> ResourceBinding<'a> {
    /// Creates a binding for the given range of a buffer.
    pub fn buffer(
        binding: u32,
        buffer: &'a dyn RendererBuffer,
        offset: usize,
        size: usize,
    ) -> Self {
        Self {
            binding,
            resource: ResourceBindingType::Buffer {
                buffer,
                offset,
                size,
            },
        }
    }

    /// Creates a binding for a texture.
    pub fn texture(binding: u32, texture: &'a dyn RendererTexture) -> Self {
        Self {
            binding,
            resource: ResourceBindingType::Texture { texture },
        }
    }

    /// Creates a binding for a texture sampler.
    pub fn sampler(binding: u32, sampler: &'a dyn RendererSampler) -> Self {
        Self {
            binding,
            resource: ResourceBindingType::Sampler { sampler },
        }
    }
}

impl TextureFormat {
    /// Calculates the size of each block in bytes.
    pub(super) fn bytes_per_block(&self) -> usize {
        match self {
            Self::R8Unorm => 1,
            Self::R16Unorm => 2,
            Self::Rgba8Unorm | Self::Rgba8Srgb | Self::Bgra8Srgb => 4,
            Self::Bc1Unorm | Self::Bc1Srgb | Self::Bc4Unorm => 8,
            Self::Bc2Unorm
            | Self::Bc2Srgb
            | Self::Bc3Unorm
            | Self::Bc3Srgb
            | Self::Bc5Unorm
            | Self::Bc6hUfloat
            | Self::Bc7Unorm
            | Self::Bc7Srgb => 16,
        }
    }

    /// Gets the width and height of a block in pixels.
    pub(super) fn block_extent(&self) -> u32 {
        match self {
            Self::R8Unorm
            | Self::R16Unorm
            | Self::Rgba8Unorm
            | Self::Rgba8Srgb
            | Self::Bgra8Srgb => 1,
            Self::Bc1Unorm
            | Self::Bc1Srgb
            | Self::Bc2Unorm
            | Self::Bc2Srgb
            | Self::Bc3Unorm
            | Self::Bc3Srgb
            | Self::Bc4Unorm
            | Self::Bc5Unorm
            | Self::Bc6hUfloat
            | Self::Bc7Unorm
            | Self::Bc7Srgb => 4,
        }
    }
}

/// Downcasts the reference to the provided type.
pub(super) fn resource_ref<T: Any>(resource: &dyn Any) -> Result<&T> {
    resource.downcast_ref::<T>().ok_or_else(|| {
        Error::invalid_argument(format!("Type mismatch: expected {}.", type_name::<T>()))
    })
}

/// Downcasts the mutable reference to the provided type.
pub(super) fn resource_mut<T: Any>(resource: &mut dyn Any) -> Result<&mut T> {
    resource.downcast_mut::<T>().ok_or_else(|| {
        Error::invalid_argument(format!("Type mismatch: expected {}.", type_name::<T>()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_format_bytes_per_block() {
        assert_eq!(TextureFormat::R8Unorm.bytes_per_block(), 1);
        assert_eq!(TextureFormat::R16Unorm.bytes_per_block(), 2);
        assert_eq!(TextureFormat::Rgba8Unorm.bytes_per_block(), 4);
        assert_eq!(TextureFormat::Rgba8Srgb.bytes_per_block(), 4);
        assert_eq!(TextureFormat::Bgra8Srgb.bytes_per_block(), 4);
        assert_eq!(TextureFormat::Bc1Unorm.bytes_per_block(), 8);
        assert_eq!(TextureFormat::Bc1Srgb.bytes_per_block(), 8);
        assert_eq!(TextureFormat::Bc2Unorm.bytes_per_block(), 16);
        assert_eq!(TextureFormat::Bc2Srgb.bytes_per_block(), 16);
        assert_eq!(TextureFormat::Bc3Unorm.bytes_per_block(), 16);
        assert_eq!(TextureFormat::Bc3Srgb.bytes_per_block(), 16);
        assert_eq!(TextureFormat::Bc4Unorm.bytes_per_block(), 8);
        assert_eq!(TextureFormat::Bc5Unorm.bytes_per_block(), 16);
        assert_eq!(TextureFormat::Bc6hUfloat.bytes_per_block(), 16);
        assert_eq!(TextureFormat::Bc7Unorm.bytes_per_block(), 16);
        assert_eq!(TextureFormat::Bc7Srgb.bytes_per_block(), 16);
    }
}
