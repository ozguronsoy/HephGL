mod dynamic_rendering;
mod render_pass_rendering;

use std::collections::HashSet;

use ash::vk::CommandBuffer;

use crate::{
    Error, Result,
    graphics_device::GraphicsDeviceFeature,
    renderers::{
        Renderer,
        settings::{GraphicsPipelineOptions, Settings},
        vulkan::{
            VulkanRenderer,
            canvas::{VulkanColorCanvas, VulkanDepthCanvas},
            pipeline::VulkanGraphicsPipeline,
            rendering::{
                dynamic_rendering::DynamicRendering, render_pass_rendering::RenderPassRendering,
            },
            swapchain::SwapchainContext,
        },
    },
    shader::Shader,
};

/// Provides rendering operations.
pub(super) trait VulkanRendering {
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
        &self,
        device: &ash::Device,
        shaders: &[&Shader],
        options: &GraphicsPipelineOptions,
        enabled_features: &HashSet<GraphicsDeviceFeature>,
        max_tessellation_patch_size: u32,
    ) -> Result<VulkanGraphicsPipeline>;
    /// Destroys the graphics pipeline.
    fn destroy_graphics_pipeline(
        &self,
        device: &ash::Device,
        pipeline: &mut VulkanGraphicsPipeline,
    ) -> Result<()>;
    /// Creates the render pass and the framebuffer of the canvas if the rendering method requires
    /// them.
    fn create_color_canvas_framebuffer(
        &self,
        device: &ash::Device,
        canvas: &mut VulkanColorCanvas,
    ) -> Result<()>;
    /// Creates the render pass and the framebuffer of the depth-only canvas if the rendering method
    /// requires them.
    fn create_depth_canvas_framebuffer(
        &self,
        device: &ash::Device,
        canvas: &mut VulkanDepthCanvas,
    ) -> Result<()>;
    /// Starts rendering into the canvas, keeping its color and depth.
    fn begin(
        &mut self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        canvas: &VulkanColorCanvas,
    ) -> Result<()>;
    /// Starts rendering into the depth-only canvas, keeping its depth.
    fn begin_depth(
        &mut self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        canvas: &VulkanDepthCanvas,
    ) -> Result<()>;
    /// Ends rendering into the canvas.
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
        let display_canvas = device_context.display_canvas.take();
        if let Some(mut rendering) = device_context.rendering.take() {
            rendering.destroy(&device_context.logical_device)?;
        }
        if let Some(mut display_canvas) = display_canvas {
            self.destroy_canvas(&mut display_canvas)?;
        }
        Ok(())
    }

    /// Ends rendering into the canvas that has the provided image as its texture, so the texture
    /// can be sampled by shaders. The aspect of the image is depth if the canvas is depth-only.
    pub(super) fn end_rendering(
        device: &ash::Device,
        rendering: &mut dyn VulkanRendering,
        command_buffer: CommandBuffer,
        image: ash::vk::Image,
        aspect_mask: ash::vk::ImageAspectFlags,
        layer_count: u32,
        sampling_stages: ash::vk::PipelineStageFlags,
    ) -> Result<()> {
        rendering.end(device, command_buffer)?;
        let (old_layout, src_access_mask, src_stage_mask) = match aspect_mask {
            ash::vk::ImageAspectFlags::DEPTH => (
                ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                ash::vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                ash::vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                    | ash::vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            ),
            _ => (
                ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            ),
        };
        Self::transition_image_layout(
            device,
            command_buffer,
            image,
            old_layout,
            ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            src_access_mask,
            ash::vk::AccessFlags::SHADER_READ,
            src_stage_mask,
            sampling_stages,
            aspect_mask,
            0,
            1,
            layer_count,
        );
        Ok(())
    }
}
