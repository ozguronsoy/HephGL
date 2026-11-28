use std::mem::DropGuard;

use ash::vk::{
    AttachmentDescription, AttachmentLoadOp, AttachmentReference, AttachmentStoreOp, CommandBuffer,
    FramebufferCreateInfo, PipelineBindPoint, PipelineCache, RenderPass, RenderPassBeginInfo,
    RenderPassCreateInfo, SampleCountFlags, SubpassContents, SubpassDescription,
};

use crate::{
    Result,
    renderers::{
        RendererCanvas, TextureFormat,
        id::RendererId,
        pipeline::GraphicsPipelineOptions,
        settings::{Msaa, Settings},
        vulkan::{
            canvas::{VulkanColorCanvas, VulkanDepthCanvas},
            device::DeviceContext,
            pipeline::VulkanGraphicsPipeline,
            rendering::VulkanRendering,
        },
    },
    shader::Shader,
};

/// Implements rendering via render pass.
pub(super) struct RenderPassRendering {
    msaa: Msaa,
}

impl VulkanRendering for RenderPassRendering {
    fn new(settings: &Settings, _: &ash::Device) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            msaa: settings.msaa,
        })
    }
    fn create_graphics_pipeline(
        &self,
        instance: &ash::Instance,
        device_context: &DeviceContext,
        shaders: &[&Shader],
        options: &GraphicsPipelineOptions,
        renderer_id: RendererId,
    ) -> Result<VulkanGraphicsPipeline> {
        let device = &device_context.logical_device;
        super::create_graphics_pipeline(
            instance,
            device_context,
            shaders,
            options,
            renderer_id,
            self.msaa,
            |pipeline_info, color_formats| {
                // The render pass only describes the canvases the pipeline is compatible with, thus
                // it can be destroyed once the pipeline is created.
                let render_pass = DropGuard::new(
                    match options.depth_only {
                        true => Self::create_depth_render_pass(device)?,
                        false => Self::create_render_pass(device, color_formats, self.msaa.into())?,
                    },
                    |render_pass| unsafe {
                        device.destroy_render_pass(render_pass, None);
                    },
                );
                let pipeline_info = pipeline_info.render_pass(*render_pass).subpass(0);
                Ok(unsafe {
                    device.create_graphics_pipelines(
                        PipelineCache::null(),
                        &[pipeline_info],
                        None,
                    )?[0]
                })
            },
        )
    }
    fn create_color_canvas_framebuffer(
        &self,
        device: &ash::Device,
        canvas: &mut VulkanColorCanvas,
    ) -> Result<()> {
        let color_formats = canvas
            .textures
            .iter()
            .map(|texture| texture.texture_options.format)
            .collect::<Vec<_>>();
        canvas.render_pass = Self::create_render_pass(device, &color_formats, canvas.msaa.into())?;
        let views = match canvas.msaa_color_images.is_empty() {
            true => canvas
                .textures
                .iter()
                .map(|texture| texture.image_view)
                .chain([canvas.depth_image.view])
                .collect::<Vec<_>>(),
            false => canvas
                .msaa_color_images
                .iter()
                .map(|msaa_color_image| msaa_color_image.view)
                .chain([canvas.depth_image.view])
                .chain(canvas.textures.iter().map(|texture| texture.image_view))
                .collect::<Vec<_>>(),
        };
        canvas.framebuffer = unsafe {
            device.create_framebuffer(
                &FramebufferCreateInfo::default()
                    .render_pass(canvas.render_pass)
                    .attachments(&views)
                    .width(canvas.width())
                    .height(canvas.height())
                    .layers(canvas.layer_count),
                None,
            )?
        };
        Ok(())
    }
    fn create_depth_canvas_framebuffer(
        &self,
        device: &ash::Device,
        canvas: &mut VulkanDepthCanvas,
    ) -> Result<()> {
        canvas.render_pass = Self::create_depth_render_pass(device)?;
        canvas.framebuffer = unsafe {
            device.create_framebuffer(
                &FramebufferCreateInfo::default()
                    .render_pass(canvas.render_pass)
                    .attachments(std::slice::from_ref(&canvas.texture.image_view))
                    .width(canvas.width())
                    .height(canvas.height())
                    .layers(canvas.layer_count),
                None,
            )?
        };
        Ok(())
    }
    fn begin(
        &self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        canvas: &VulkanColorCanvas,
    ) -> Result<()> {
        let begin_info = RenderPassBeginInfo::default()
            .render_pass(canvas.render_pass)
            .framebuffer(canvas.framebuffer)
            .render_area(ash::vk::Rect2D {
                offset: ash::vk::Offset2D { x: 0, y: 0 },
                extent: ash::vk::Extent2D {
                    width: canvas.width(),
                    height: canvas.height(),
                },
            });
        unsafe {
            device.cmd_begin_render_pass(command_buffer, &begin_info, SubpassContents::INLINE);
        }
        Ok(())
    }
    fn begin_depth(
        &self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        canvas: &VulkanDepthCanvas,
    ) -> Result<()> {
        let begin_info = RenderPassBeginInfo::default()
            .render_pass(canvas.render_pass)
            .framebuffer(canvas.framebuffer)
            .render_area(ash::vk::Rect2D {
                offset: ash::vk::Offset2D { x: 0, y: 0 },
                extent: ash::vk::Extent2D {
                    width: canvas.width(),
                    height: canvas.height(),
                },
            });
        unsafe {
            device.cmd_begin_render_pass(command_buffer, &begin_info, SubpassContents::INLINE);
        }
        Ok(())
    }
    fn end(&self, device: &ash::Device, command_buffer: CommandBuffer) -> Result<()> {
        unsafe {
            device.cmd_end_render_pass(command_buffer);
        }
        Ok(())
    }
    fn destroy(&mut self, _: &ash::Device) -> Result<()> {
        Ok(())
    }
}

