use ash::vk::CommandBuffer;

use crate::{
    renderers::{
        Renderer, RendererResult,
        settings::Settings,
        vulkan::{VulkanRenderer, rendering::VulkanRendering, swapchain::SwapchainContext},
    },
    shader::Shader,
};

/// Implements rendering via render pass.
pub struct RenderPassRendering;

impl VulkanRendering for RenderPassRendering {
    fn new(_: &ash::Device, _: &SwapchainContext) -> RendererResult<Self>
    where
        Self: Sized,
    {
        todo!();
    }
    fn create_graphics_pipeline(
        &mut self,
        _: &Settings,
        _: &ash::Device,
        _: &[&Shader],
    ) -> RendererResult<<VulkanRenderer as Renderer>::GraphicsPipelineHandle> {
        todo!();
    }
    fn destroy_graphics_pipeline(
        &mut self,
        _: &ash::Device,
        _: &<VulkanRenderer as Renderer>::GraphicsPipelineHandle,
    ) -> RendererResult<()> {
        todo!();
    }
    fn begin(
        &mut self,
        _: &Settings,
        _: &ash::Device,
        _: CommandBuffer,
        _: &SwapchainContext,
    ) -> RendererResult<()> {
        todo!();
    }
    fn end(&mut self, _: &ash::Device, _: CommandBuffer) -> RendererResult<()> {
        todo!();
    }
    fn destroy(&mut self, _: &ash::Device) -> RendererResult<()> {
        todo!();
    }
}
