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
    Buffer(Buffer, BufferUsage, usize, usize),
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
    /// Each frame has it's own descriptor pools/sets.
    pub(super) descriptor_set_cache_per_frame: Vec<Vec<DescriptorSet>>,
    pub(super) resource_handles_per_frame: Vec<Vec<VulkanResourceHandle>>,
    /// This is used to detect when the pipeline is cloned to a worker thread so we can clean the
    /// `descriptor_sets` vector to avoid using the resources of another thread.
    pub(super) thread_id: std::thread::ThreadId,
    pub(super) time_stamps: Vec<std::time::Instant>,
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
        let time_stamp = device_context.graphics_queue_context.frames
            [device_context.current_frame_index]
            .thread_contexts[thread_context_index]
            .descriptor_pool_manager
            .time_stamp();

        let binding_count = pipeline.descriptor_binding_counts.iter().sum();

        if pipeline.descriptor_layouts.is_empty() || binding_count == 0 {
            return Ok(Vec::new());
        }
        if pipeline.descriptor_set_cache_per_frame.len() != self.settings.frames_in_flight {
            pipeline.descriptor_set_cache_per_frame.clear();
            pipeline.resource_handles_per_frame.clear();
            pipeline.time_stamps.clear();
            pipeline
                .descriptor_set_cache_per_frame
                .resize(self.settings.frames_in_flight, Vec::new());
            pipeline
                .resource_handles_per_frame
                .resize(self.settings.frames_in_flight, Vec::new());
            pipeline
                .time_stamps
                .resize(self.settings.frames_in_flight, std::time::Instant::now());
        }
        if let Some(time_stamp) = time_stamp
            && pipeline.time_stamps[device_context.current_frame_index] != time_stamp
        {
            pipeline.descriptor_set_cache_per_frame[device_context.current_frame_index].clear();
            pipeline.resource_handles_per_frame[device_context.current_frame_index].clear();
            pipeline.time_stamps[device_context.current_frame_index] = time_stamp;
        }
        if !pipeline.descriptor_set_cache_per_frame[device_context.current_frame_index]
            .len()
            .is_multiple_of(pipeline.descriptor_layouts.len())
            || !pipeline.resource_handles_per_frame[device_context.current_frame_index]
                .len()
                .is_multiple_of(binding_count)
        {
            // The descriptor sets pipeline is corrupted somehow.
            pipeline.descriptor_set_cache_per_frame[device_context.current_frame_index].clear();
            pipeline.resource_handles_per_frame[device_context.current_frame_index].clear();
        }
        if pipeline.thread_id != std::thread::current().id() {
            // The pipeline is cloned to a worker thread, we need to clear the descriptor sets
            // since they are not valid for this thread.
            pipeline.descriptor_set_cache_per_frame[device_context.current_frame_index].clear();
            pipeline.resource_handles_per_frame[device_context.current_frame_index].clear();
            pipeline.thread_id = std::thread::current().id();
            pipeline.time_stamps[device_context.current_frame_index] =
                time_stamp.unwrap_or(std::time::Instant::now());
        }

        Self::verify_binding_sets(
            &pipeline.descriptor_layouts,
            &pipeline.descriptor_binding_counts,
            binding_sets,
        )?;

        let (mut descriptor_sets, update_sets) = Self::get_cached_descriptor_sets(
            pipeline.descriptor_layouts.len(),
            binding_count,
            &pipeline.descriptor_set_cache_per_frame[device_context.current_frame_index],
            &pipeline.resource_handles_per_frame[device_context.current_frame_index],
            binding_sets,
        );

        if descriptor_sets.is_empty() {
            descriptor_sets = device_context.graphics_queue_context.frames
                [device_context.current_frame_index]
                .thread_contexts[thread_context_index]
                .descriptor_pool_manager
                .allocate(&device_context.logical_device, &pipeline.descriptor_layouts)?;
            Self::extend_cached_descriptor_sets(
                pipeline.descriptor_layouts.len(),
                binding_count,
                &mut pipeline.descriptor_set_cache_per_frame[device_context.current_frame_index],
                &mut pipeline.resource_handles_per_frame[device_context.current_frame_index],
                binding_sets,
                &descriptor_sets,
            )?;
            if time_stamp.is_none() {
                // This is the first pool/set allocation.
                pipeline.time_stamps[device_context.current_frame_index] = device_context
                    .graphics_queue_context
                    .frames[device_context.current_frame_index]
                    .thread_contexts[thread_context_index]
                    .descriptor_pool_manager
                    .time_stamp()
                    .unwrap();
            }
        } else if !update_sets {
            return Ok(descriptor_sets);
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

    /// Number of binding sets and the number of bindings in each set are constant. Thus we can
    /// split the cached descriptor sets and resource handles to chunks. If we find a chunk of
    /// resource handles that match the `binding_sets` input, that's a cache-hit. We use that
    /// chunk's position to get the cached descriptor sets since they are guaranteed to be in the
    /// same chunk position.
    ///
    /// TODO: Return descriptor sets as a slice to reduce allocations.
    fn get_cached_descriptor_sets(
        layout_count: usize,
        binding_count: usize,
        cache: &[DescriptorSet],
        handles: &[VulkanResourceHandle],
        binding_sets: &[&[ResourceBinding<'_>]],
    ) -> (Vec<DescriptorSet>, bool) {
        let mut update = false;
        let cached_sets = handles
            .chunks_exact(binding_count)
            .position(|chunk| {
                for (&handle, &binding) in chunk.iter().zip(binding_sets.iter().flat_map(|&s| {
                    s.iter().filter(|binding| {
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
                })) {
                    match handle {
                        VulkanResourceHandle::Buffer(buffer, r_usage, r_offset, r_size) => {
                            let ResourceBindingType::Buffer {
                                handle,
                                usage,
                                offset,
                                size,
                            } = binding.resource
                            else {
                                return false;
                            };
                            let Ok(vulkan_buffer) = resource_ref::<VulkanBuffer>(handle) else {
                                return false;
                            };
                            if vulkan_buffer.buffer != buffer {
                                return false;
                            }
                            update |= r_usage != usage || r_offset != offset || r_size != size;
                        }
                        VulkanResourceHandle::Texture(image) => {
                            let ResourceBindingType::Texture { handle, .. } = binding.resource
                            else {
                                return false;
                            };
                            let Ok(vulkan_texture) = resource_ref::<VulkanTexture>(handle) else {
                                return false;
                            };
                            if vulkan_texture.image != image {
                                return false;
                            }
                            // TODO: Make sampler updatable?
                        }
                        VulkanResourceHandle::Sampler(sampler) => {
                            let ResourceBindingType::Sampler { handle, .. } = binding.resource
                            else {
                                return false;
                            };
                            let Ok(vulkan_sampler) = resource_ref::<VulkanSampler>(handle) else {
                                return false;
                            };
                            if vulkan_sampler.sampler != sampler {
                                return false;
                            }
                        }
                    }
                }
                true
            })
            .and_then(|p| cache.chunks_exact(layout_count).nth(p))
            .unwrap_or_default()
            .to_vec();
        update |= cached_sets.is_empty();
        (cached_sets, update)
    }

    fn extend_cached_descriptor_sets(
        layout_count: usize,
        binding_count: usize,
        cache: &mut Vec<ash::vk::DescriptorSet>,
        handles: &mut Vec<VulkanResourceHandle>,
        binding_sets: &[&[ResourceBinding<'_>]],
        descriptor_sets: &[ash::vk::DescriptorSet],
    ) -> Result<()> {
        cache.reserve(layout_count);
        handles.reserve(binding_count);
        for (&binding_set, &descriptor_set) in binding_sets.iter().zip(descriptor_sets) {
            cache.push(descriptor_set);
            for binding in binding_set {
                handles.push(match binding.resource {
                    ResourceBindingType::Buffer {
                        handle,
                        usage,
                        offset,
                        size,
                    } => {
                        let vulkan_buffer = resource_ref::<VulkanBuffer>(handle)?;
                        if vulkan_buffer.usage == BufferUsage::Vertex
                            || vulkan_buffer.usage == BufferUsage::Index
                        {
                            continue;
                        }
                        VulkanResourceHandle::Buffer(vulkan_buffer.buffer, usage, offset, size)
                    }
                    ResourceBindingType::Texture { handle } => {
                        let vulkan_texture = resource_ref::<VulkanTexture>(handle)?;
                        VulkanResourceHandle::Texture(vulkan_texture.image)
                    }
                    ResourceBindingType::Sampler { handle } => {
                        let vulkan_sampler = resource_ref::<VulkanSampler>(handle)?;
                        VulkanResourceHandle::Sampler(vulkan_sampler.sampler)
                    }
                });
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
