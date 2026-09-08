use ash::vk::{Buffer, DescriptorSet, DescriptorSetLayout, Pipeline, PipelineLayout, ShaderModule};

use crate::renderers::{GpuBuffer, vulkan::queue::QueueType};

/// Represents a Vulkan shader.
#[derive(Debug, Copy, Clone)]
pub struct VulkanShader {
    pub(super) module: ShaderModule,
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

/// Represents a Vulkan graphics pipeline.
#[derive(Debug, Copy, Clone)]
pub struct VulkanGraphicsPipeline {
    // TODO
}

/// Represents a Vulkan compute pipeline.
#[derive(Debug, Copy, Clone)]
pub struct VulkanComputePipeline {
    pub(super) pipeline: Pipeline,
    pub(super) layout: PipelineLayout,
    pub(super) descriptor_layout: DescriptorSetLayout,
}

/// Represents a resource set compatible to a specific shader.
#[derive(Debug, Copy, Clone)]
pub struct VulkanResourceSet {
    /// The Vulkan descriptor set.
    pub(super) descriptor_set: DescriptorSet,
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
