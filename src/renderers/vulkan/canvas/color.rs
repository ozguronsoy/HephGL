use std::mem::DropGuard;

use ash::vk::{
    AccessFlags, CommandBuffer, Extent2D, Format, Handle, ImageAspectFlags, ImageLayout,
    ImageUsageFlags, PipelineStageFlags, SampleCountFlags,
};

use crate::{
    Error, Result,
    renderers::{
        RendererCanvas,
        canvas::CanvasOptions,
        debug::RendererDebugObject,
        id::RendererId,
        resources::{RendererTexture, TextureFormat, TextureOptions},
        settings::{Msaa, Settings},
        vulkan::{
            VulkanRenderer,
            canvas::{VulkanCanvas, attachment_image::AttachmentImage},
            device::DeviceContext,
            rendering::VulkanRendering,
            resources::VulkanTexture,
        },
    },
};

/// Represents a Vulkan canvas that has color images and a depth buffer.
pub struct VulkanColorCanvas {
    /// The ID of the renderer that created the canvas.
    pub renderer_id: RendererId,
    /// The options the canvas was created with.
    pub options: CanvasOptions,
    /// The single-sample color images that are sampled by shaders.
    pub textures: Vec<VulkanTexture>,
    /// The multisampled color images, one for each texture, or empty if MSAA is disabled.
    pub msaa_color_images: Vec<AttachmentImage>,
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

impl RendererDebugObject for VulkanColorCanvas {}

impl RendererCanvas for VulkanColorCanvas {
    fn is_allocated(&self) -> bool {
        !self.depth_image.image.is_null()
    }

    fn options(&self) -> &CanvasOptions {
        &self.options
    }

    fn textures(&self) -> Box<dyn Iterator<Item = &dyn RendererTexture> + '_> {
        Box::new(
            self.textures
                .iter()
                .map(|texture| texture as &dyn RendererTexture),
        )
    }
}

impl VulkanCanvas for VulkanColorCanvas {
    fn render_target(&self) -> (&[VulkanTexture], ImageAspectFlags) {
        (&self.textures, ImageAspectFlags::COLOR)
    }

    fn layer_count(&self) -> u32 {
        self.layer_count
    }

    fn renderer_id(&self) -> RendererId {
        self.renderer_id
    }

    fn is_compatible(&self, settings: &Settings, swapchain_format: Format) -> bool {
        self.msaa == settings.msaa
            && self.layer_count == settings.array_layer_count()
            && self.options.texture_formats.iter().zip(&self.textures).all(
                |(texture_format, texture)| {
                    texture_format.is_some()
                        || Format::from(texture.texture_options.format) == swapchain_format
                },
            )
    }

    fn begin_rendering(
        &self,
        device: &ash::Device,
        rendering: &dyn VulkanRendering,
        command_buffer: CommandBuffer,
        sampling_stages: PipelineStageFlags,
    ) -> Result<()> {
        let color_access = AccessFlags::COLOR_ATTACHMENT_READ | AccessFlags::COLOR_ATTACHMENT_WRITE;
        for (index, texture) in self.textures.iter().enumerate() {
            match self.msaa_color_images.get(index) {
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
                        texture.image,
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
                        texture.image,
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
        for texture in &self.textures {
            VulkanRenderer::clear_attachment_image(
                device,
                command_buffer,
                texture.image,
                ImageAspectFlags::COLOR,
                self.layer_count,
                ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                AccessFlags::SHADER_READ,
                sampling_stages,
            );
        }
        for msaa_color_image in &self.msaa_color_images {
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
        for msaa_color_image in self.msaa_color_images.drain(..) {
            VulkanRenderer::destroy_attachment_image(device_context, msaa_color_image);
        }
        VulkanRenderer::destroy_attachment_image(device_context, self.depth_image);
        for texture in &mut self.textures {
            VulkanRenderer::destroy_attachment_image(
                device_context,
                AttachmentImage {
                    image: texture.image,
                    view: texture.image_view,
                    allocation: texture.vma_allocation,
                },
            );
            texture.image = ash::vk::Image::null();
            texture.image_view = ash::vk::ImageView::null();
            texture.texture_options = TextureOptions::default();
        }

        self.framebuffer = ash::vk::Framebuffer::null();
        self.render_pass = ash::vk::RenderPass::null();
        self.depth_image.image = ash::vk::Image::null();
        self.depth_image.view = ash::vk::ImageView::null();
    }
}

impl VulkanColorCanvas {
    /// Creates the images of the canvas and the resources required for rendering into them.
    pub fn new(
        device_context: &DeviceContext,
        options: &CanvasOptions,
        texture_formats: &[TextureFormat],
        msaa: Msaa,
        layer_count: u32,
        renderer_id: RendererId,
    ) -> Result<Self> {
        let rendering = unsafe { &*device_context.rendering.get() }
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let extent = Extent2D {
            width: options.width,
            height: options.height,
        };
        let sample_count: SampleCountFlags = msaa.into();

        let destroy_images = |images: Vec<AttachmentImage>| {
            for image in images {
                VulkanRenderer::destroy_attachment_image(device_context, image);
            }
        };
        let mut color_images =
            DropGuard::new(Vec::with_capacity(texture_formats.len()), destroy_images);
        let mut msaa_color_images = DropGuard::new(Vec::new(), destroy_images);
        for texture_format in texture_formats {
            let format = Format::from(*texture_format);
            color_images.push(VulkanRenderer::create_attachment_image(
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
            )?);
            if sample_count != SampleCountFlags::TYPE_1 {
                msaa_color_images.push(VulkanRenderer::create_attachment_image(
                    device_context,
                    format,
                    extent,
                    layer_count,
                    sample_count,
                    ImageUsageFlags::COLOR_ATTACHMENT | ImageUsageFlags::TRANSFER_DST,
                    ImageAspectFlags::COLOR,
                )?);
            }
        }
        let depth_image = VulkanRenderer::create_attachment_image(
            device_context,
            Format::D32_SFLOAT,
            extent,
            layer_count,
            sample_count,
            ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | ImageUsageFlags::TRANSFER_DST,
            ImageAspectFlags::DEPTH,
        )?;

        let mut canvas = Self {
            renderer_id,
            options: options.clone(),
            textures: DropGuard::dismiss(color_images)
                .into_iter()
                .zip(texture_formats)
                .map(|(color_image, &format)| VulkanTexture {
                    renderer_id,
                    image: color_image.image,
                    image_view: color_image.view,
                    vma_allocation: color_image.allocation,
                    texture_options: TextureOptions {
                        width: options.width,
                        height: options.height,
                        format,
                        mip_level_count: 1,
                    },
                })
                .collect::<Vec<_>>(),
            msaa_color_images: DropGuard::dismiss(msaa_color_images),
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
