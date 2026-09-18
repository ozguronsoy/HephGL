use ash::vk::{DescriptorSetLayout, Pipeline, PipelineLayout};

use crate::renderers::{Renderer, RendererResult, error::RendererError, vulkan::VulkanRenderer};

/// Represents a Vulkan compute pipeline.
pub struct VulkanComputePipeline {
    pub(super) pipeline: Pipeline,
    pub(super) layout: PipelineLayout,
    pub(super) descriptor_layouts: Vec<DescriptorSetLayout>,
}

impl VulkanRenderer {
    /// Gets a pointer to the pipeline object if the handle is valid.
    pub(super) fn compute_pipeline(
        &self,
        pipeline_handle: <VulkanRenderer as Renderer>::ComputePipelineHandle,
    ) -> RendererResult<*const VulkanComputePipeline> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let compute_pipelines = device_context.compute_pipelines.lock()?;
        if compute_pipelines.len() > pipeline_handle.index {
            let p_pipeline =
                compute_pipelines[pipeline_handle.index].as_ref() as *const VulkanComputePipeline;
            if p_pipeline as usize == pipeline_handle.ptr {
                return Ok(p_pipeline);
            }
        }
        Err(RendererError::invalid_argument(
            "Invalid compute pipeline handle.",
        ))
    }
}