impl RenderPassRendering {
    /// Creates a render pass that renders into a canvas. The colors and depth are loaded so the
    /// canvas keeps its content between render passes. Render passes created with the same color
    /// formats and sample count are compatible with each other.
    fn create_render_pass(
        device: &ash::Device,
        color_formats: &[TextureFormat],
        sample_count: SampleCountFlags,
    ) -> Result<RenderPass> {
        let color_attachments = color_formats
            .iter()
            .map(|color_format| {
                AttachmentDescription::default()
                    .format((*color_format).into())
                    .samples(sample_count)
                    .load_op(AttachmentLoadOp::LOAD)
                    .store_op(AttachmentStoreOp::STORE)
                    .stencil_load_op(AttachmentLoadOp::DONT_CARE)
                    .stencil_store_op(AttachmentStoreOp::DONT_CARE)
                    .initial_layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .final_layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            })
            .collect::<Vec<_>>();
        let depth_attachment = AttachmentDescription::default()
            .format(ash::vk::Format::D32_SFLOAT)
            .samples(sample_count)
            .load_op(AttachmentLoadOp::LOAD)
            .store_op(AttachmentStoreOp::STORE)
            .stencil_load_op(AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(AttachmentStoreOp::DONT_CARE)
            .initial_layout(ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .final_layout(ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let color_count = color_formats.len() as u32;
        let color_references = (0..color_count)
            .map(|index| {
                AttachmentReference::default()
                    .attachment(index)
                    .layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            })
            .collect::<Vec<_>>();
        let depth_reference = AttachmentReference::default()
            .attachment(color_count)
            .layout(ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let resolve_references = (0..color_count)
            .map(|index| {
                AttachmentReference::default()
                    .attachment(color_count + 1 + index)
                    .layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            })
            .collect::<Vec<_>>();
        let mut subpass = SubpassDescription::default()
            .pipeline_bind_point(PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_references)
            .depth_stencil_attachment(&depth_reference);
        let mut attachments = color_attachments.clone();
        attachments.push(depth_attachment);
        if sample_count != SampleCountFlags::TYPE_1 {
            subpass = subpass.resolve_attachments(&resolve_references);
            attachments.extend(color_attachments.iter().map(|color_attachment| {
                color_attachment
                    .samples(SampleCountFlags::TYPE_1)
                    .load_op(AttachmentLoadOp::DONT_CARE)
            }));
        }
        unsafe {
            Ok(device.create_render_pass(
                &RenderPassCreateInfo::default()
                    .attachments(&attachments)
                    .subpasses(std::slice::from_ref(&subpass)),
                None,
            )?)
        }
    }

    fn create_depth_render_pass(device: &ash::Device) -> Result<RenderPass> {
        let depth_attachment = AttachmentDescription::default()
            .format(ash::vk::Format::D32_SFLOAT)
            .samples(SampleCountFlags::TYPE_1)
            .load_op(AttachmentLoadOp::LOAD)
            .store_op(AttachmentStoreOp::STORE)
            .stencil_load_op(AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(AttachmentStoreOp::DONT_CARE)
            .initial_layout(ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .final_layout(ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let depth_reference = AttachmentReference::default()
            .attachment(0)
            .layout(ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let subpass = SubpassDescription::default()
            .pipeline_bind_point(PipelineBindPoint::GRAPHICS)
            .depth_stencil_attachment(&depth_reference);
        unsafe {
            Ok(device.create_render_pass(
                &RenderPassCreateInfo::default()
                    .attachments(std::slice::from_ref(&depth_attachment))
                    .subpasses(std::slice::from_ref(&subpass)),
                None,
            )?)
        }
    }
}
