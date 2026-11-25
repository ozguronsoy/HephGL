mod attachment_image;
mod color;
mod depth;

use std::any::Any;

use ash::vk::{
    CommandBuffer, Extent2D, Format, FormatFeatureFlags, ImageAspectFlags, PipelineStageFlags,
};

pub(super) use crate::renderers::vulkan::canvas::{
    color::VulkanColorCanvas, depth::VulkanDepthCanvas,
};
use crate::{
    Error, Result,
    renderers::{
        RendererCanvas, RendererResource, downcast_mut,
        resources::TextureFormat,
        settings::{CanvasOptions, Settings},
        vulkan::{
            VulkanRenderer, device::DeviceContext, rendering::VulkanRendering,
            resources::VulkanTexture, thread::GraphicsThreadContext,
        },
    },
};

/// Provides the operations that depend on the images of a canvas.
pub(super) trait VulkanCanvas: RendererCanvas {
    /// Returns the textures of the canvas and the aspect of their images.
    fn render_target(&self) -> (&[VulkanTexture], ImageAspectFlags);
    /// Returns the number of array layers of the canvas images.
    fn layer_count(&self) -> u32;
    /// Checks whether the canvas can be rendered into with the current settings.
    fn is_compatible(&self, settings: &Settings, swapchain_format: Format) -> bool;
    /// Prepares the canvas images for rendering and starts rendering into them.
    fn begin_rendering(
        &self,
        device: &ash::Device,
        rendering: &dyn VulkanRendering,
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
        return Ok(downcast_mut::<VulkanColorCanvas>(canvas)?);
    }
    if canvas.is::<VulkanDepthCanvas>() {
        return Ok(downcast_mut::<VulkanDepthCanvas>(canvas)?);
    }
    Err(Error::invalid_argument(
        "Type mismatch: expected a Vulkan canvas.",
    ))
}

impl VulkanRenderer {
    /// Creates a canvas using the provided options and clears it in the current thread's graphics
    /// command buffer.
    pub(super) fn create_vulkan_canvas(
        &self,
        options: &CanvasOptions,
    ) -> Result<Box<dyn VulkanCanvas>> {
        let instance = self
            .instance
            .as_ref()
            .ok_or(Error::invalid_operation("Renderer is not initialized."))?;
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;

        if options.width == 0 || options.height == 0 {
            return Err(Error::invalid_argument(
                "Canvas dimensions must be greater than zero.",
            ));
        }

        let limits = unsafe {
            instance
                .get_physical_device_properties(device_context.physical_device)
                .limits
        };
        let max_width = limits
            .max_image_dimension2_d
            .min(limits.max_framebuffer_width);
        let max_height = limits
            .max_image_dimension2_d
            .min(limits.max_framebuffer_height);
        if options.width > max_width || options.height > max_height {
            return Err(Error::InvalidArgument(format!(
                "Canvas dimensions cannot exceed {max_width}x{max_height}."
            )));
        }

        let current_frame = unsafe {
            &mut *device_context.graphics_queue_context.frames
                [device_context.current_frame_index.get()]
            .base
            .thread_contexts[thread_context_index]
                .get()
        };
        current_frame.base.debug_label |=
            Self::begin_labels(self.debugger.as_deref(), current_frame.base.command_buffer)?;

        let layer_count = self.settings.array_layer_count();
        let mut canvas: Box<dyn VulkanCanvas> = match options.depth_only {
            true => Box::new(VulkanDepthCanvas::new(
                device_context,
                options,
                layer_count,
            )?),
            false => Box::new(VulkanColorCanvas::new(
                device_context,
                options,
                &Self::resolve_texture_formats(
                    instance,
                    device_context,
                    &options.texture_formats,
                    FormatFeatureFlags::COLOR_ATTACHMENT
                        | FormatFeatureFlags::SAMPLED_IMAGE
                        | FormatFeatureFlags::TRANSFER_DST,
                )?,
                self.settings.msaa,
                layer_count,
            )?),
        };
        if let Err(error) = match unsafe { &*device_context.rendering.get() }.as_ref() {
            Some(rendering) => Self::clear_new_canvas(
                &device_context.logical_device,
                rendering.as_ref(),
                current_frame,
                canvas.as_ref(),
                device_context.sampling_stages,
            ),
            None => Err(Error::invalid_operation("Device is not set.")),
        } {
            canvas.destroy(device_context);
            return Err(error);
        }

        Ok(canvas)
    }

