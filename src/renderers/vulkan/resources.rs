use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    ops::Range,
};

use ash::vk::{Buffer, DescriptorSet, DescriptorType, Handle};

use crate::{
    Error, Result,
    renderers::{
        debug::RendererDebugObject,
        downcast_ref,
        id::RendererId,
        resources::{
            BufferUsage, CompareOp, RendererBuffer, RendererResource, RendererSampler,
            RendererTexture, ResourceBinding, ResourceBindingType, SamplerAddressMode,
            SamplerFilter, TextureFormat, TextureOptions,
        },
        vulkan::{
            VulkanRenderer,
            frame::{ComputeFrame, GraphicsFrame},
            pipeline::{VulkanComputePipeline, VulkanGraphicsPipeline},
            queue::QueueContext,
        },
    },
    shader::{ShaderBindingType, ShaderStage},
};

#[derive(Clone)]
pub(super) struct DescriptorSetCache {
    pub(super) descriptor_sets: Vec<DescriptorSet>,
    chunk_indices: HashMap<u64, usize>,
    generation: Option<u64>,
    /// This is used to detect when the pipeline is cloned to a worker thread so we can clean the
    /// cache to avoid using the resources of another thread.
    thread_id: std::thread::ThreadId,
}

/// Represents a Vulkan buffer.
#[derive(Copy, Clone)]
pub(super) struct VulkanBuffer {
    pub(super) renderer_id: RendererId,
    pub(super) buffer: Buffer,
    pub(super) vma_allocation: vk_mem::Allocation,
    pub(super) size: usize,
    pub(super) usage: BufferUsage,
}

/// Represents a Vulkan texture.
#[derive(Copy, Clone)]
pub(super) struct VulkanTexture {
    pub(super) renderer_id: RendererId,
    pub(super) image: ash::vk::Image,
    pub(super) image_view: ash::vk::ImageView,
    pub(super) vma_allocation: vk_mem::Allocation,
    pub(super) texture_options: TextureOptions,
}

/// Represents a Vulkan texture sampler.
#[derive(Copy, Clone)]
pub(super) struct VulkanSampler {
    pub(super) renderer_id: RendererId,
    pub(super) sampler: ash::vk::Sampler,
}

impl RendererDebugObject for VulkanSampler {}
impl RendererResource for VulkanSampler {
    fn is_allocated(&self) -> bool {
        !self.sampler.is_null()
    }
}
impl RendererSampler for VulkanSampler {}

impl RendererDebugObject for VulkanTexture {}
impl RendererResource for VulkanTexture {
    fn is_allocated(&self) -> bool {
        !(self.image.is_null() || self.image_view.is_null())
    }
}
impl RendererTexture for VulkanTexture {
    fn width(&self) -> u32 {
        self.texture_options.width
    }

    fn height(&self) -> u32 {
        self.texture_options.height
    }

    fn format(&self) -> TextureFormat {
        self.texture_options.format
    }

    fn mip_level_count(&self) -> u32 {
        self.texture_options.mip_level_count
    }
}

impl RendererDebugObject for VulkanBuffer {}
impl RendererResource for VulkanBuffer {
    fn is_allocated(&self) -> bool {
        !self.buffer.is_null()
    }
}
impl RendererBuffer for VulkanBuffer {
    fn size(&self) -> usize {
        self.size
    }

    fn usage(&self) -> BufferUsage {
        self.usage
    }
}

impl DescriptorSetCache {
    pub(super) fn new() -> Self {
        Self {
            descriptor_sets: Vec::new(),
            chunk_indices: HashMap::new(),
            generation: None,
            thread_id: std::thread::current().id(),
        }
    }
}

