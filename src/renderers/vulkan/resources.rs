use ash::vk::{
    Buffer, DescriptorSet, DescriptorSetLayout, DescriptorType, Pipeline, PipelineLayout,
};

use crate::{
    renderers::{
        GpuBuffer, Renderer, RendererResult,
        error::RendererError,
        resources::{BufferUsage, GpuTexture, ResourceBinding, ResourceBindingType, TextureFormat},
        settings::TextureOptions,
        vulkan::VulkanRenderer,
    },
    shader::ShaderBindingType,
};

/// Represents a Vulkan buffer.
#[derive(Copy, Clone)]
pub struct VulkanBuffer {
    /// The Vulkan buffer.
    pub(super) buffer: Buffer,
    /// The memory allocation.
    pub(super) vma_allocation: vk_mem::Allocation,
    /// The size of the buffer in bytes.
    pub(super) size: usize,
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

/// Represents a Vulkan graphics pipeline.
#[derive(Clone)]
pub struct VulkanGraphicsPipeline {
    pub(super) pipeline: ash::vk::Pipeline,
    pub(super) layout: ash::vk::PipelineLayout,
    pub(super) descriptor_layouts: Vec<ash::vk::DescriptorSetLayout>,
}

/// Represents a Vulkan compute pipeline.
#[derive(Clone)]
pub struct VulkanComputePipeline {
    pub(super) pipeline: Pipeline,
    pub(super) layout: PipelineLayout,
    pub(super) descriptor_layouts: Vec<DescriptorSetLayout>,
}

impl GpuTexture for VulkanTexture {
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

impl GpuBuffer for VulkanBuffer {
    fn size(&self) -> usize {
        self.size
    }
}

impl VulkanRenderer {
    pub(super) fn create_compute_resource_sets(
        &self,
        pipeline: &<VulkanRenderer as Renderer>::ComputePipeline,
        binding_sets: &[&[ResourceBinding<VulkanBuffer, VulkanTexture, VulkanSampler>]],
    ) -> RendererResult<Vec<DescriptorSet>> {
        if binding_sets.is_empty() {
            return Ok(Vec::new());
        }

        // TODO: Verify group count and bindings.
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let compute_queue_context = device_context.compute_queue_context.as_ref().ok_or(
            RendererError::invalid_operation(
                "Device is not initialized with `ComputeShaders` feature.",
            ),
        )?;
        let thread_context_index = Self::thread_context_index()?;
        let current_frame = &compute_queue_context.frames[self.current_frame_index as usize]
            .thread_contexts[thread_context_index];
        let descriptor_pool = current_frame.descriptor_pool;

        let alloc_info = ash::vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&pipeline.descriptor_layouts);
        let descriptor_sets = unsafe {
            device_context
                .logical_device
                .allocate_descriptor_sets(&alloc_info)?
        };

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
                        if offset + size > handle.size {
                            return Err(RendererError::invalid_argument(
                                "Buffer overflow when binding resources.",
                            ));
                        }
                        match usage {
                            BufferUsage::Storage | BufferUsage::Uniform => {}
                            _ => {
                                return Err(RendererError::invalid_argument(
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
                        image_infos.push(
                            ash::vk::DescriptorImageInfo::default()
                                .image_view(handle.image_view)
                                .image_layout(ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
                        );
                    }
                    ResourceBindingType::Sampler { handle } => {
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
        &self,
        pipeline: &<VulkanRenderer as Renderer>::GraphicsPipeline,
        binding_sets: &[&[ResourceBinding<VulkanBuffer, VulkanTexture, VulkanSampler>]],
    ) -> RendererResult<Vec<DescriptorSet>> {
        if pipeline.descriptor_layouts.is_empty() {
            return Ok(Vec::new());
        }

        // TODO: Verify group count and bindings.
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let graphics_queue_context = &device_context.graphics_queue_context;
        let thread_context_index = Self::thread_context_index()?;
        let current_frame = &graphics_queue_context.frames[self.current_frame_index as usize]
            .thread_contexts[thread_context_index];
        let descriptor_pool = current_frame.descriptor_pool;

        let alloc_info = ash::vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&pipeline.descriptor_layouts);
        let descriptor_sets = unsafe {
            device_context
                .logical_device
                .allocate_descriptor_sets(&alloc_info)?
        };

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
                        if offset + size > handle.size {
                            return Err(RendererError::invalid_argument(
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
                        image_infos.push(
                            ash::vk::DescriptorImageInfo::default()
                                .image_view(handle.image_view)
                                .image_layout(ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
                        );
                    }
                    ResourceBindingType::Sampler { handle } => {
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
    ) -> RendererResult<ash::vk::ShaderStageFlags> {
        match stage {
            naga::ShaderStage::Compute => Ok(ash::vk::ShaderStageFlags::COMPUTE),
            naga::ShaderStage::Vertex => Ok(ash::vk::ShaderStageFlags::VERTEX),
            naga::ShaderStage::Fragment => Ok(ash::vk::ShaderStageFlags::FRAGMENT),
            _ => Err(RendererError::invalid_argument("Invalid shader stage.")),
        }
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
