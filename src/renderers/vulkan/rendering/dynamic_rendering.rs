use ash::vk::{
    AttachmentLoadOp, AttachmentStoreOp, CommandBuffer, ImageLayout, PipelineCache,
    PipelineRenderingCreateInfo, RenderingAttachmentInfo, RenderingInfo,
};

use crate::{
    Result,
    renderers::{
        RendererCanvas,
        id::RendererId,
        settings::{GraphicsPipelineOptions, Msaa, Settings},
        vulkan::{
            canvas::{VulkanColorCanvas, VulkanDepthCanvas},
            device::DeviceContext,
            pipeline::VulkanGraphicsPipeline,
            rendering::VulkanRendering,
        },
    },
    shader::Shader,
};

/// Implements dynamic rendering.
pub(super) struct DynamicRendering {
    depth_format: ash::vk::Format,
    msaa: Msaa,
}

impl VulkanRendering for DynamicRendering {
    fn new(settings: &Settings, _: &ash::Device) -> Result<Self>
    where
        Self: Sized,
    {
        // This is recreated when settings change, so we can store the settings we need here.
        Ok(Self {
            // TODO: Get this from swapchain context?
            depth_format: ash::vk::Format::D32_SFLOAT,
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
        super::create_graphics_pipeline(
            instance,
            device_context,
            shaders,
            options,
            renderer_id,
            self.msaa,
            |pipeline_info, color_formats| {
                let vulkan_color_formats = color_formats
                    .iter()
                    .map(|color_format| ash::vk::Format::from(*color_format))
                    .collect::<Vec<_>>();
                let mut rendering_info = PipelineRenderingCreateInfo::default()
                    .color_attachment_formats(&vulkan_color_formats)
                    .depth_attachment_format(self.depth_format);
                let pipeline_info = pipeline_info
                    .render_pass(ash::vk::RenderPass::null())
                    .push_next(&mut rendering_info);
                Ok(unsafe {
                    device_context.logical_device.create_graphics_pipelines(
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
        _: &ash::Device,
        _: &mut VulkanColorCanvas,
    ) -> Result<()> {
        Ok(())
    }
    fn create_depth_canvas_framebuffer(
        &self,
        _: &ash::Device,
        _: &mut VulkanDepthCanvas,
    ) -> Result<()> {
        Ok(())
    }
    fn begin(
        &self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        canvas: &VulkanColorCanvas,
    ) -> Result<()> {
        let color_attachment_infos = canvas
            .textures
            .iter()
            .enumerate()
            .map(
                |(index, texture)| match canvas.msaa_color_images.get(index) {
                    None => RenderingAttachmentInfo::default()
                        .image_view(texture.image_view)
                        .image_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .load_op(AttachmentLoadOp::LOAD)
                        .store_op(AttachmentStoreOp::STORE),
                    Some(msaa_color_image) => RenderingAttachmentInfo::default()
                        .image_view(msaa_color_image.view)
                        .image_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .load_op(AttachmentLoadOp::LOAD)
                        .store_op(AttachmentStoreOp::STORE)
                        .resolve_mode(ash::vk::ResolveModeFlags::AVERAGE)
                        .resolve_image_view(texture.image_view)
                        .resolve_image_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
                },
            )
            .collect::<Vec<_>>();
        let depth_attachment_info = RenderingAttachmentInfo::default()
            .image_view(canvas.depth_image.view)
            .image_layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(AttachmentLoadOp::LOAD)
            .store_op(AttachmentStoreOp::STORE);
        let rendering_info = RenderingInfo::default()
            .render_area(ash::vk::Rect2D {
                offset: ash::vk::Offset2D { x: 0, y: 0 },
                extent: ash::vk::Extent2D {
                    width: canvas.width(),
                    height: canvas.height(),
                },
            })
            .layer_count(canvas.layer_count)
            .color_attachments(&color_attachment_infos)
            .depth_attachment(&depth_attachment_info);
        unsafe {
            device.cmd_begin_rendering(command_buffer, &rendering_info);
        }
        Ok(())
    }
    fn begin_depth(
        &self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        canvas: &VulkanDepthCanvas,
    ) -> Result<()> {
        let depth_attachment_info = RenderingAttachmentInfo::default()
            .image_view(canvas.texture.image_view)
            .image_layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(AttachmentLoadOp::LOAD)
            .store_op(AttachmentStoreOp::STORE);
        let rendering_info = RenderingInfo::default()
            .render_area(ash::vk::Rect2D {
                offset: ash::vk::Offset2D { x: 0, y: 0 },
                extent: ash::vk::Extent2D {
                    width: canvas.width(),
                    height: canvas.height(),
                },
            })
            .layer_count(canvas.layer_count)
            .depth_attachment(&depth_attachment_info);
        unsafe {
            device.cmd_begin_rendering(command_buffer, &rendering_info);
        }
        Ok(())
    }
    fn end(&self, device: &ash::Device, command_buffer: CommandBuffer) -> Result<()> {
        unsafe {
            device.cmd_end_rendering(command_buffer);
        }
        Ok(())
    }
    fn destroy(&mut self, _: &ash::Device) -> Result<()> {
        Ok(())
    }
}
