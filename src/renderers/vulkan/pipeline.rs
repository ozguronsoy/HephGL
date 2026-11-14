use ash::vk::{DescriptorSetLayout, Handle, Pipeline, PipelineLayout};

use crate::renderers::{
    RendererComputePipeline, RendererGraphicsPipeline, RendererPipeline,
    vulkan::resources::DescriptorSetCache,
};

/// Represents a Vulkan compute pipeline.
#[derive(Clone)]
pub(super) struct VulkanComputePipeline {
    pub(super) pipeline: Pipeline,
    pub(super) layout: PipelineLayout,
    pub(super) descriptor_layouts: Vec<DescriptorSetLayout>,
    pub(super) descriptor_binding_counts: Vec<usize>,
    pub(super) descriptor_set_cache: DescriptorSetCache,
}

/// Represents a Vulkan graphics pipeline.
#[derive(Clone)]
pub(super) struct VulkanGraphicsPipeline {
    pub(super) pipeline: Pipeline,
    pub(super) layout: PipelineLayout,
    pub(super) descriptor_layouts: Vec<DescriptorSetLayout>,
    pub(super) descriptor_binding_counts: Vec<usize>,
    pub(super) descriptor_set_cache: DescriptorSetCache,
    pub(super) depth_only: bool,
}

impl RendererPipeline for VulkanGraphicsPipeline {
    fn is_allocated(&self) -> bool {
        !(self.pipeline.is_null() || self.layout.is_null())
    }
}
impl RendererGraphicsPipeline for VulkanGraphicsPipeline {}

impl RendererPipeline for VulkanComputePipeline {
    fn is_allocated(&self) -> bool {
        !(self.pipeline.is_null() || self.layout.is_null())
    }
}
impl RendererComputePipeline for VulkanComputePipeline {}
