use ash::vk::{
    AccessFlags, Extent2D, Extent3D, Format, ImageAspectFlags, ImageCreateInfo, ImageLayout,
    ImageSubresourceRange, ImageTiling, ImageType, ImageUsageFlags, ImageViewCreateInfo,
    ImageViewType, PipelineStageFlags, SampleCountFlags, SharingMode,
};
use vk_mem::Alloc;

use crate::{
    Error, Result,
    renderers::{
        RendererCanvas,
        resources::{RendererResource, RendererTexture},
        settings::{Msaa, Settings},
        vulkan::{
            VulkanRenderer, device::DeviceContext, frame::ThreadContext,
            rendering::VulkanRendering, resources::VulkanTexture,
        },
    },
};

/// An image that is rendered into, and a view that covers all of its array layers.
#[derive(Copy, Clone)]
pub(super) struct AttachmentImage {
    pub(super) image: ash::vk::Image,
    pub(super) view: ash::vk::ImageView,
    pub(super) allocation: vk_mem::Allocation,
}

/// Represents a Vulkan canvas.
pub(super) struct VulkanCanvas {
    /// The single-sample color image that is sampled by shaders.
    pub(super) texture: VulkanTexture,
    /// The multisampled color image, or `None` if MSAA is disabled.
    pub(super) msaa_color_image: Option<AttachmentImage>,
    /// The depth buffer.
    pub(super) depth_image: AttachmentImage,
    /// The render pass used for rendering into the canvas, or null if it is not required.
    pub(super) render_pass: ash::vk::RenderPass,
    /// The framebuffer used for rendering into the canvas, or null if it is not required.
    pub(super) framebuffer: ash::vk::Framebuffer,
    /// The MSAA sample count of the canvas.
    pub(super) msaa: Msaa,
    /// The number of array layers of the canvas images.
    pub(super) layer_count: u32,
    /// The layout of the image that keeps the color between renders.
    pub(super) layout: ImageLayout,
}

impl RendererCanvas for VulkanCanvas {
    fn width(&self) -> u32 {
        self.texture.width()
    }

    fn height(&self) -> u32 {
        self.texture.height()
    }

    fn texture(&self) -> &dyn RendererTexture {
        &self.texture
    }
}

