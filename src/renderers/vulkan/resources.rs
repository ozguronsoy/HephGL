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

#[derive(Clone, Copy)]
pub(super) enum VulkanResourceHandle {
    Buffer(Buffer),
    Texture(ash::vk::Image),
    Sampler(ash::vk::Sampler),
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
}

/// Represents a Vulkan graphics pipeline.
#[derive(Clone)]
pub struct VulkanGraphicsPipeline {
    pub(super) pipeline: Pipeline,
    pub(super) layout: PipelineLayout,
    pub(super) descriptor_layouts: Vec<DescriptorSetLayout>,
    pub(super) descriptor_binding_counts: Vec<usize>,
    pub(super) descriptor_set_cache_per_frame:
        Vec<Vec<(Option<VulkanResourceHandle>, DescriptorSet)>>,
    /// This is used to detect when the pipeline is cloned to a worker thread so we can clean the
    /// `descriptor_sets` vector to avoid using the resources of another thread.
    pub(super) thread_id: std::thread::ThreadId,
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

impl VulkanRenderer {
    pub(super) fn create_compute_resource_sets(
        &mut self,
        pipeline: &VulkanComputePipeline,
        binding_sets: &[&[ResourceBinding<'_>]],
    ) -> Result<Vec<DescriptorSet>> {
        if binding_sets.is_empty() {
            return Ok(Vec::new());
        }

        // TODO: Verify group count and bindings.
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
        let descriptor_sets = compute_queue_context.frames[device_context.current_frame_index]
            .thread_contexts[thread_context_index]
            .descriptor_pool_manager
            .allocate(&device_context.logical_device, &pipeline.descriptor_layouts)?;

        for (i, &binding_set) in binding_sets.iter().enumerate() {
            let mut buffer_infos = Vec::new();
            let mut image_infos = Vec::new();
            for binding in binding_set {
                match &binding.resource {
                    ResourceBindingType::Buffer {
                        handle,
                        usage,
                        offset,
                        size,
                    } => {
                        let handle = resource_ref::<VulkanBuffer>(*handle)?;
                        if offset + size > handle.size {
                            return Err(Error::invalid_argument(
                                "Buffer overflow when binding resources.",
                            ));
                        }
                        match usage {
                            BufferUsage::Storage | BufferUsage::Uniform => {}
                            _ => {
                                return Err(Error::invalid_argument(
                                    "Invalid buffer usage for descriptor resource binding.",
                                ));
                            }
                        }
                        buffer_infos.push(
                            ash::vk::DescriptorBufferInfo::default()
                                .buffer(handle.buffer)
                                .offset(*offset as u64)
                                .range(*size as u64),
                        );
                    }
                    ResourceBindingType::Texture { handle } => {
                        let handle = resource_ref::<VulkanTexture>(*handle)?;
                        image_infos.push(
                            ash::vk::DescriptorImageInfo::default()
                                .image_view(handle.image_view)
                                .image_layout(ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
                        );
                    }
                    ResourceBindingType::Sampler { handle } => {
                        let handle = resource_ref::<VulkanSampler>(*handle)?;
                        image_infos
                            .push(ash::vk::DescriptorImageInfo::default().sampler(handle.sampler));
                    }
                }
            }

            let mut buffer_index = 0;
            let mut image_index = 0;
            let mut writes = Vec::with_capacity(binding_set.len());
            for binding in binding_set {
                let write = match binding.resource {
                    ResourceBindingType::Buffer { usage, .. } => {
                        let descriptor_type = match usage {
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
                device_context
                    .logical_device
                    .update_descriptor_sets(&writes, &[]);
            }
        }

        Ok(descriptor_sets)
    }

    pub(super) fn create_graphics_resource_sets(
        &mut self,
        pipeline: &mut VulkanGraphicsPipeline,
        binding_sets: &[&[ResourceBinding<'_>]],
    ) -> Result<Vec<DescriptorSet>> {
        // TODO: Take the vertex and index buffers as separate args rather than in the binding sets
        // since they are not part of the descriptor sets.

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;

        if pipeline.descriptor_layouts.is_empty()
            || !pipeline
                .descriptor_binding_counts
                .iter()
                .any(|&count| count > 0)
        {
            return Ok(Vec::new());
        }
        if self.settings.frames_in_flight != pipeline.descriptor_set_cache_per_frame.len() {
            // TODO: This is a temporary fix for when the frames in flight setting is changed at
            // runtime. We need to clear the descriptor sets whenever they are invalidated. We can
            // use a timestamp in the `DescriptorPoolManager` to determine validity.
            pipeline.descriptor_set_cache_per_frame.clear();
            pipeline
                .descriptor_set_cache_per_frame
                .resize(self.settings.frames_in_flight, Vec::new());
        }
        if !pipeline.descriptor_set_cache_per_frame[device_context.current_frame_index]
            .len()
            .is_multiple_of(pipeline.descriptor_layouts.len())
        {
            // The descriptor sets pipeline is corrupted somehow.
            pipeline.descriptor_set_cache_per_frame[device_context.current_frame_index].clear();
        }
        if pipeline.thread_id != std::thread::current().id() {
            // The pipeline is cloned to a worker thread, we need to clear the descriptor sets
            // since they are not valid for this thread.
            pipeline.descriptor_set_cache_per_frame[device_context.current_frame_index].clear();
            pipeline.thread_id = std::thread::current().id();
        }

        Self::verify_binding_sets(
            &pipeline.descriptor_layouts,
            &pipeline.descriptor_binding_counts,
            binding_sets,
        )?;

        let mut descriptor_sets = Self::get_cached_descriptor_sets(
            pipeline.descriptor_layouts.len(),
            pipeline
                .descriptor_binding_counts
                .iter()
                .position(|&s| s > 0)
                .ok_or(Error::fail(""))?,
            &pipeline.descriptor_set_cache_per_frame[device_context.current_frame_index],
            binding_sets,
        )?;
        if descriptor_sets.is_empty() {
            descriptor_sets = device_context.graphics_queue_context.frames
                [device_context.current_frame_index]
                .thread_contexts[thread_context_index]
                .descriptor_pool_manager
                .allocate(&device_context.logical_device, &pipeline.descriptor_layouts)?;
            Self::extend_cached_descriptor_sets(
                pipeline.descriptor_layouts.len(),
                &mut pipeline.descriptor_set_cache_per_frame[device_context.current_frame_index],
                binding_sets,
                &descriptor_sets,
            )?;
        }

        for (i, &binding_set) in binding_sets.iter().enumerate() {
            if i >= descriptor_sets.len() {
                break;
            }

            let descriptor_bindings = binding_set
                .iter()
                .filter(|binding| {
                    matches!(
                        binding.resource,
                        ResourceBindingType::Buffer {
                            usage: BufferUsage::Storage | BufferUsage::Uniform,
                            ..
                        } | ResourceBindingType::Texture { .. }
                            | ResourceBindingType::Sampler { .. }
                    )
                })
                .collect::<Vec<_>>();

            let mut buffer_infos = Vec::new();
            let mut image_infos = Vec::new();

            for binding in &descriptor_bindings {
                match &binding.resource {
                    ResourceBindingType::Buffer {
                        handle,
                        offset,
                        size,
                        ..
                    } => {
                        let handle = resource_ref::<VulkanBuffer>(*handle)?;
                        if offset + size > handle.size {
                            return Err(Error::invalid_argument(
                                "Buffer overflow when binding resources.",
                            ));
                        }
                        buffer_infos.push(
                            ash::vk::DescriptorBufferInfo::default()
                                .buffer(handle.buffer)
                                .offset(*offset as u64)
                                .range(*size as u64),
                        );
                    }
                    ResourceBindingType::Texture { handle } => {
                        let handle = resource_ref::<VulkanTexture>(*handle)?;
                        image_infos.push(
                            ash::vk::DescriptorImageInfo::default()
                                .image_view(handle.image_view)
                                .image_layout(ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
                        );
                    }
                    ResourceBindingType::Sampler { handle } => {
                        let handle = resource_ref::<VulkanSampler>(*handle)?;
                        image_infos
                            .push(ash::vk::DescriptorImageInfo::default().sampler(handle.sampler));
                    }
                }
            }

            let mut buffer_index = 0;
            let mut image_index = 0;
            let mut writes = Vec::with_capacity(descriptor_bindings.len());
            for binding in descriptor_bindings {
                let write = match binding.resource {
                    ResourceBindingType::Buffer { usage, .. } => {
                        let descriptor_type = match usage {
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
                device_context
                    .logical_device
                    .update_descriptor_sets(&writes, &[]);
            }
        }

        Ok(descriptor_sets)
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

    fn verify_binding_sets(
        descriptor_layouts: &[ash::vk::DescriptorSetLayout],
        descriptor_binding_counts: &[usize],
        binding_sets: &[&[ResourceBinding<'_>]],
    ) -> Result<()> {
        if binding_sets.len() != descriptor_layouts.len()
            || binding_sets.iter().zip(descriptor_binding_counts).any(
                |(&binding_set, &binding_count)| {
                    binding_set
                        .iter()
                        .filter(|binding| {
                            // Exclude vertex and index buffers from the count since they are not
                            // part of the descriptor set.
                            !matches!(
                                binding.resource,
                                ResourceBindingType::Buffer {
                                    usage: BufferUsage::Vertex | BufferUsage::Index,
                                    ..
                                }
                            )
                        })
                        .count()
                        != binding_count
                },
            )
        {
            return Err(Error::invalid_argument(
                "Binding sets do not match the pipeline layout.",
            ));
        }
        // TODO: Verify binding types.
        Ok(())
    }

    fn get_cached_descriptor_sets(
        layout_count: usize,
        set_offset: usize,
        cache: &[(Option<VulkanResourceHandle>, ash::vk::DescriptorSet)],
        binding_sets: &[&[ResourceBinding<'_>]],
    ) -> Result<Vec<DescriptorSet>> {
        for i in (0..cache.len()).step_by(layout_count) {
            // Use the first set with a binding to determine a cache hit.
            let Some(handle) = cache[i + set_offset].0 else {
                return Err(Error::fail("Failed to get cached descriptor sets."));
            };
            // The ash resources are opaque handles, so we can determine a cache hit by comparing
            // them.
            match handle {
                VulkanResourceHandle::Buffer(buffer) => {
                    if let ResourceBindingType::Buffer { handle, .. } = binding_sets[0][0].resource
                    {
                        let vulkan_buffer = resource_ref::<VulkanBuffer>(handle)?;
                        if vulkan_buffer.buffer == buffer {
                            return Ok(cache[i..i + layout_count].iter().map(|ds| ds.1).collect());
                        }
                    }
                }
                VulkanResourceHandle::Texture(image) => {
                    if let ResourceBindingType::Texture { handle, .. } = binding_sets[0][0].resource
                    {
                        let vulkan_texture = resource_ref::<VulkanTexture>(handle)?;
                        if vulkan_texture.image == image {
                            return Ok(cache[i..i + layout_count].iter().map(|ds| ds.1).collect());
                        }
                    }
                }
                VulkanResourceHandle::Sampler(sampler) => {
                    if let ResourceBindingType::Sampler { handle, .. } = binding_sets[0][0].resource
                    {
                        let vulkan_sampler = resource_ref::<VulkanSampler>(handle)?;
                        if vulkan_sampler.sampler == sampler {
                            return Ok(cache[i..i + layout_count].iter().map(|ds| ds.1).collect());
                        }
                    }
                }
            }
        }
        Ok(Vec::new())
    }

    fn extend_cached_descriptor_sets(
        layout_count: usize,
        cache: &mut Vec<(Option<VulkanResourceHandle>, ash::vk::DescriptorSet)>,
        binding_sets: &[&[ResourceBinding<'_>]],
        descriptor_sets: &[ash::vk::DescriptorSet],
    ) -> Result<()> {
        cache.reserve(layout_count);
        for (&binding_set, &descriptor_set) in binding_sets.iter().zip(descriptor_sets) {
            if binding_set.is_empty() {
                cache.push((None, descriptor_set));
            } else {
                let handle = match binding_set[0].resource {
                    ResourceBindingType::Buffer { handle, .. } => {
                        let vulkan_buffer = resource_ref::<VulkanBuffer>(handle)?;
                        VulkanResourceHandle::Buffer(vulkan_buffer.buffer)
                    }
                    ResourceBindingType::Texture { handle } => {
                        let vulkan_texture = resource_ref::<VulkanTexture>(handle)?;
                        VulkanResourceHandle::Texture(vulkan_texture.image)
                    }
                    ResourceBindingType::Sampler { handle } => {
                        let vulkan_sampler = resource_ref::<VulkanSampler>(handle)?;
                        VulkanResourceHandle::Sampler(vulkan_sampler.sampler)
                    }
                };
                cache.push((Some(handle), descriptor_set));
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
