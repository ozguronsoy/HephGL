use ash::vk::{
    AccessFlags, CommandBuffer, Extent2D, Format, ImageAspectFlags, ImageLayout, ImageUsageFlags,
    PipelineStageFlags, SampleCountFlags,
};

use crate::{
    Error, Result,
    renderers::{
        RendererCanvas,
        resources::{RendererTexture, TextureFormat},
        settings::{CanvasOptions, Settings, TextureOptions},
        vulkan::{
            VulkanRenderer,
            canvas::{VulkanCanvas, attachment_image::AttachmentImage},
            device::DeviceContext,
            rendering::VulkanRendering,
            resources::VulkanTexture,
        },
    },
};

/// Represents a Vulkan canvas that has only a depth buffer.
pub struct VulkanDepthCanvas {
    /// The options the canvas was created with.
    pub options: CanvasOptions,
    /// The depth buffer that is sampled by shaders. It is never multisampled.
    pub texture: VulkanTexture,
    /// The render pass used for rendering into the canvas, or null if it is not required.
    pub render_pass: ash::vk::RenderPass,
    /// The framebuffer used for rendering into the canvas, or null if it is not required.
    pub framebuffer: ash::vk::Framebuffer,
    /// The number of array layers of the depth buffer.
    pub layer_count: u32,
}

impl RendererCanvas for VulkanDepthCanvas {
    fn width(&self) -> u32 {
        self.texture.width()
    }

    fn height(&self) -> u32 {
        self.texture.height()
    }

    fn options(&self) -> &CanvasOptions {
        &self.options
    }

    fn texture(&self) -> &dyn RendererTexture {
        &self.texture
    }
}

impl VulkanCanvas for VulkanDepthCanvas {
    fn render_target(&self) -> (ash::vk::Image, ImageAspectFlags) {
        (self.texture.image, ImageAspectFlags::DEPTH)
    }

    fn layer_count(&self) -> u32 {
        self.layer_count
    }

    fn is_compatible(&self, settings: &Settings, _: Format) -> bool {
        self.layer_count == settings.array_layer_count()
    }

    fn begin_rendering(
        &self,
        device: &ash::Device,
        rendering: &mut dyn VulkanRendering,
        command_buffer: CommandBuffer,
        sampling_stages: PipelineStageFlags,
    ) -> Result<()> {
        VulkanRenderer::transition_image_layout(
            device,
            command_buffer,
            self.texture.image,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            AccessFlags::empty(),
            AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                | AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            sampling_stages,
            PipelineStageFlags::EARLY_FRAGMENT_TESTS | PipelineStageFlags::LATE_FRAGMENT_TESTS,
            ImageAspectFlags::DEPTH,
            0,
            1,
            self.layer_count,
        );
        rendering.begin_depth(device, command_buffer, self)
    }

    fn clear_new(
        &self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        sampling_stages: PipelineStageFlags,
    ) {
        VulkanRenderer::clear_attachment_image(
            device,
            command_buffer,
            self.texture.image,
            ImageAspectFlags::DEPTH,
            self.layer_count,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            AccessFlags::SHADER_READ,
            sampling_stages,
        );
    }

    fn destroy(&mut self, device_context: &DeviceContext) {
        unsafe {
            device_context
                .logical_device
                .destroy_framebuffer(self.framebuffer, None);
            device_context
                .logical_device
                .destroy_render_pass(self.render_pass, None);
        }
        VulkanRenderer::destroy_attachment_image(
            device_context,
            AttachmentImage {
                image: self.texture.image,
                view: self.texture.image_view,
                allocation: self.texture.vma_allocation,
            },
        );

        self.framebuffer = ash::vk::Framebuffer::null();
        self.render_pass = ash::vk::RenderPass::null();
        self.texture.image = ash::vk::Image::null();
        self.texture.image_view = ash::vk::ImageView::null();
        self.texture.texture_options = TextureOptions::default();
    }
}

impl VulkanDepthCanvas {
    /// Creates the depth buffer of the canvas and the resources required for rendering into it.
    pub fn new(
        device_context: &DeviceContext,
        options: &CanvasOptions,
        layer_count: u32,
    ) -> Result<Self> {
        let rendering = device_context
            .rendering
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let depth_image = VulkanRenderer::create_attachment_image(
            device_context,
            Format::D32_SFLOAT,
            Extent2D {
                width: options.width,
                height: options.height,
            },
            layer_count,
            SampleCountFlags::TYPE_1,
            ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                | ImageUsageFlags::SAMPLED
                | ImageUsageFlags::TRANSFER_DST,
            ImageAspectFlags::DEPTH,
        )?;

        let mut canvas = Self {
            options: *options,
            texture: VulkanTexture {
                image: depth_image.image,
                image_view: depth_image.view,
                vma_allocation: depth_image.allocation,
                texture_options: TextureOptions {
                    width: options.width,
                    height: options.height,
                    format: TextureFormat::D32Sfloat,
                    mip_level_count: 1,
                },
            },
            render_pass: ash::vk::RenderPass::null(),
            framebuffer: ash::vk::Framebuffer::null(),
            layer_count,
        };
        if let Err(error) =
            rendering.create_depth_canvas_framebuffer(&device_context.logical_device, &mut canvas)
        {
            canvas.destroy(device_context);
            return Err(error);
        }
        Ok(canvas)
    }
}