    /// Starts rendering into the canvas, ending the rendering into the previous canvas if there is
    /// one.
    pub(super) fn begin_canvas_rendering(
        device: &ash::Device,
        rendering: &dyn VulkanRendering,
        swapchain_format: Format,
        settings: &Settings,
        thread_context: &mut GraphicsThreadContext,
        canvas: &dyn VulkanCanvas,
        sampling_stages: PipelineStageFlags,
    ) -> Result<()> {
        let (textures, aspect_mask) = canvas.render_target();
        if thread_context
            .render_target_images
            .iter()
            .eq(textures.iter().map(|texture| &texture.image))
        {
            return Ok(());
        }
        if textures.iter().any(|texture| !texture.is_allocated()) {
            return Err(Error::invalid_argument("Canvas is not allocated."));
        }
        if !canvas.is_compatible(settings, swapchain_format) {
            return Err(Error::invalid_operation(
                "Canvas is incompatible with the current settings and must be recreated.",
            ));
        }
        Self::end_rendering(
            device,
            rendering,
            thread_context,
            canvas.layer_count(),
            sampling_stages,
        )?;

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

        thread_context
            .render_target_images
            .extend(textures.iter().map(|texture| texture.image));
        thread_context.render_target_aspect_mask = aspect_mask;
        thread_context.base.recorded = true;
        Ok(())
    }

    /// Clears the images of a newly created canvas, ending the rendering into the previous canvas
    /// if there is one.
    pub(super) fn clear_new_canvas(
        device: &ash::Device,
        rendering: &dyn VulkanRendering,
        thread_context: &mut GraphicsThreadContext,
        canvas: &dyn VulkanCanvas,
        sampling_stages: PipelineStageFlags,
    ) -> Result<()> {
        Self::end_rendering(
            device,
            rendering,
            thread_context,
            canvas.layer_count(),
            sampling_stages,
        )?;
        canvas.clear_new(device, thread_context.base.command_buffer, sampling_stages);
        thread_context.base.recorded = true;
        Ok(())
    }

    /// Replaces the display texture format in the texture formats of a canvas or a pipeline with
    /// the swapchain format, and checks whether the current device supports them.
    pub(super) fn resolve_texture_formats(
        instance: &ash::Instance,
        device_context: &DeviceContext,
        texture_formats: &[Option<TextureFormat>],
        required_features: FormatFeatureFlags,
    ) -> Result<Vec<TextureFormat>> {
        if texture_formats.is_empty() {
            return Err(Error::invalid_argument(
                "At least one texture format is required.",
            ));
        }
        let max_color_attachments = unsafe {
            instance
                .get_physical_device_properties(device_context.physical_device)
                .limits
                .max_color_attachments
        };
        if texture_formats.len() > max_color_attachments as usize {
            return Err(Error::InvalidArgument(format!(
                "The number of texture formats cannot exceed {max_color_attachments}."
            )));
        }
        texture_formats
            .iter()
            .map(|texture_format| {
                let Some(texture_format) = *texture_format else {
                    return TextureFormat::try_from(
                        unsafe { &*device_context.swapchain_context.get() }.format,
                    );
                };
                let format_properties = unsafe {
                    instance.get_physical_device_format_properties(
                        device_context.physical_device,
                        texture_format.into(),
                    )
                };
                if !format_properties
                    .optimal_tiling_features
                    .contains(required_features)
                {
                    return Err(Error::InvalidOperation(format!(
                        "The device does not support rendering into `{texture_format:?}` textures."
                    )));
                }
                Ok(texture_format)
            })
            .collect::<Result<Vec<_>>>()
    }
}
