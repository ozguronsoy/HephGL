use ash::vk::{Buffer, DescriptorSet, DescriptorType};

use crate::{
    renderers::{
        GpuBuffer, Renderer, RendererResult,
        error::RendererError,
        resources::{BufferUsage, ResourceBinding, ResourceBindingType},
        vulkan::{VulkanRenderer, pipeline::VulkanComputePipeline, queue::QueueType},
    },
    shader::ShaderBindingType,
};

/// An opaque handle to a compute pipeline.
#[derive(Clone, Copy)]
pub struct VulkanComputePipelineHandle {
    /// Pointer to the pipeline instance.
    pub(super) ptr: usize,
    /// Position of the current pipeline within the internal pipeline list. This is used for
    /// validating the handle.
    pub(super) index: usize,
}

/// An opaque handle to a graphics pipeline.
#[derive(Clone, Copy)]
pub struct VulkanGraphicsPipelineHandle {
    // TODO
}

/// Represents a Vulkan buffer.
#[derive(Debug, Copy, Clone)]
pub struct VulkanBuffer {
    /// The Vulkan buffer.
    pub(super) buffer: Buffer,
    /// The memory allocation.
    pub(super) vma_allocation: vk_mem::Allocation,
    /// The size of the buffer in bytes.
    pub(super) size: usize,
}

/// Represents a recorded Vulkan command.
#[derive(Debug, Copy, Clone)]
pub struct VulkanRecordedCommand {
    pub(super) queue_type: QueueType,
    pub(super) frame_index: u32,
    pub(super) thread_context_index: usize,
}

impl GpuBuffer for VulkanBuffer {
    fn size(&self) -> usize {
        self.size
    }
}

impl VulkanRenderer {
    pub(super) fn create_resource_sets(
        &self,
        pipeline: &VulkanComputePipeline,
        binding_sets: &[&[ResourceBinding<<VulkanRenderer as Renderer>::BufferHandle>]],
    ) -> RendererResult<Vec<DescriptorSet>> {
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
            let buffer_infos: Vec<_> = binding_set
                .iter()
                .map(|binding| match &binding.resource {
                    ResourceBindingType::Buffer {
                        handle,
                        offset,
                        size,
                        ..
                    } => ash::vk::DescriptorBufferInfo::default()
                        .buffer(handle.buffer)
                        .offset(*offset as u64)
                        .range(*size as u64),
                })
                .collect();

            let mut writes = Vec::with_capacity(binding_set.len());
            for (binding, info) in binding_set.iter().zip(buffer_infos.iter()) {
                let descriptor_type = match binding.resource {
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
                            BufferUsage::Storage => ash::vk::DescriptorType::STORAGE_BUFFER,
                            BufferUsage::Uniform => ash::vk::DescriptorType::UNIFORM_BUFFER,
                            _ => {
                                return Err(RendererError::invalid_argument(
                                    "Invalid buffer usage for resource binding.",
                                ));
                            }
                        }
                    }
                };
                writes.push(
                    ash::vk::WriteDescriptorSet::default()
                        .dst_set(descriptor_sets[i])
                        .dst_binding(binding.binding)
                        .descriptor_type(descriptor_type)
                        .buffer_info(std::slice::from_ref(info)),
                );
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
        }
    }
}
