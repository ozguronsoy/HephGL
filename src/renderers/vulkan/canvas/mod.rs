mod attachment_image;
mod color;
mod depth;

use std::any::Any;

use ash::vk::{CommandBuffer, Extent2D, Format, ImageAspectFlags, PipelineStageFlags};

pub(super) use crate::renderers::vulkan::canvas::{
    color::VulkanColorCanvas, depth::VulkanDepthCanvas,
};
use crate::{
    Error, Result,
    renderers::{
        RendererCanvas, resource_mut,
        settings::Settings,
        vulkan::{
            VulkanRenderer, device::DeviceContext, rendering::VulkanRendering,
            thread::GraphicsThreadContext,
        },
    },
};

/// Provides the operations that depend on the images of a canvas.
pub(super) trait VulkanCanvas: RendererCanvas {
    /// Returns the image of the canvas texture and the aspect of that image.
    fn render_target(&self) -> (ash::vk::Image, ImageAspectFlags);
    /// Returns the number of array layers of the canvas images.
    fn layer_count(&self) -> u32;
    /// Checks whether the canvas can be rendered into with the current settings.
    fn is_compatible(&self, settings: &Settings, swapchain_format: Format) -> bool;
    /// Prepares the canvas images for rendering and starts rendering into them.
    fn begin_rendering(
        &self,
        device: &ash::Device,
        rendering: &mut dyn VulkanRendering,
        command_buffer: CommandBuffer,
        sampling_stages: PipelineStageFlags,
    ) -> Result<()>;
    /// Clears the images of the newly created canvas, so its texture can be sampled by shaders.
    fn clear_new(
        &self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        sampling_stages: PipelineStageFlags,
    );
    /// Frees the resources allocated for the canvas.
    fn destroy(&mut self, device_context: &DeviceContext);
}

/// Downcasts the mutable reference to the Vulkan canvas it refers to.
pub(super) fn vulkan_canvas_mut(canvas: &mut dyn RendererCanvas) -> Result<&mut dyn VulkanCanvas> {
    let canvas: &mut dyn Any = canvas;
    if canvas.is::<VulkanColorCanvas>() {
        return Ok(resource_mut::<VulkanColorCanvas>(canvas)?);
    }
    if canvas.is::<VulkanDepthCanvas>() {
        return Ok(resource_mut::<VulkanDepthCanvas>(canvas)?);
    }
    Err(Error::invalid_argument(
        "Type mismatch: expected a Vulkan canvas.",
    ))
}

impl VulkanRenderer {
    /// Starts rendering into the canvas, ending the rendering into the previous canvas if there is
    /// one.
    pub(super) fn begin_canvas_rendering(
        device: &ash::Device,
        rendering: &mut dyn VulkanRendering,
        swapchain_format: Format,
        settings: &Settings,
        thread_context: &mut GraphicsThreadContext,
        canvas: &mut dyn VulkanCanvas,
        sampling_stages: PipelineStageFlags,
    ) -> Result<()> {
        let render_target = canvas.render_target();
        if thread_context.render_target == Some(render_target) {
            return Ok(());
        }
        if !canvas.texture().is_allocated() {
            return Err(Error::invalid_argument("Canvas is not allocated."));
        }
        if !canvas.is_compatible(settings, swapchain_format) {
            return Err(Error::invalid_operation(
                "Canvas is incompatible with the current settings and must be recreated.",
            ));
        }
        if let Some((previous_image, previous_aspect_mask)) = thread_context.render_target.take() {
            Self::end_rendering(
                device,
                rendering,
                thread_context.base.command_buffer,
                previous_image,
                previous_aspect_mask,
                canvas.layer_count(),
                sampling_stages,
            )?;
        }

        let command_buffer = thread_context.base.command_buffer;
        canvas.begin_rendering(device, rendering, command_buffer, sampling_stages)?;

        let extent = Extent2D {
            width: canvas.width(),
            height: canvas.height(),
        };
        let viewport = ash::vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: extent.width as f32,
            height: extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        let scissor = ash::vk::Rect2D {
            offset: ash::vk::Offset2D { x: 0, y: 0 },
            extent,
        };
        unsafe {
            device.cmd_set_viewport(command_buffer, 0, std::slice::from_ref(&viewport));
            device.cmd_set_scissor(command_buffer, 0, std::slice::from_ref(&scissor));
        }

        thread_context.render_target = Some(render_target);
        thread_context.base.recorded = true;
        Ok(())
    }

    /// Clears the images of a newly created canvas, ending the rendering into the previous canvas
    /// if there is one.
    pub(super) fn clear_new_canvas(
        device: &ash::Device,
        rendering: &mut dyn VulkanRendering,
        thread_context: &mut GraphicsThreadContext,
        canvas: &dyn VulkanCanvas,
        sampling_stages: PipelineStageFlags,
    ) -> Result<()> {
        if let Some((previous_image, previous_aspect_mask)) = thread_context.render_target.take() {
            Self::end_rendering(
                device,
                rendering,
                thread_context.base.command_buffer,
                previous_image,
                previous_aspect_mask,
                canvas.layer_count(),
                sampling_stages,
            )?;
        }
        canvas.clear_new(device, thread_context.base.command_buffer, sampling_stages);
        thread_context.base.recorded = true;
        Ok(())
    }
}
