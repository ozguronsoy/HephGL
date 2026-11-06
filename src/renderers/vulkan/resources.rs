use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    ops::Range,
};

use ash::vk::{
    Buffer, DescriptorSet, DescriptorSetLayout, DescriptorType, Handle, Pipeline, PipelineLayout,
};

use crate::{
    Error, Result,
    renderers::{
        resource_ref,
        resources::{
            BufferUsage, RendererBuffer, RendererComputePipeline, RendererGraphicsPipeline,
            RendererResource, RendererSampler, RendererTexture, ResourceBinding,
            ResourceBindingType, TextureFormat,
        },
        settings::TextureOptions,
        vulkan::VulkanRenderer,
    },
    shader::ShaderBindingType,
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
pub struct VulkanBuffer {
    pub(super) buffer: Buffer,
    pub(super) vma_allocation: vk_mem::Allocation,
    pub(super) size: usize,
    pub(super) usage: BufferUsage,
}

/// Represents a Vulkan texture.
#[derive(Copy, Clone)]
pub struct VulkanTexture {
    pub(super) image: ash::vk::Image,
    pub(super) image_view: ash::vk::ImageView,
    pub(super) vma_allocation: vk_mem::Allocation,
    pub(super) texture_options: TextureOptions,
}

/// Represents a Vulkan texture sampler.
#[derive(Copy, Clone)]
pub struct VulkanSampler {
    pub(super) sampler: ash::vk::Sampler,
}

/// Represents a Vulkan compute pipeline.
#[derive(Clone)]
pub struct VulkanComputePipeline {
    pub(super) pipeline: Pipeline,
    pub(super) layout: PipelineLayout,
    pub(super) descriptor_layouts: Vec<DescriptorSetLayout>,
    pub(super) descriptor_binding_counts: Vec<usize>,
    pub(super) descriptor_set_cache: DescriptorSetCache,
}

/// Represents a Vulkan graphics pipeline.
#[derive(Clone)]
pub struct VulkanGraphicsPipeline {
    pub(super) pipeline: Pipeline,
    pub(super) layout: PipelineLayout,
    pub(super) descriptor_layouts: Vec<DescriptorSetLayout>,
    pub(super) descriptor_binding_counts: Vec<usize>,
    pub(super) descriptor_set_cache: DescriptorSetCache,
}

impl RendererResource for VulkanGraphicsPipeline {
    fn is_allocated(&self) -> bool {
        !(self.pipeline.is_null() || self.layout.is_null())
    }
}
impl RendererGraphicsPipeline for VulkanGraphicsPipeline {}

impl RendererResource for VulkanComputePipeline {
    fn is_allocated(&self) -> bool {
        !(self.pipeline.is_null() || self.layout.is_null())
    }
}
impl RendererComputePipeline for VulkanComputePipeline {}

impl RendererResource for VulkanSampler {
    fn is_allocated(&self) -> bool {
        !self.sampler.is_null()
    }
}
impl RendererSampler for VulkanSampler {}

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
        &mut self,
        pipeline: &mut VulkanComputePipeline,
        binding_sets: &[&[ResourceBinding<'_>]],
    ) -> Result<Range<usize>> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let compute_queue_context =
            device_context
                .compute_queue_context
                .as_mut()
                .ok_or(Error::invalid_operation(
                    "Device is not initialized with `ComputeShaders` feature.",
                ))?;
        let thread_context_index = Self::thread_context_index()?;
        let pool_generation = compute_queue_context.frames[0].thread_contexts[thread_context_index]
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

        let descriptor_sets = compute_queue_context.frames[device_context.current_frame_index]
            .thread_contexts[thread_context_index]
            .descriptor_pool_manager
            .allocate(&device_context.logical_device, &pipeline.descriptor_layouts)?;
        let range = Self::extend_cached_descriptor_sets(
            pipeline.descriptor_layouts.len(),
            &mut pipeline.descriptor_set_cache,
            hash,
            &descriptor_sets,
        )?;
        Self::write_descriptor_sets(
            &device_context.logical_device,
            &descriptor_sets,
            binding_sets,
        )?;

        Ok(range)
    }

    pub(super) fn create_graphics_resource_sets(
        &mut self,
        pipeline: &mut VulkanGraphicsPipeline,
        binding_sets: &[&[ResourceBinding<'_>]],
    ) -> Result<Range<usize>> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;
        let pool_generation = device_context.graphics_queue_context.frames[0].thread_contexts
            [thread_context_index]
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

        let descriptor_sets = device_context.graphics_queue_context.frames
            [device_context.current_frame_index]
            .thread_contexts[thread_context_index]
            .descriptor_pool_manager
            .allocate(&device_context.logical_device, &pipeline.descriptor_layouts)?;
        let range = Self::extend_cached_descriptor_sets(
            pipeline.descriptor_layouts.len(),
            &mut pipeline.descriptor_set_cache,
            hash,
            &descriptor_sets,
        )?;
        Self::write_descriptor_sets(
            &device_context.logical_device,
            &descriptor_sets,
            binding_sets,
        )?;

        Ok(range)
    }

    pub(super) fn convert_shader_stage(
        stage: naga::ShaderStage,
    ) -> Result<ash::vk::ShaderStageFlags> {
        match stage {
            naga::ShaderStage::Compute => Ok(ash::vk::ShaderStageFlags::COMPUTE),
            naga::ShaderStage::Vertex => Ok(ash::vk::ShaderStageFlags::VERTEX),
            naga::ShaderStage::Fragment => Ok(ash::vk::ShaderStageFlags::FRAGMENT),
            _ => Err(Error::invalid_argument("Invalid shader stage.")),
        }
    }

    fn validate_cache_consistency(layout_count: usize, cache: &DescriptorSetCache) -> bool {
        let descriptor_sets = cache.descriptor_sets.chunks_exact(layout_count);
        descriptor_sets.remainder().is_empty() && descriptor_sets.len() == cache.chunk_indices.len()
    }

    fn validate_binding_sets(
        descriptor_layouts: &[ash::vk::DescriptorSetLayout],
        descriptor_binding_counts: &[usize],
        binding_sets: &[&[ResourceBinding<'_>]],
    ) -> Result<()> {
        for binding in binding_sets.iter().flat_map(|&s| s.iter()) {
            if let ResourceBindingType::Buffer {
                buffer,
                offset,
                size,
            } = binding.resource
            {
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
                    let Ok(buffer) = resource_ref::<VulkanBuffer>(buffer) else {
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
                    let Ok(texture) = resource_ref::<VulkanTexture>(texture) else {
                        return Err(Error::invalid_argument(
                            "Invalid buffer type, expected a VulkanTexture.",
                        ));
                    };
                    texture.image.hash(&mut hasher);
                    texture.image_view.hash(&mut hasher);
                }
                ResourceBindingType::Sampler { sampler } => {
                    let Ok(sampler) = resource_ref::<VulkanSampler>(sampler) else {
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
        for (i, &binding_set) in binding_sets.iter().enumerate() {
            let mut buffer_infos = Vec::new();
            let mut image_infos = Vec::new();
            for binding in binding_set {
                match binding.resource {
                    ResourceBindingType::Buffer {
                        buffer,
                        offset,
                        size,
                    } => {
                        let buffer = resource_ref::<VulkanBuffer>(buffer)?;
                        buffer_infos.push(
                            ash::vk::DescriptorBufferInfo::default()
                                .buffer(buffer.buffer)
                                .offset(offset as u64)
                                .range(size as u64),
                        );
                    }
                    ResourceBindingType::Texture { texture } => {
                        let texture = resource_ref::<VulkanTexture>(texture)?;
                        image_infos.push(
                            ash::vk::DescriptorImageInfo::default()
                                .image_view(texture.image_view)
                                .image_layout(ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
                        );
                    }
                    ResourceBindingType::Sampler { sampler } => {
                        let sampler = resource_ref::<VulkanSampler>(sampler)?;
                        image_infos
                            .push(ash::vk::DescriptorImageInfo::default().sampler(sampler.sampler));
                    }
                }
            }

            let mut buffer_index = 0;
            let mut image_index = 0;
            let mut writes = Vec::with_capacity(binding_set.len());
            for binding in binding_set {
                let write = match binding.resource {
                    ResourceBindingType::Buffer { buffer, .. } => {
                        let descriptor_type = match buffer.usage() {
                            BufferUsage::Storage => ash::vk::DescriptorType::STORAGE_BUFFER,
                            BufferUsage::Uniform => ash::vk::DescriptorType::UNIFORM_BUFFER,
                            _ => unreachable!(),
                        };

                        let info = &buffer_infos[buffer_index];
                        buffer_index += 1;

                        ash::vk::WriteDescriptorSet::default()
                            .dst_set(descriptor_sets[i])
                            .dst_binding(binding.binding)
                            .descriptor_type(descriptor_type)
                            .buffer_info(std::slice::from_ref(info))
                    }
                    ResourceBindingType::Texture { .. } => {
                        let info = &image_infos[image_index];
                        image_index += 1;

                        ash::vk::WriteDescriptorSet::default()
                            .dst_set(descriptor_sets[i])
                            .dst_binding(binding.binding)
                            .descriptor_type(ash::vk::DescriptorType::SAMPLED_IMAGE)
                            .image_info(std::slice::from_ref(info))
                    }
                    ResourceBindingType::Sampler { .. } => {
                        let info = &image_infos[image_index];
                        image_index += 1;

                        ash::vk::WriteDescriptorSet::default()
                            .dst_set(descriptor_sets[i])
                            .dst_binding(binding.binding)
                            .descriptor_type(ash::vk::DescriptorType::SAMPLER)
                            .image_info(std::slice::from_ref(info))
                    }
                };
                writes.push(write);
            }

            unsafe {
                logical_device.update_descriptor_sets(&writes, &[]);
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

impl From<TextureFormat> for ash::vk::Format {
    fn from(value: TextureFormat) -> Self {
        match value {
            TextureFormat::R8Unorm => Self::R8_UNORM,
            TextureFormat::Rgba8Unorm => Self::R8G8B8A8_UNORM,
            TextureFormat::Rgba8Srgb => Self::R8G8B8A8_SRGB,
        }
    }
}
