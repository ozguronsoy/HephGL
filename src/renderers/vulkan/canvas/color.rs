use ash::vk::{
    AccessFlags, CommandBuffer, Extent2D, Format, ImageAspectFlags, ImageLayout, ImageUsageFlags,
    PipelineStageFlags, SampleCountFlags,
};

use crate::{
    Error, Result,
    renderers::{
        RendererCanvas,
        resources::{RendererTexture, TextureFormat},
        settings::{CanvasOptions, Msaa, Settings, TextureOptions},
        vulkan::{
            VulkanRenderer,
            canvas::{VulkanCanvas, attachment_image::AttachmentImage},
            device::DeviceContext,
            rendering::VulkanRendering,
            resources::VulkanTexture,
        },
    },
};

/// Represents a Vulkan canvas that has a color image and a depth buffer.
pub struct VulkanColorCanvas {
    /// The options the canvas was created with.
    pub options: CanvasOptions,
    /// The single-sample color image that is sampled by shaders.
    pub texture: VulkanTexture,
    /// The multisampled color image, or `None` if MSAA is disabled.
    pub msaa_color_image: Option<AttachmentImage>,
    /// The depth buffer.
    pub depth_image: AttachmentImage,
    /// The render pass used for rendering into the canvas, or null if it is not required.
    pub render_pass: ash::vk::RenderPass,
    /// The framebuffer used for rendering into the canvas, or null if it is not required.
    pub framebuffer: ash::vk::Framebuffer,
    /// The MSAA sample count of the canvas.
    pub msaa: Msaa,
    /// The number of array layers of the canvas images.
    pub layer_count: u32,
}

impl RendererCanvas for VulkanColorCanvas {
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

impl VulkanCanvas for VulkanColorCanvas {
    fn render_target(&self) -> (ash::vk::Image, ImageAspectFlags) {
        (self.texture.image, ImageAspectFlags::COLOR)
    }

    fn layer_count(&self) -> u32 {
        self.layer_count
    }

    fn is_compatible(&self, settings: &Settings, swapchain_format: Format) -> bool {
        self.msaa == settings.msaa
            && self.layer_count == settings.array_layer_count()
            && Format::from(self.texture.texture_options.format) == swapchain_format
    }