impl VulkanRenderer {
    pub(super) fn create_compute_resource_sets(
        device: &ash::Device,
        queue_context: &QueueContext<ComputeFrame>,
        frame_index: usize,
        pipeline: &mut VulkanComputePipeline,
        binding_sets: &[&[ResourceBinding<'_>]],
    ) -> Result<Range<usize>> {
        let thread_context_index = Self::thread_context_index()?;
        let pool_generation =
            unsafe { &*queue_context.frames[0].base.thread_contexts[thread_context_index].get() }
                .descriptor_pool_manager
                .generation();

        if pipeline.descriptor_layouts.is_empty()
            || pipeline.descriptor_binding_counts.iter().sum::<usize>() == 0
        {
            return Ok(0..0);
        }
        if pipeline.descriptor_set_cache.thread_id != std::thread::current().id() {
            // The pipeline is cloned to a worker thread, we need to clear the descriptor sets
            // since they are not valid for this thread.
            pipeline.descriptor_set_cache.descriptor_sets.clear();
            pipeline.descriptor_set_cache.chunk_indices.clear();
            pipeline.descriptor_set_cache.generation = Some(pool_generation);
            pipeline.descriptor_set_cache.thread_id = std::thread::current().id();
        }
        if pipeline.descriptor_set_cache.generation != Some(pool_generation) {
            pipeline.descriptor_set_cache.descriptor_sets.clear();
            pipeline.descriptor_set_cache.chunk_indices.clear();
            pipeline.descriptor_set_cache.generation = Some(pool_generation);
        }
        if !Self::validate_cache_consistency(
            pipeline.descriptor_layouts.len(),
            &pipeline.descriptor_set_cache,
        ) {
            // The descriptor sets pipeline is corrupted somehow.
            pipeline.descriptor_set_cache.descriptor_sets.clear();
            pipeline.descriptor_set_cache.chunk_indices.clear();
        }

        Self::validate_binding_sets(
            &pipeline.descriptor_layouts,
            &pipeline.descriptor_binding_counts,
            pipeline.renderer_id,
            binding_sets,
        )?;
        let hash = Self::hash_resource_bindings(binding_sets)?;

        if let Some(range) = Self::get_cached_descriptor_sets(
            pipeline.descriptor_layouts.len(),
            &pipeline.descriptor_set_cache,
            hash,
        ) {
            return Ok(range);
        }

        let descriptor_sets = unsafe {
            &mut *queue_context.frames[frame_index].base.thread_contexts[thread_context_index].get()
        }
        .descriptor_pool_manager
        .allocate(device, &pipeline.descriptor_layouts)?;
        let range = Self::extend_cached_descriptor_sets(
            pipeline.descriptor_layouts.len(),
            &mut pipeline.descriptor_set_cache,
            hash,
            &descriptor_sets,
        )?;
        Self::write_descriptor_sets(device, &descriptor_sets, binding_sets)?;

        Ok(range)
    }

    pub(super) fn create_graphics_resource_sets(
        device: &ash::Device,
        queue_context: &QueueContext<GraphicsFrame>,
        frame_index: usize,
        pipeline: &mut VulkanGraphicsPipeline,
        binding_sets: &[&[ResourceBinding<'_>]],
    ) -> Result<Range<usize>> {
        let thread_context_index = Self::thread_context_index()?;
        let pool_generation =
            unsafe { &*queue_context.frames[0].base.thread_contexts[thread_context_index].get() }
                .descriptor_pool_manager
                .generation();

        if pipeline.descriptor_layouts.is_empty()
            || pipeline.descriptor_binding_counts.iter().sum::<usize>() == 0
        {
            return Ok(0..0);
        }
        if pipeline.descriptor_set_cache.thread_id != std::thread::current().id() {
            // The pipeline is cloned to a worker thread, we need to clear the descriptor sets
            // since they are not valid for this thread.
            pipeline.descriptor_set_cache.descriptor_sets.clear();
            pipeline.descriptor_set_cache.chunk_indices.clear();
            pipeline.descriptor_set_cache.generation = Some(pool_generation);
            pipeline.descriptor_set_cache.thread_id = std::thread::current().id();
        }
        if pipeline.descriptor_set_cache.generation != Some(pool_generation) {
            pipeline.descriptor_set_cache.descriptor_sets.clear();
            pipeline.descriptor_set_cache.chunk_indices.clear();
            pipeline.descriptor_set_cache.generation = Some(pool_generation);
        }
        if !Self::validate_cache_consistency(
            pipeline.descriptor_layouts.len(),
            &pipeline.descriptor_set_cache,
        ) {
            // The descriptor sets pipeline is corrupted somehow.
            pipeline.descriptor_set_cache.descriptor_sets.clear();
            pipeline.descriptor_set_cache.chunk_indices.clear();
        }

        Self::validate_binding_sets(
            &pipeline.descriptor_layouts,
            &pipeline.descriptor_binding_counts,
            pipeline.renderer_id,
            binding_sets,
        )?;
        let hash = Self::hash_resource_bindings(binding_sets)?;

        if let Some(range) = Self::get_cached_descriptor_sets(
            pipeline.descriptor_layouts.len(),
            &pipeline.descriptor_set_cache,
            hash,
        ) {
            return Ok(range);
        }

        let descriptor_sets = unsafe {
            &mut *queue_context.frames[frame_index].base.thread_contexts[thread_context_index].get()
        }
        .descriptor_pool_manager
        .allocate(device, &pipeline.descriptor_layouts)?;
        let range = Self::extend_cached_descriptor_sets(
            pipeline.descriptor_layouts.len(),
            &mut pipeline.descriptor_set_cache,
            hash,
            &descriptor_sets,
        )?;
        Self::write_descriptor_sets(device, &descriptor_sets, binding_sets)?;

        Ok(range)
    }

    fn validate_cache_consistency(layout_count: usize, cache: &DescriptorSetCache) -> bool {
        let descriptor_sets = cache.descriptor_sets.chunks_exact(layout_count);
        descriptor_sets.remainder().is_empty() && descriptor_sets.len() == cache.chunk_indices.len()
    }

    fn validate_binding_sets(
        descriptor_layouts: &[ash::vk::DescriptorSetLayout],
        descriptor_binding_counts: &[usize],
        renderer_id: RendererId,
        binding_sets: &[&[ResourceBinding<'_>]],
    ) -> Result<()> {
        for binding in binding_sets.iter().flat_map(|&s| s.iter()) {
            match binding.resource {
                ResourceBindingType::Buffer {
                    buffer,
                    offset,
                    size,
                } => {
                    let buffer = downcast_ref::<VulkanBuffer>(buffer)?;
                    if buffer.renderer_id != renderer_id {
                        return Err(Error::invalid_argument(
                            "Cannot bind a buffer created by another renderer or old device.",
                        ));
                    }
                    if !buffer.is_allocated() {
                        return Err(Error::invalid_argument("Cannot bind a destroyed buffer."));
                    }
                    if offset.checked_add(size).is_none_or(|x| x > buffer.size()) {
                        return Err(Error::invalid_argument(
                            "Buffer overflow while binding resources.",
                        ));
                    }
                    if matches!(buffer.usage(), BufferUsage::Index | BufferUsage::Vertex) {
                        return Err(Error::invalid_argument(
                            "Index and vertex buffers are not allowed in binding sets.",
                        ));
                    }
                }
                ResourceBindingType::Texture { texture } => {
                    let texture = downcast_ref::<VulkanTexture>(texture)?;
                    if texture.renderer_id != renderer_id {
                        return Err(Error::invalid_argument(
                            "Cannot bind a texture created by another renderer or old device.",
                        ));
                    }
                    if !texture.is_allocated() {
                        return Err(Error::invalid_argument("Cannot bind a destroyed texture."));
                    }
                }
                ResourceBindingType::Sampler { sampler } => {
                    let sampler = downcast_ref::<VulkanSampler>(sampler)?;
                    if sampler.renderer_id != renderer_id {
                        return Err(Error::invalid_argument(
                            "Cannot bind a sampler created by another renderer or old device.",
                        ));
                    }
                    if !sampler.is_allocated() {
                        return Err(Error::invalid_argument("Cannot bind a destroyed sampler."));
                    }
                }
            }
        }
        if binding_sets.len() != descriptor_layouts.len()
            || binding_sets
                .iter()
                .zip(descriptor_binding_counts)
                .any(|(&binding_set, &binding_count)| binding_set.len() != binding_count)
        {
            return Err(Error::invalid_argument(
                "Binding sets do not match the pipeline layout.",
            ));
        }
        // TODO: Verify binding types.
        Ok(())
    }

    fn hash_resource_bindings(binding_sets: &[&[ResourceBinding<'_>]]) -> Result<u64> {
        let mut hasher = DefaultHasher::new();
        for binding in binding_sets.iter().flat_map(|&s| s.iter()) {
            binding.binding.hash(&mut hasher);
            match binding.resource {
                ResourceBindingType::Buffer {
                    buffer,
                    offset,
                    size,
                } => {
                    let Ok(buffer) = downcast_ref::<VulkanBuffer>(buffer) else {
                        return Err(Error::invalid_argument(
                            "Invalid buffer type, expected a VulkanBuffer.",
                        ));
                    };
                    buffer.buffer.hash(&mut hasher);
                    buffer.usage.hash(&mut hasher);
                    offset.hash(&mut hasher);
                    size.hash(&mut hasher);
                }
                ResourceBindingType::Texture { texture } => {
                    let Ok(texture) = downcast_ref::<VulkanTexture>(texture) else {
                        return Err(Error::invalid_argument(
                            "Invalid buffer type, expected a VulkanTexture.",
                        ));
                    };
                    texture.image.hash(&mut hasher);
                    texture.image_view.hash(&mut hasher);
                }
                ResourceBindingType::Sampler { sampler } => {
                    let Ok(sampler) = downcast_ref::<VulkanSampler>(sampler) else {
                        return Err(Error::invalid_argument(
                            "Invalid buffer type, expected a VulkanSampler.",
                        ));
                    };
                    sampler.sampler.hash(&mut hasher);
                }
            }
        }
        Ok(hasher.finish())
    }

    fn get_cached_descriptor_sets(
        layout_count: usize,
        cache: &DescriptorSetCache,
        hash: u64,
    ) -> Option<Range<usize>> {
        // The cache size is validated in an earlier step, so overflow is not possible.
        let start = cache.chunk_indices.get(&hash)? * layout_count;
        Some(start..start + layout_count)
    }

    /// Appends the descriptor sets to the cache and returns their range in
    /// `cache.descriptor_sets`.
    fn extend_cached_descriptor_sets(
        layout_count: usize,
        cache: &mut DescriptorSetCache,
        hash: u64,
        descriptor_sets: &[ash::vk::DescriptorSet],
    ) -> Result<Range<usize>> {
        let index = cache.descriptor_sets.chunks_exact(layout_count).len();
        let start = cache.descriptor_sets.len();
        cache.descriptor_sets.extend(descriptor_sets);
        let _ = cache.chunk_indices.insert(hash, index);
        Ok(start..cache.descriptor_sets.len())
    }

    fn write_descriptor_sets(
        logical_device: &ash::Device,
        descriptor_sets: &[DescriptorSet],
        binding_sets: &[&[ResourceBinding<'_>]],
    ) -> Result<()> {
        for (&descriptor_set, &binding_set) in descriptor_sets.iter().zip(binding_sets) {
            for binding in binding_set {
                let write = ash::vk::WriteDescriptorSet::default()
                    .dst_set(descriptor_set)
                    .dst_binding(binding.binding);
                match binding.resource {
                    ResourceBindingType::Buffer {
                        buffer,
                        offset,
                        size,
                    } => {
                        let descriptor_type = match buffer.usage() {
                            BufferUsage::Storage => ash::vk::DescriptorType::STORAGE_BUFFER,
                            BufferUsage::Uniform => ash::vk::DescriptorType::UNIFORM_BUFFER,
                            _ => unreachable!(),
                        };
                        let buffer = downcast_ref::<VulkanBuffer>(buffer)?;
                        let info = ash::vk::DescriptorBufferInfo::default()
                            .buffer(buffer.buffer)
                            .offset(offset as u64)
                            .range(size as u64);
                        let write = write
                            .descriptor_type(descriptor_type)
                            .buffer_info(std::slice::from_ref(&info));
                        unsafe { logical_device.update_descriptor_sets(&[write], &[]) };
                    }
                    ResourceBindingType::Texture { texture } => {
                        let texture = downcast_ref::<VulkanTexture>(texture)?;
                        let info = ash::vk::DescriptorImageInfo::default()
                            .image_view(texture.image_view)
                            .image_layout(ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                        let write = write
                            .descriptor_type(ash::vk::DescriptorType::SAMPLED_IMAGE)
                            .image_info(std::slice::from_ref(&info));
                        unsafe { logical_device.update_descriptor_sets(&[write], &[]) };
                    }
                    ResourceBindingType::Sampler { sampler } => {
                        let sampler = downcast_ref::<VulkanSampler>(sampler)?;
                        let info = ash::vk::DescriptorImageInfo::default().sampler(sampler.sampler);
                        let write = write
                            .descriptor_type(ash::vk::DescriptorType::SAMPLER)
                            .image_info(std::slice::from_ref(&info));
                        unsafe { logical_device.update_descriptor_sets(&[write], &[]) };
                    }
                }
            }
        }

        Ok(())
    }
}

impl From<ShaderBindingType> for DescriptorType {
    fn from(value: ShaderBindingType) -> Self {
        match value {
            ShaderBindingType::UniformBuffer => DescriptorType::UNIFORM_BUFFER,
            ShaderBindingType::StorageBuffer => DescriptorType::STORAGE_BUFFER,
            ShaderBindingType::Texture => DescriptorType::SAMPLED_IMAGE,
            ShaderBindingType::Sampler => DescriptorType::SAMPLER,
        }
    }
}

impl From<ShaderStage> for ash::vk::ShaderStageFlags {
    fn from(value: ShaderStage) -> Self {
        match value {
            ShaderStage::Vertex => ash::vk::ShaderStageFlags::VERTEX,
            ShaderStage::TessellationControl => ash::vk::ShaderStageFlags::TESSELLATION_CONTROL,
            ShaderStage::TessellationEvaluation => {
                ash::vk::ShaderStageFlags::TESSELLATION_EVALUATION
            }
            ShaderStage::Geometry => ash::vk::ShaderStageFlags::GEOMETRY,
            ShaderStage::Task => ash::vk::ShaderStageFlags::TASK_EXT,
            ShaderStage::Mesh => ash::vk::ShaderStageFlags::MESH_EXT,
            ShaderStage::Fragment => ash::vk::ShaderStageFlags::FRAGMENT,
            ShaderStage::Compute => ash::vk::ShaderStageFlags::COMPUTE,
        }
    }
}

impl From<TextureFormat> for ash::vk::Format {
    fn from(value: TextureFormat) -> Self {
        match value {
            TextureFormat::R8Unorm => Self::R8_UNORM,
            TextureFormat::R16Unorm => Self::R16_UNORM,
            TextureFormat::Rgba8Unorm => Self::R8G8B8A8_UNORM,
            TextureFormat::Rgba8Srgb => Self::R8G8B8A8_SRGB,
            TextureFormat::Bgra8Srgb => Self::B8G8R8A8_SRGB,
            TextureFormat::Rgb10a2Unorm => Self::A2B10G10R10_UNORM_PACK32,
            TextureFormat::Bgr10a2Unorm => Self::A2R10G10B10_UNORM_PACK32,
            TextureFormat::Rgba16Sfloat => Self::R16G16B16A16_SFLOAT,
            TextureFormat::Rg16Sfloat => Self::R16G16_SFLOAT,
            TextureFormat::R32Sfloat => Self::R32_SFLOAT,
            TextureFormat::Bc1Unorm => Self::BC1_RGBA_UNORM_BLOCK,
            TextureFormat::Bc1Srgb => Self::BC1_RGBA_SRGB_BLOCK,
            TextureFormat::Bc2Unorm => Self::BC2_UNORM_BLOCK,
            TextureFormat::Bc2Srgb => Self::BC2_SRGB_BLOCK,
            TextureFormat::Bc3Unorm => Self::BC3_UNORM_BLOCK,
            TextureFormat::Bc3Srgb => Self::BC3_SRGB_BLOCK,
            TextureFormat::Bc4Unorm => Self::BC4_UNORM_BLOCK,
            TextureFormat::Bc5Unorm => Self::BC5_UNORM_BLOCK,
            TextureFormat::Bc6hUfloat => Self::BC6H_UFLOAT_BLOCK,
            TextureFormat::Bc7Unorm => Self::BC7_UNORM_BLOCK,
            TextureFormat::Bc7Srgb => Self::BC7_SRGB_BLOCK,
            TextureFormat::D32Sfloat => Self::D32_SFLOAT,
        }
    }
}

impl TryFrom<ash::vk::Format> for TextureFormat {
    type Error = Error;

    fn try_from(value: ash::vk::Format) -> Result<Self> {
        match value {
            ash::vk::Format::R8_UNORM => Ok(Self::R8Unorm),
            ash::vk::Format::R16_UNORM => Ok(Self::R16Unorm),
            ash::vk::Format::R8G8B8A8_UNORM => Ok(Self::Rgba8Unorm),
            ash::vk::Format::R8G8B8A8_SRGB => Ok(Self::Rgba8Srgb),
            ash::vk::Format::B8G8R8A8_SRGB => Ok(Self::Bgra8Srgb),
            ash::vk::Format::A2B10G10R10_UNORM_PACK32 => Ok(Self::Rgb10a2Unorm),
            ash::vk::Format::A2R10G10B10_UNORM_PACK32 => Ok(Self::Bgr10a2Unorm),
            ash::vk::Format::R16G16B16A16_SFLOAT => Ok(Self::Rgba16Sfloat),
            ash::vk::Format::R16G16_SFLOAT => Ok(Self::Rg16Sfloat),
            ash::vk::Format::R32_SFLOAT => Ok(Self::R32Sfloat),
            ash::vk::Format::BC1_RGBA_UNORM_BLOCK => Ok(Self::Bc1Unorm),
            ash::vk::Format::BC1_RGBA_SRGB_BLOCK => Ok(Self::Bc1Srgb),
            ash::vk::Format::BC2_UNORM_BLOCK => Ok(Self::Bc2Unorm),
            ash::vk::Format::BC2_SRGB_BLOCK => Ok(Self::Bc2Srgb),
            ash::vk::Format::BC3_UNORM_BLOCK => Ok(Self::Bc3Unorm),
            ash::vk::Format::BC3_SRGB_BLOCK => Ok(Self::Bc3Srgb),
            ash::vk::Format::BC4_UNORM_BLOCK => Ok(Self::Bc4Unorm),
            ash::vk::Format::BC5_UNORM_BLOCK => Ok(Self::Bc5Unorm),
            ash::vk::Format::BC6H_UFLOAT_BLOCK => Ok(Self::Bc6hUfloat),
            ash::vk::Format::BC7_UNORM_BLOCK => Ok(Self::Bc7Unorm),
            ash::vk::Format::BC7_SRGB_BLOCK => Ok(Self::Bc7Srgb),
            ash::vk::Format::D32_SFLOAT => Ok(Self::D32Sfloat),
            _ => Err(Error::InvalidArgument(format!(
                "Vulkan format `{value:?}` has no matching texture format."
            ))),
        }
    }
}

impl From<SamplerFilter> for ash::vk::Filter {
    fn from(value: SamplerFilter) -> Self {
        match value {
            SamplerFilter::Nearest => Self::NEAREST,
            SamplerFilter::Linear => Self::LINEAR,
        }
    }
}

impl From<SamplerFilter> for ash::vk::SamplerMipmapMode {
    fn from(value: SamplerFilter) -> Self {
        match value {
            SamplerFilter::Nearest => Self::NEAREST,
            SamplerFilter::Linear => Self::LINEAR,
        }
    }
}

impl From<SamplerAddressMode> for ash::vk::SamplerAddressMode {
    fn from(value: SamplerAddressMode) -> Self {
        match value {
            SamplerAddressMode::Repeat => Self::REPEAT,
            SamplerAddressMode::MirroredRepeat => Self::MIRRORED_REPEAT,
            SamplerAddressMode::ClampToEdge => Self::CLAMP_TO_EDGE,
        }
    }
}

impl From<CompareOp> for ash::vk::CompareOp {
    fn from(value: CompareOp) -> Self {
        match value {
            CompareOp::Never => Self::NEVER,
            CompareOp::Equal => Self::EQUAL,
            CompareOp::NotEqual => Self::NOT_EQUAL,
            CompareOp::Less => Self::LESS,
            CompareOp::LessOrEqual => Self::LESS_OR_EQUAL,
            CompareOp::Greater => Self::GREATER,
            CompareOp::GreaterOrEqual => Self::GREATER_OR_EQUAL,
            CompareOp::Always => Self::ALWAYS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_binding_type() {
        assert_eq!(
            DescriptorType::from(ShaderBindingType::UniformBuffer),
            DescriptorType::UNIFORM_BUFFER
        );
        assert_eq!(
            DescriptorType::from(ShaderBindingType::StorageBuffer),
            DescriptorType::STORAGE_BUFFER
        );
        assert_eq!(
            DescriptorType::from(ShaderBindingType::Texture),
            DescriptorType::SAMPLED_IMAGE
        );
        assert_eq!(
            DescriptorType::from(ShaderBindingType::Sampler),
            DescriptorType::SAMPLER
        );
    }

    #[test]
    fn test_shader_stage() {
        assert_eq!(
            ash::vk::ShaderStageFlags::from(ShaderStage::Vertex),
            ash::vk::ShaderStageFlags::VERTEX
        );
        assert_eq!(
            ash::vk::ShaderStageFlags::from(ShaderStage::TessellationControl),
            ash::vk::ShaderStageFlags::TESSELLATION_CONTROL
        );
        assert_eq!(
            ash::vk::ShaderStageFlags::from(ShaderStage::TessellationEvaluation),
            ash::vk::ShaderStageFlags::TESSELLATION_EVALUATION
        );
        assert_eq!(
            ash::vk::ShaderStageFlags::from(ShaderStage::Geometry),
            ash::vk::ShaderStageFlags::GEOMETRY
        );
        assert_eq!(
            ash::vk::ShaderStageFlags::from(ShaderStage::Task),
            ash::vk::ShaderStageFlags::TASK_EXT
        );
        assert_eq!(
            ash::vk::ShaderStageFlags::from(ShaderStage::Mesh),
            ash::vk::ShaderStageFlags::MESH_EXT
        );
        assert_eq!(
            ash::vk::ShaderStageFlags::from(ShaderStage::Fragment),
            ash::vk::ShaderStageFlags::FRAGMENT
        );
        assert_eq!(
            ash::vk::ShaderStageFlags::from(ShaderStage::Compute),
            ash::vk::ShaderStageFlags::COMPUTE
        );
    }

    #[test]
    fn test_texture_format_to_vk_format() {
        assert_eq!(
            ash::vk::Format::from(TextureFormat::R8Unorm),
            ash::vk::Format::R8_UNORM
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::R16Unorm),
            ash::vk::Format::R16_UNORM
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Rgba8Unorm),
            ash::vk::Format::R8G8B8A8_UNORM
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Rgba8Srgb),
            ash::vk::Format::R8G8B8A8_SRGB
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Bgra8Srgb),
            ash::vk::Format::B8G8R8A8_SRGB
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Rgb10a2Unorm),
            ash::vk::Format::A2B10G10R10_UNORM_PACK32
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Bgr10a2Unorm),
            ash::vk::Format::A2R10G10B10_UNORM_PACK32
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Rgba16Sfloat),
            ash::vk::Format::R16G16B16A16_SFLOAT
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Rg16Sfloat),
            ash::vk::Format::R16G16_SFLOAT
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::R32Sfloat),
            ash::vk::Format::R32_SFLOAT
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Bc1Unorm),
            ash::vk::Format::BC1_RGBA_UNORM_BLOCK
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Bc1Srgb),
            ash::vk::Format::BC1_RGBA_SRGB_BLOCK
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Bc2Unorm),
            ash::vk::Format::BC2_UNORM_BLOCK
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Bc2Srgb),
            ash::vk::Format::BC2_SRGB_BLOCK
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Bc3Unorm),
            ash::vk::Format::BC3_UNORM_BLOCK
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Bc3Srgb),
            ash::vk::Format::BC3_SRGB_BLOCK
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Bc4Unorm),
            ash::vk::Format::BC4_UNORM_BLOCK
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Bc5Unorm),
            ash::vk::Format::BC5_UNORM_BLOCK
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Bc6hUfloat),
            ash::vk::Format::BC6H_UFLOAT_BLOCK
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Bc7Unorm),
            ash::vk::Format::BC7_UNORM_BLOCK
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::Bc7Srgb),
            ash::vk::Format::BC7_SRGB_BLOCK
        );
        assert_eq!(
            ash::vk::Format::from(TextureFormat::D32Sfloat),
            ash::vk::Format::D32_SFLOAT
        );
    }

    #[test]
    fn test_vk_format_to_texture_format() {
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::R8_UNORM),
            Ok(TextureFormat::R8Unorm)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::R16_UNORM),
            Ok(TextureFormat::R16Unorm)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::R8G8B8A8_UNORM),
            Ok(TextureFormat::Rgba8Unorm)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::R8G8B8A8_SRGB),
            Ok(TextureFormat::Rgba8Srgb)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::B8G8R8A8_SRGB),
            Ok(TextureFormat::Bgra8Srgb)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::A2B10G10R10_UNORM_PACK32),
            Ok(TextureFormat::Rgb10a2Unorm)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::A2R10G10B10_UNORM_PACK32),
            Ok(TextureFormat::Bgr10a2Unorm)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::R16G16B16A16_SFLOAT),
            Ok(TextureFormat::Rgba16Sfloat)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::R16G16_SFLOAT),
            Ok(TextureFormat::Rg16Sfloat)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::R32_SFLOAT),
            Ok(TextureFormat::R32Sfloat)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::BC1_RGBA_UNORM_BLOCK),
            Ok(TextureFormat::Bc1Unorm)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::BC1_RGBA_SRGB_BLOCK),
            Ok(TextureFormat::Bc1Srgb)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::BC2_UNORM_BLOCK),
            Ok(TextureFormat::Bc2Unorm)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::BC2_SRGB_BLOCK),
            Ok(TextureFormat::Bc2Srgb)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::BC3_UNORM_BLOCK),
            Ok(TextureFormat::Bc3Unorm)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::BC3_SRGB_BLOCK),
            Ok(TextureFormat::Bc3Srgb)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::BC4_UNORM_BLOCK),
            Ok(TextureFormat::Bc4Unorm)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::BC5_UNORM_BLOCK),
            Ok(TextureFormat::Bc5Unorm)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::BC6H_UFLOAT_BLOCK),
            Ok(TextureFormat::Bc6hUfloat)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::BC7_UNORM_BLOCK),
            Ok(TextureFormat::Bc7Unorm)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::BC7_SRGB_BLOCK),
            Ok(TextureFormat::Bc7Srgb)
        );
        assert_eq!(
            TextureFormat::try_from(ash::vk::Format::D32_SFLOAT),
            Ok(TextureFormat::D32Sfloat)
        );
    }

    #[test]
    fn test_unsupported_vk_format_to_texture_format() {
        assert!(matches!(
            TextureFormat::try_from(ash::vk::Format::UNDEFINED),
            Err(Error::InvalidArgument(_))
        ));
        assert!(matches!(
            TextureFormat::try_from(ash::vk::Format::A1R5G5B5_UNORM_PACK16),
            Err(Error::InvalidArgument(_))
        ));
    }

    #[test]
    fn test_sampler_filter() {
        assert_eq!(
            ash::vk::Filter::from(SamplerFilter::Nearest),
            ash::vk::Filter::NEAREST
        );
        assert_eq!(
            ash::vk::Filter::from(SamplerFilter::Linear),
            ash::vk::Filter::LINEAR
        );
        assert_eq!(
            ash::vk::SamplerMipmapMode::from(SamplerFilter::Nearest),
            ash::vk::SamplerMipmapMode::NEAREST
        );
        assert_eq!(
            ash::vk::SamplerMipmapMode::from(SamplerFilter::Linear),
            ash::vk::SamplerMipmapMode::LINEAR
        );
    }

    #[test]
    fn test_sampler_address_mode() {
        assert_eq!(
            ash::vk::SamplerAddressMode::from(SamplerAddressMode::Repeat),
            ash::vk::SamplerAddressMode::REPEAT
        );
        assert_eq!(
            ash::vk::SamplerAddressMode::from(SamplerAddressMode::MirroredRepeat),
            ash::vk::SamplerAddressMode::MIRRORED_REPEAT
        );
        assert_eq!(
            ash::vk::SamplerAddressMode::from(SamplerAddressMode::ClampToEdge),
            ash::vk::SamplerAddressMode::CLAMP_TO_EDGE
        );
    }

    #[test]
    fn test_compare_op() {
        assert_eq!(
            ash::vk::CompareOp::from(CompareOp::Never),
            ash::vk::CompareOp::NEVER
        );
        assert_eq!(
            ash::vk::CompareOp::from(CompareOp::Equal),
            ash::vk::CompareOp::EQUAL
        );
        assert_eq!(
            ash::vk::CompareOp::from(CompareOp::NotEqual),
            ash::vk::CompareOp::NOT_EQUAL
        );
        assert_eq!(
            ash::vk::CompareOp::from(CompareOp::Less),
            ash::vk::CompareOp::LESS
        );
        assert_eq!(
            ash::vk::CompareOp::from(CompareOp::LessOrEqual),
            ash::vk::CompareOp::LESS_OR_EQUAL
        );
        assert_eq!(
            ash::vk::CompareOp::from(CompareOp::Greater),
            ash::vk::CompareOp::GREATER
        );
        assert_eq!(
            ash::vk::CompareOp::from(CompareOp::GreaterOrEqual),
            ash::vk::CompareOp::GREATER_OR_EQUAL
        );
        assert_eq!(
            ash::vk::CompareOp::from(CompareOp::Always),
            ash::vk::CompareOp::ALWAYS
        );
    }
}