impl VulkanRenderer {
    /// Starts rendering into the canvas, ending the rendering into the previous canvas if there is
    /// one.
    pub(super) fn begin_canvas_rendering(
        device: &ash::Device,
        rendering: &mut dyn VulkanRendering,
        swapchain_format: Format,
        settings: &Settings,
        thread_context: &mut ThreadContext,
        canvas: &mut VulkanCanvas,
    ) -> Result<()> {
        if thread_context.render_target == Some(canvas.texture.image) {
            return Ok(());
        }
        if !canvas.texture.is_allocated() {
            return Err(Error::invalid_argument("Canvas is not allocated."));
        }
        if canvas.msaa != settings.msaa
            || canvas.layer_count != settings.array_layer_count()
            || Format::from(canvas.texture.texture_options.format) != swapchain_format
        {
            return Err(Error::invalid_operation(
                "Canvas is incompatible with the current settings and must be recreated.",
            ));
        }
        if let Some(previous_image) = thread_context.render_target.take() {
            Self::end_rendering(
                device,
                rendering,
                thread_context.command_buffer,
                previous_image,
                canvas.layer_count,
            )?;
        }

        let command_buffer = thread_context.command_buffer;
        let color_access = AccessFlags::COLOR_ATTACHMENT_READ | AccessFlags::COLOR_ATTACHMENT_WRITE;
        let sampling_stages =
            PipelineStageFlags::VERTEX_SHADER | PipelineStageFlags::FRAGMENT_SHADER;
        canvas.layout = match canvas.msaa_color_image {
            Some(msaa_color_image) => {
                Self::transition_image_layout(
                    device,
                    command_buffer,
                    msaa_color_image.image,
                    canvas.layout,
                    ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    AccessFlags::COLOR_ATTACHMENT_WRITE,
                    color_access,
                    PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    ImageAspectFlags::COLOR,
                    0,
                    1,
                    canvas.layer_count,
                );
                Self::transition_image_layout(
                    device,
                    command_buffer,
                    canvas.texture.image,
                    ImageLayout::UNDEFINED,
                    ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    AccessFlags::empty(),
                    AccessFlags::COLOR_ATTACHMENT_WRITE,
                    sampling_stages,
                    PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    ImageAspectFlags::COLOR,
                    0,
                    1,
                    canvas.layer_count,
                );
                ImageLayout::COLOR_ATTACHMENT_OPTIMAL
            }
            None => {
                Self::transition_image_layout(
                    device,
                    command_buffer,
                    canvas.texture.image,
                    canvas.layout,
                    ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    AccessFlags::empty(),
                    color_access,
                    sampling_stages,
                    PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    ImageAspectFlags::COLOR,
                    0,
                    1,
                    canvas.layer_count,
                );
                ImageLayout::SHADER_READ_ONLY_OPTIMAL
            }
        };

        let depth_stages =
            PipelineStageFlags::EARLY_FRAGMENT_TESTS | PipelineStageFlags::LATE_FRAGMENT_TESTS;
        Self::transition_image_layout(
            device,
            command_buffer,
            canvas.depth_image.image,
            ImageLayout::UNDEFINED,
            ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                | AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            depth_stages,
            depth_stages,
            ImageAspectFlags::DEPTH,
            0,
            1,
            canvas.layer_count,
        );
        rendering.begin(device, command_buffer, canvas)?;

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

        thread_context.render_target = Some(canvas.texture.image);
        thread_context.recorded = true;
        Ok(())
    }

    /// Creates a 2D image and a view that covers all of its array layers.
    pub(super) fn create_attachment_image(
        device_context: &DeviceContext,
        format: Format,
        extent: Extent2D,
        layer_count: u32,
        sample_count: SampleCountFlags,
        usage: ImageUsageFlags,
        aspect_mask: ImageAspectFlags,
    ) -> Result<AttachmentImage> {
        let image_info = ImageCreateInfo::default()
            .image_type(ImageType::TYPE_2D)
            .format(format)
            .extent(Extent3D {
                width: extent.width,
                height: extent.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(layer_count)
            .samples(sample_count)
            .tiling(ImageTiling::OPTIMAL)
            .usage(usage)
            .sharing_mode(SharingMode::EXCLUSIVE)
            .initial_layout(ImageLayout::UNDEFINED);
        let allocation_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::AutoPreferDevice,
            ..Default::default()
        };
        let (image, mut allocation) = unsafe {
            device_context
                .vma_allocator
                .create_image(&image_info, &allocation_info)?
        };

        let view_info = ImageViewCreateInfo::default()
            .image(image)
            .view_type(match layer_count {
                1 => ImageViewType::TYPE_2D,
                _ => ImageViewType::TYPE_2D_ARRAY,
            })
            .format(format)
            .subresource_range(
                ImageSubresourceRange::default()
                    .aspect_mask(aspect_mask)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(layer_count),
            );
        match unsafe {
            device_context
                .logical_device
                .create_image_view(&view_info, None)
        } {
            Ok(view) => Ok(AttachmentImage {
                image,
                view,
                allocation,
            }),
            Err(error) => {
                unsafe {
                    device_context
                        .vma_allocator
                        .destroy_image(image, &mut allocation);
                }
                Err(error.into())
            }
        }
    }

    /// Frees the image and its view.
    pub(super) fn destroy_attachment_image(
        device_context: &DeviceContext,
        mut image: AttachmentImage,
    ) {
        unsafe {
            device_context
                .logical_device
                .destroy_image_view(image.view, None);
            device_context
                .vma_allocator
                .destroy_image(image.image, &mut image.allocation);
        }
    }
}
