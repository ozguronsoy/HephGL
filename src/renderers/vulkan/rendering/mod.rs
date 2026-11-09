mod dynamic_rendering;
mod lifetime_guards;
mod render_pass_rendering;

use ash::vk::{CommandBuffer, PipelineBindPoint};
use renkrs::RGB;

use crate::{
    Error, Result,
    graphics_device::GraphicsDeviceFeature,
    renderers::{
        BufferUsage, Renderer, RendererCanvas, ResourceBinding,
        pipeline::RendererGraphicsPipeline,
        resource_mut, resource_ref,
        resources::{RendererBuffer, ResourceBindingType},
        settings::{GraphicsPipelineOptions, Settings},
        vulkan::{
            VulkanRenderer,
            canvas::VulkanCanvas,
            frame::FrameState,
            pipeline::VulkanGraphicsPipeline,
            rendering::{
                dynamic_rendering::DynamicRendering, render_pass_rendering::RenderPassRendering,
            },
            resources::VulkanBuffer,
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
        enabled_features: &[GraphicsDeviceFeature],
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
    fn create_canvas_framebuffer(
        &self,
        device: &ash::Device,
        canvas: &mut VulkanCanvas,
    ) -> Result<()>;
    /// Starts rendering into the canvas, keeping its color and depth.
    fn begin(
        &mut self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        canvas: &VulkanCanvas,
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
    /// can be sampled by shaders.
    pub(super) fn end_rendering(
        device: &ash::Device,
        rendering: &mut dyn VulkanRendering,
        command_buffer: CommandBuffer,
        image: ash::vk::Image,
        layer_count: u32,
        sampling_stages: ash::vk::PipelineStageFlags,
    ) -> Result<()> {
        rendering.end(device, command_buffer)?;
        Self::transition_image_layout(
            device,
            command_buffer,
            image,
            ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            ash::vk::AccessFlags::SHADER_READ,
            ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            sampling_stages,
            ash::vk::ImageAspectFlags::COLOR,
            0,
            1,
            layer_count,
        );
        Ok(())
    }

    /// Records a draw command that renders into the canvas, or into the display canvas if there is
    /// no canvas.
    pub(super) fn record_draw(
        &mut self,
        canvas: Option<&mut VulkanCanvas>,
        pipeline: &mut dyn RendererGraphicsPipeline,
        index_buffer: Option<&dyn RendererBuffer>,
        vertex_bindings: &[ResourceBinding<'_>],
        binding_sets: &[&[ResourceBinding<'_>]],
        draw_count: u32,
        instance_count: u32,
    ) -> Result<()> {
        let pipeline = resource_mut::<VulkanGraphicsPipeline>(pipeline)?;
        let mapped_range = self.create_graphics_resource_sets(pipeline, binding_sets)?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let rendering = device_context
            .rendering
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let graphics_queue_context = &mut device_context.graphics_queue_context;
        let thread_context_index = Self::thread_context_index()?;
        let current_frame = &mut graphics_queue_context.frames[device_context.current_frame_index]
            .base
            .thread_contexts[thread_context_index];
        let canvas = match canvas {
            Some(canvas) => canvas,
            None => device_context
                .display_canvas
                .as_mut()
                .ok_or(Error::invalid_operation("Device is not set."))?,
        };
        Self::begin_canvas_rendering(
            &device_context.logical_device,
            rendering.as_mut(),
            device_context.swapchain_context.format,
            &self.settings,
            current_frame,
            canvas,
            device_context.sampling_stages,
        )?;
        unsafe {
            if current_frame.bound_pipeline != Some(pipeline.pipeline) {
                device_context.logical_device.cmd_bind_pipeline(
                    current_frame.base.command_buffer,
                    PipelineBindPoint::GRAPHICS,
                    pipeline.pipeline,
                );
                current_frame.bound_pipeline = Some(pipeline.pipeline);
            }
            if !mapped_range.is_empty() {
                device_context.logical_device.cmd_bind_descriptor_sets(
                    current_frame.base.command_buffer,
                    PipelineBindPoint::GRAPHICS,
                    pipeline.layout,
                    0,
                    &pipeline.descriptor_set_cache.descriptor_sets[mapped_range],
                    &[],
                );
            }
            for vertex_binding in vertex_bindings {
                let ResourceBindingType::Buffer {
                    buffer,
                    offset,
                    size,
                } = vertex_binding.resource
                else {
                    return Err(Error::invalid_argument(
                        "Vertex binding type must be a buffer",
                    ));
                };
                if buffer.usage() != BufferUsage::Vertex {
                    return Err(Error::invalid_argument(
                        "Vertex buffer's usage must be `BufferUsage::Vertex`.",
                    ));
                }

                let vertex_buffer = resource_ref::<VulkanBuffer>(buffer)?;
                if offset
                    .checked_add(size)
                    .is_none_or(|x| x > vertex_buffer.size)
                {
                    return Err(Error::invalid_argument(
                        "Buffer overflow while binding the vertex buffer.",
                    ));
                }
                device_context.logical_device.cmd_bind_vertex_buffers(
                    current_frame.base.command_buffer,
                    vertex_binding.binding,
                    std::slice::from_ref(&vertex_buffer.buffer),
                    std::slice::from_ref(&(offset as u64)),
                );
            }

            if let Some(index_buffer) = index_buffer {
                if index_buffer.usage() != BufferUsage::Index {
                    return Err(Error::invalid_argument(
                        "Index buffer's usage must be `BufferUsage::Index`.",
                    ));
                }

                let index_buffer = resource_ref::<VulkanBuffer>(index_buffer)?;
                device_context.logical_device.cmd_bind_index_buffer(
                    current_frame.base.command_buffer,
                    index_buffer.buffer,
                    0,
                    ash::vk::IndexType::UINT32,
                );
                device_context.logical_device.cmd_draw_indexed(
                    current_frame.base.command_buffer,
                    draw_count,
                    instance_count,
                    0,
                    0,
                    0,
                );
            } else {
                device_context.logical_device.cmd_draw(
                    current_frame.base.command_buffer,
                    draw_count,
                    instance_count,
                    0,
                    0,
                );
            }
        }

        current_frame.base.recorded = true;
        Ok(())
    }

    /// Records a command that clears the color and depth of the canvas, or of the display canvas if
    /// there is no canvas.
    pub(super) fn record_clear(
        &mut self,
        canvas: Option<&mut VulkanCanvas>,
        color: RGB<f32>,
    ) -> Result<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let rendering = device_context
            .rendering
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;
        let current_frame = &mut device_context.graphics_queue_context.frames
            [device_context.current_frame_index]
            .base
            .thread_contexts[thread_context_index];

        {
            let state = current_frame.base.sync_state.0.lock()?;
            if *state != FrameState::Started {
                return Err(Error::invalid_operation("Cannot clear outside of a frame."));
            }
        }

        let canvas = match canvas {
            Some(canvas) => canvas,
            None => device_context
                .display_canvas
                .as_mut()
                .ok_or(Error::invalid_operation("Device is not set."))?,
        };
        Self::begin_canvas_rendering(
            &device_context.logical_device,
            rendering.as_mut(),
            device_context.swapchain_context.format,
            &self.settings,
            current_frame,
            canvas,
            device_context.sampling_stages,
        )?;

        let clear_attachments = [
            ash::vk::ClearAttachment::default()
                .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
                .color_attachment(0)
                .clear_value(ash::vk::ClearValue {
                    color: ash::vk::ClearColorValue {
                        float32: [color.r, color.g, color.b, 1.0],
                    },
                }),
            ash::vk::ClearAttachment::default()
                .aspect_mask(ash::vk::ImageAspectFlags::DEPTH)
                .clear_value(ash::vk::ClearValue {
                    depth_stencil: ash::vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                }),
        ];
        let clear_rect = ash::vk::ClearRect::default()
            .rect(ash::vk::Rect2D {
                offset: ash::vk::Offset2D { x: 0, y: 0 },
                extent: ash::vk::Extent2D {
                    width: canvas.width(),
                    height: canvas.height(),
                },
            })
            .base_array_layer(0)
            .layer_count(canvas.layer_count);
        unsafe {
            device_context.logical_device.cmd_clear_attachments(
                current_frame.base.command_buffer,
                &clear_attachments,
                std::slice::from_ref(&clear_rect),
            );
        }

        current_frame.base.recorded = true;
        Ok(())
    }
}
