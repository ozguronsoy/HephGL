mod dynamic_rendering;
mod render_pass_rendering;

use ash::vk::CommandBuffer;

use crate::{
    renderers::{
        Renderer, RendererResult,
        error::RendererError,
        settings::Settings,
        vulkan::{
            VulkanRenderer,
            rendering::{
                dynamic_rendering::DynamicRendering, render_pass_rendering::RenderPassRendering,
            },
            swapchain::SwapchainContext,
        },
    },
    shader::Shader,
};

/// Provides rendering operations.
pub trait VulkanRendering {
    /// Creates the resources required for rendering.
    fn new(device: &ash::Device, swapchain_context: &SwapchainContext) -> RendererResult<Self>
    where
        Self: Sized;
    /// Creates a graphics pipeline using the provided shaders.
    fn create_graphics_pipeline(
        &mut self,
        device: &ash::Device,
        shaders: &[&Shader],
    ) -> RendererResult<<VulkanRenderer as Renderer>::GraphicsPipelineHandle>;
    /// Destroys the graphics pipeline.
    fn destroy_graphics_pipeline(
        &mut self,
        device: &ash::Device,
        pipeline: &<VulkanRenderer as Renderer>::GraphicsPipelineHandle,
    ) -> RendererResult<()>;
    /// Starts rendering the frame.
    fn begin(
        &mut self,
        settings: &Settings,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        swapchain_context: &SwapchainContext,
    ) -> RendererResult<()>;
    /// Ends rendering the frame.
    fn end(&mut self, device: &ash::Device, command_buffer: CommandBuffer) -> RendererResult<()>;
    /// Frees the resources.
    fn destroy(&mut self, device: &ash::Device) -> RendererResult<()>;
}

impl VulkanRenderer {
    /// Creates rendering resources for the current device.
    pub(super) fn create_rendering(&mut self) -> RendererResult<()> {
        self.destroy_rendering()?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        if device_context.supports_dynamic_rendering {
            device_context.rendering = Some(Box::new(DynamicRendering::new(
                &device_context.logical_device,
                &device_context.swapchain_context,
            )?));
        } else {
            device_context.rendering = Some(Box::new(RenderPassRendering::new(
                &device_context.logical_device,
                &device_context.swapchain_context,
            )?));
        }

        Ok(())
    }

    /// Destroys the rendering resources.
    pub(super) fn destroy_rendering(&mut self) -> RendererResult<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        if let Some(mut rendering) = device_context.rendering.take() {
            rendering.destroy(&device_context.logical_device)?;
        }
        Ok(())
    }
}
