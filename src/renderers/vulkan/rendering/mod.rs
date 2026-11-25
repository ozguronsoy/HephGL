mod dynamic_rendering;
mod render_pass_rendering;

use ash::vk::CommandBuffer;

use crate::{
    Error, Result,
    renderers::{
        Renderer,
        settings::{GraphicsPipelineOptions, Settings},
        vulkan::{
            VulkanRenderer,
            canvas::{VulkanColorCanvas, VulkanDepthCanvas},
            device::DeviceContext,
            pipeline::VulkanGraphicsPipeline,
            rendering::{
                dynamic_rendering::DynamicRendering, render_pass_rendering::RenderPassRendering,
            },
            thread::GraphicsThreadContext,
        },
    },
    shader::Shader,
};

/// Provides rendering operations.
pub(super) trait VulkanRendering {
    /// Creates the resources required for rendering.
    fn new(settings: &Settings, device: &ash::Device) -> Result<Self>
    where
        Self: Sized;
    /// Creates a graphics pipeline using the provided shaders.
    fn create_graphics_pipeline(
        &self,
        instance: &ash::Instance,
        device_context: &DeviceContext,
        shaders: &[&Shader],
        options: &GraphicsPipelineOptions,
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
        &self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        canvas: &VulkanColorCanvas,
    ) -> Result<()>;
    /// Starts rendering into the depth-only canvas, keeping its depth.
    fn begin_depth(
        &self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        canvas: &VulkanDepthCanvas,
    ) -> Result<()>;
    /// Ends rendering into the canvas.
    fn end(&self, device: &ash::Device, command_buffer: CommandBuffer) -> Result<()>;
    /// Frees the resources.
    fn destroy(&mut self, device: &ash::Device) -> Result<()>;
}

impl VulkanRenderer {
    /// Creates rendering resources for the current device.
    pub(super) fn create_rendering(&self) -> Result<()> {
        self.destroy_rendering()?;

        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;

        let rendering: Box<dyn VulkanRendering> = if device_context.supports_dynamic_rendering {
            Box::new(DynamicRendering::new(
                &self.settings,
                &device_context.logical_device,
            )?)
        } else {
            Box::new(RenderPassRendering::new(
                &self.settings,
                &device_context.logical_device,
            )?)
        };
        unsafe { *device_context.rendering.get() = Some(rendering) };

        Ok(())
    }

    /// Destroys the rendering resources.
    pub(super) fn destroy_rendering(&self) -> Result<()> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let display_canvas = unsafe { &mut *device_context.display_canvas.get() }.take();
        if let Some(mut rendering) = unsafe { &mut *device_context.rendering.get() }.take() {
            rendering.destroy(&device_context.logical_device)?;
        }
        if let Some(mut display_canvas) = display_canvas {
            self.destroy_canvas(&mut display_canvas)?;
        }
        Ok(())
    }

    /// Ends rendering into the canvas the thread context is rendering into, if there is one, so its
    /// textures can be sampled by shaders.
    pub(super) fn end_rendering(
        device: &ash::Device,
        rendering: &dyn VulkanRendering,
        thread_context: &mut GraphicsThreadContext,
        layer_count: u32,
        sampling_stages: ash::vk::PipelineStageFlags,
    ) -> Result<()> {
        if thread_context.render_target_images.is_empty() {
            return Ok(());
        }
        let command_buffer = thread_context.base.command_buffer;
        let aspect_mask = thread_context.render_target_aspect_mask;
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
        for image in thread_context.render_target_images.drain(..) {
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
        }
        Ok(())
    }
}
