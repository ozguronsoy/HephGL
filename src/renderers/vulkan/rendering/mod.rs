mod dynamic_rendering;
mod lifetime_guards;
mod render_pass_rendering;

use ash::vk::CommandBuffer;

use crate::{
    Error, Result,
    renderers::{
        settings::{GraphicsPipelineOptions, Settings},
        vulkan::{
            VulkanRenderer,
            rendering::{
                dynamic_rendering::DynamicRendering, render_pass_rendering::RenderPassRendering,
            },
            resources::VulkanGraphicsPipeline,
            swapchain::SwapchainContext,
        },
    },
    shader::Shader,
};

/// Provides rendering operations.
pub trait VulkanRendering {
    /// Creates the resources required for rendering.
    fn new(
        settings: &Settings,
        device: &ash::Device,
        swapchain_context: &SwapchainContext,
    ) -> Result<Self>
    where
        Self: Sized;
    /// Creates a graphics pipeline using the provided shaders.
    fn create_graphics_pipeline(
        &mut self,
        device: &ash::Device,
        shaders: &[&Shader],
        options: &GraphicsPipelineOptions,
    ) -> Result<VulkanGraphicsPipeline>;
    /// Destroys the graphics pipeline.
    fn destroy_graphics_pipeline(
        &mut self,
        device: &ash::Device,
        pipeline: &VulkanGraphicsPipeline,
    ) -> Result<()>;
    /// Starts rendering the frame.
    fn begin(
        &mut self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        swapchain_context: &SwapchainContext,
    ) -> Result<()>;
    /// Ends rendering the frame.
    fn end(&mut self, device: &ash::Device, command_buffer: CommandBuffer) -> Result<()>;
    /// Frees the resources.
    fn destroy(&mut self, device: &ash::Device) -> Result<()>;
}

impl VulkanRenderer {
    /// Creates rendering resources for the current device.
    pub(super) fn create_rendering(&mut self) -> Result<()> {
        self.destroy_rendering()?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;

        if device_context.supports_dynamic_rendering {
            device_context.rendering = Some(Box::new(DynamicRendering::new(
                &self.settings,
                &device_context.logical_device,
                &device_context.swapchain_context,
            )?));
        } else {
            device_context.rendering = Some(Box::new(RenderPassRendering::new(
                &self.settings,
                &device_context.logical_device,
                &device_context.swapchain_context,
            )?));
        }

        Ok(())
    }

    /// Destroys the rendering resources.
    pub(super) fn destroy_rendering(&mut self) -> Result<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        if let Some(mut rendering) = device_context.rendering.take() {
            rendering.destroy(&device_context.logical_device)?;
        }
        Ok(())
    }
}
