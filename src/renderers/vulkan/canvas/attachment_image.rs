use ash::vk::{
    AccessFlags, ClearColorValue, ClearDepthStencilValue, CommandBuffer, Extent2D, Extent3D,
    Format, ImageAspectFlags, ImageCreateInfo, ImageLayout, ImageSubresourceRange, ImageTiling,
    ImageType, ImageUsageFlags, ImageViewCreateInfo, ImageViewType, PipelineStageFlags,
    SampleCountFlags, SharingMode,
};
use vk_mem::Alloc;

use crate::{
    Result,
    renderers::vulkan::{VulkanRenderer, device::DeviceContext},
};

/// An image that is rendered into, and a view that covers all of its array layers.
#[derive(Copy, Clone)]
pub struct AttachmentImage {
    pub image: ash::vk::Image,
    pub view: ash::vk::ImageView,
    pub allocation: vk_mem::Allocation,
}

impl VulkanRenderer {
    /// Clears a newly created image to black, or to the farthest depth if it is a depth image, and
    /// transitions it to the provided layout.
    pub(super) fn clear_attachment_image(
        device: &ash::Device,
        command_buffer: CommandBuffer,
        image: ash::vk::Image,
        aspect_mask: ImageAspectFlags,
        layer_count: u32,
        layout: ImageLayout,
        access_mask: AccessFlags,
        stage_mask: PipelineStageFlags,
    ) {
        Self::transition_image_layout(
            device,
            command_buffer,
            image,
            ImageLayout::UNDEFINED,
            ImageLayout::TRANSFER_DST_OPTIMAL,
            AccessFlags::empty(),
            AccessFlags::TRANSFER_WRITE,
            PipelineStageFlags::TOP_OF_PIPE,
            PipelineStageFlags::TRANSFER,
            aspect_mask,
            0,
            1,
            layer_count,
        );
        let range = ImageSubresourceRange::default()
            .aspect_mask(aspect_mask)
            .base_mip_level(0)
            .level_count(1)
            .base_array_layer(0)
            .layer_count(layer_count);
        unsafe {
            if aspect_mask == ImageAspectFlags::DEPTH {
                device.cmd_clear_depth_stencil_image(
                    command_buffer,
                    image,
                    ImageLayout::TRANSFER_DST_OPTIMAL,
                    &ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                    std::slice::from_ref(&range),
                );
            } else {
                device.cmd_clear_color_image(
                    command_buffer,
                    image,
                    ImageLayout::TRANSFER_DST_OPTIMAL,
                    &ClearColorValue {
                        float32: [0.0, 0.0, 0.0, 1.0],
                    },
                    std::slice::from_ref(&range),
                );
            }
        }
        Self::transition_image_layout(
            device,
            command_buffer,
            image,
            ImageLayout::TRANSFER_DST_OPTIMAL,
            layout,
            AccessFlags::TRANSFER_WRITE,
            access_mask,
            PipelineStageFlags::TRANSFER,
            stage_mask,
            aspect_mask,
            0,
            1,
            layer_count,
        );
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