    fn begin_rendering(
        &self,
        device: &ash::Device,
        rendering: &mut dyn VulkanRendering,
        command_buffer: CommandBuffer,
        sampling_stages: PipelineStageFlags,
    ) -> Result<()> {
        let color_access = AccessFlags::COLOR_ATTACHMENT_READ | AccessFlags::COLOR_ATTACHMENT_WRITE;
        match self.msaa_color_image {
            Some(msaa_color_image) => {
                VulkanRenderer::transition_image_layout(
                    device,
                    command_buffer,
                    msaa_color_image.image,
                    ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    AccessFlags::COLOR_ATTACHMENT_WRITE,
                    color_access,
                    PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    ImageAspectFlags::COLOR,
                    0,
                    1,
                    self.layer_count,
                );
                VulkanRenderer::transition_image_layout(
                    device,
                    command_buffer,
                    self.texture.image,
                    ImageLayout::UNDEFINED,
                    ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    AccessFlags::empty(),
                    AccessFlags::COLOR_ATTACHMENT_WRITE,
                    sampling_stages,
                    PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    ImageAspectFlags::COLOR,
                    0,
                    1,
                    self.layer_count,
                );
            }
            None => {
                VulkanRenderer::transition_image_layout(
                    device,
                    command_buffer,
                    self.texture.image,
                    ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    AccessFlags::empty(),
                    color_access,
                    sampling_stages,
                    PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    ImageAspectFlags::COLOR,
                    0,
                    1,
                    self.layer_count,
                );
            }
        }

        let depth_stages =
            PipelineStageFlags::EARLY_FRAGMENT_TESTS | PipelineStageFlags::LATE_FRAGMENT_TESTS;
        VulkanRenderer::transition_image_layout(
            device,
            command_buffer,
            self.depth_image.image,
            ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                | AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            depth_stages,
            depth_stages,
            ImageAspectFlags::DEPTH,
            0,
            1,
            self.layer_count,
        );
        rendering.begin(device, command_buffer, self)
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
            ImageAspectFlags::COLOR,
            self.layer_count,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            AccessFlags::SHADER_READ,
            sampling_stages,
        );
        if let Some(msaa_color_image) = self.msaa_color_image {
            VulkanRenderer::clear_attachment_image(
                device,
                command_buffer,
                msaa_color_image.image,
                ImageAspectFlags::COLOR,
                self.layer_count,
                ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                AccessFlags::COLOR_ATTACHMENT_READ | AccessFlags::COLOR_ATTACHMENT_WRITE,
                PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            );
        }
        VulkanRenderer::clear_attachment_image(
            device,
            command_buffer,
            self.depth_image.image,
            ImageAspectFlags::DEPTH,
            self.layer_count,
            ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                | AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            PipelineStageFlags::EARLY_FRAGMENT_TESTS | PipelineStageFlags::LATE_FRAGMENT_TESTS,
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
        if let Some(msaa_color_image) = self.msaa_color_image.take() {
            VulkanRenderer::destroy_attachment_image(device_context, msaa_color_image);
        }
        VulkanRenderer::destroy_attachment_image(device_context, self.depth_image);
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
        self.depth_image.image = ash::vk::Image::null();
        self.depth_image.view = ash::vk::ImageView::null();
        self.texture.image = ash::vk::Image::null();
        self.texture.image_view = ash::vk::ImageView::null();
        self.texture.texture_options = TextureOptions::default();
    }
}

impl VulkanColorCanvas {
    /// Creates the images of the canvas and the resources required for rendering into them.
    pub fn new(
        device_context: &DeviceContext,
        options: &CanvasOptions,
        msaa: Msaa,
        layer_count: u32,
    ) -> Result<Self> {
        let rendering = device_context
            .rendering
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let format = device_context.swapchain_context.format;
        let texture_format = TextureFormat::try_from(format)?;
        let extent = Extent2D {
            width: options.width,
            height: options.height,
        };
        let sample_count: SampleCountFlags = msaa.into();

        let color_image = VulkanRenderer::create_attachment_image(
            device_context,
            format,
            extent,
            layer_count,
            SampleCountFlags::TYPE_1,
            ImageUsageFlags::COLOR_ATTACHMENT
                | ImageUsageFlags::SAMPLED
                | ImageUsageFlags::TRANSFER_SRC
                | ImageUsageFlags::TRANSFER_DST,
            ImageAspectFlags::COLOR,
        )?;
        let msaa_color_image = match sample_count {
            SampleCountFlags::TYPE_1 => None,
            _ => Some(
                match VulkanRenderer::create_attachment_image(
                    device_context,
                    format,
                    extent,
                    layer_count,
                    sample_count,
                    ImageUsageFlags::COLOR_ATTACHMENT | ImageUsageFlags::TRANSFER_DST,
                    ImageAspectFlags::COLOR,
                ) {
                    Ok(msaa_color_image) => msaa_color_image,
                    Err(error) => {
                        VulkanRenderer::destroy_attachment_image(device_context, color_image);
                        return Err(error);
                    }
                },
            ),
        };
        let depth_image = match VulkanRenderer::create_attachment_image(
            device_context,
            Format::D32_SFLOAT,
            extent,
            layer_count,
            sample_count,
            ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | ImageUsageFlags::TRANSFER_DST,
            ImageAspectFlags::DEPTH,
        ) {
            Ok(depth_image) => depth_image,
            Err(error) => {
                VulkanRenderer::destroy_attachment_image(device_context, color_image);
                if let Some(msaa_color_image) = msaa_color_image {
                    VulkanRenderer::destroy_attachment_image(device_context, msaa_color_image);
                }
                return Err(error);
            }
        };

        let mut canvas = Self {
            options: *options,
            texture: VulkanTexture {
                image: color_image.image,
                image_view: color_image.view,
                vma_allocation: color_image.allocation,
                texture_options: TextureOptions {
                    width: options.width,
                    height: options.height,
                    format: texture_format,
                    mip_level_count: 1,
                },
            },
            msaa_color_image,
            depth_image,
            render_pass: ash::vk::RenderPass::null(),
            framebuffer: ash::vk::Framebuffer::null(),
            msaa,
            layer_count,
        };
        if let Err(error) =
            rendering.create_color_canvas_framebuffer(&device_context.logical_device, &mut canvas)
        {
            canvas.destroy(device_context);
            return Err(error);
        }
        Ok(canvas)
    }
}
