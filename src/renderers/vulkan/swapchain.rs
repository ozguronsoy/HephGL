use ash::vk::{
    ColorSpaceKHR, CompositeAlphaFlagsKHR, Extent3D, Handle, ImageCreateInfo, ImageLayout,
    ImageSubresourceRange, ImageTiling, ImageUsageFlags, ImageViewCreateInfo, PresentModeKHR,
    SampleCountFlags, SemaphoreCreateInfo, SharingMode, SwapchainCreateInfoKHR, SwapchainKHR,
};
use vk_mem::Alloc;

use crate::renderers::{RendererResult, error::RendererError, vulkan::VulkanRenderer};

/// Represents the Vulkan swapchain and its associated resources required for presentation.
pub struct SwapchainContext {
    /// The swapchain extension loader.
    pub loader: ash::khr::swapchain::Device,
    /// The Vulkan swapchain instance.
    pub swapchain: ash::vk::SwapchainKHR,
    /// The pixel format of the surface images.
    pub format: ash::vk::Format,
    /// The width and height of the swapchain images.
    pub extent: ash::vk::Extent2D,
    /// The Vulkan images.
    pub images: Vec<ash::vk::Image>,
    /// The views required to draw into the Vulkan images as color attachments.
    pub image_views: Vec<ash::vk::ImageView>,
    /// The semaphore used for GPU-GPU synchronization.
    pub semaphores: Vec<ash::vk::Semaphore>,
    /// The depth buffer.
    pub depth_image: ash::vk::Image,
    /// The allocation of depth buffer.
    pub depth_image_allocation: Option<vk_mem::Allocation>,
    /// The depth buffer view
    pub depth_image_view: ash::vk::ImageView,
}

impl VulkanRenderer {
    /// Creates the swapchain, images, image views, semaphores, and the depth buffer for the
    /// renderer.
    pub(super) fn create_swapchain(&mut self) -> RendererResult<()> {
        self.destroy_swapchain()?;

        let window_surface_loader =
            self.window_surface_loader
                .as_ref()
                .ok_or(RendererError::invalid_operation(
                    "Renderer is not initialized.",
                ))?;
        let window_surface = self.window_surface.ok_or(RendererError::invalid_operation(
            "Renderer is not initialized.",
        ))?;
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        let surface_capabilities = unsafe {
            window_surface_loader.get_physical_device_surface_capabilities(
                device_context.physical_device,
                window_surface,
            )?
        };
        let present_modes = unsafe {
            window_surface_loader.get_physical_device_surface_present_modes(
                device_context.physical_device,
                window_surface,
            )?
        };
        if !present_modes.contains(&PresentModeKHR::FIFO)
            && !present_modes.contains(&PresentModeKHR::IMMEDIATE)
        {
            return Err(RendererError::fail(
                "Current device does not support rendering to the target window.",
            ));
        }

        // TODO: Add an enum for enabling HDR.
        device_context.swapchain_context.format = ash::vk::Format::B8G8R8A8_SRGB;
        let color_space = ColorSpaceKHR::SRGB_NONLINEAR;

        device_context.swapchain_context.extent =
            if surface_capabilities.current_extent.width == u32::MAX {
                ash::vk::Extent2D {
                    width: self.settings.default_size.0.clamp(
                        surface_capabilities.min_image_extent.width,
                        surface_capabilities.max_image_extent.width,
                    ),
                    height: self.settings.default_size.1.clamp(
                        surface_capabilities.min_image_extent.height,
                        surface_capabilities.max_image_extent.height,
                    ),
                }
            } else {
                surface_capabilities.current_extent
            };

        // `0` means there is no limit for number of images.
        let requested_image_count = match surface_capabilities.max_image_count == 0 {
            true => self
                .settings
                .frames_in_flight
                .max(surface_capabilities.min_image_count),
            false => self.settings.frames_in_flight.clamp(
                surface_capabilities.min_image_count,
                surface_capabilities.max_image_count,
            ),
        };
        let (array_layer_count, image_view_type) = match self.settings.stereoscopic_3d_rendering {
            true => (
                2.min(surface_capabilities.max_image_array_layers),
                ash::vk::ImageViewType::TYPE_2D_ARRAY,
            ),
            false => (1, ash::vk::ImageViewType::TYPE_2D),
        };

        // Create the swapchain, image views, and semaphores.
        unsafe {
            let swapchain_create_info = SwapchainCreateInfoKHR::default()
                .surface(window_surface)
                .min_image_count(requested_image_count)
                .image_format(device_context.swapchain_context.format)
                .image_color_space(color_space)
                .image_extent(device_context.swapchain_context.extent)
                .image_array_layers(array_layer_count)
                .image_usage(ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(SharingMode::EXCLUSIVE)
                .pre_transform(surface_capabilities.current_transform)
                .composite_alpha(CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(
                    if self.settings.vsync && present_modes.contains(&PresentModeKHR::IMMEDIATE) {
                        PresentModeKHR::IMMEDIATE
                    } else {
                        PresentModeKHR::FIFO
                    },
                )
                .clipped(true);
            device_context.swapchain_context.swapchain = device_context
                .swapchain_context
                .loader
                .create_swapchain(&swapchain_create_info, None)?;

            device_context.swapchain_context.images = device_context
                .swapchain_context
                .loader
                .get_swapchain_images(device_context.swapchain_context.swapchain)?;

            let image_count = device_context.swapchain_context.images.len();
            device_context.swapchain_context.image_views = Vec::with_capacity(image_count);
            device_context.swapchain_context.semaphores = Vec::with_capacity(image_count);
            for image in &device_context.swapchain_context.images {
                let image_view_create_info = ImageViewCreateInfo::default()
                    .image(*image)
                    .view_type(image_view_type)
                    .format(device_context.swapchain_context.format)
                    .components(ash::vk::ComponentMapping {
                        r: ash::vk::ComponentSwizzle::R,
                        g: ash::vk::ComponentSwizzle::G,
                        b: ash::vk::ComponentSwizzle::B,
                        a: ash::vk::ComponentSwizzle::A,
                    })
                    .subresource_range(
                        ash::vk::ImageSubresourceRange::default()
                            .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
                            .base_mip_level(0)
                            .level_count(1)
                            .base_array_layer(0)
                            .layer_count(array_layer_count),
                    );
                device_context.swapchain_context.image_views.push(
                    device_context
                        .logical_device
                        .create_image_view(&image_view_create_info, None)?,
                );

                let semaphore_create_info = SemaphoreCreateInfo::default();
                device_context.swapchain_context.semaphores.push(
                    device_context
                        .logical_device
                        .create_semaphore(&semaphore_create_info, None)?,
                );
            }
        }

        // Create depth buffer.
        unsafe {
            let depth_format = ash::vk::Format::D32_SFLOAT;
            let depth_image_create_info = ImageCreateInfo::default()
                .image_type(ash::vk::ImageType::TYPE_2D)
                .format(depth_format)
                .extent(Extent3D {
                    width: device_context.swapchain_context.extent.width,
                    height: device_context.swapchain_context.extent.height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(array_layer_count)
                .samples(SampleCountFlags::TYPE_1)
                .tiling(ImageTiling::OPTIMAL)
                .usage(ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                .initial_layout(ImageLayout::UNDEFINED);
            let allocation_info = vk_mem::AllocationCreateInfo {
                usage: vk_mem::MemoryUsage::AutoPreferDevice,
                ..Default::default()
            };
            let (depth_image, dept_image_allocation) = device_context
                .vma_allocator
                .create_image(&depth_image_create_info, &allocation_info)?;
            device_context.swapchain_context.depth_image = depth_image;
            device_context.swapchain_context.depth_image_allocation = Some(dept_image_allocation);

            let depth_image_view_create_info = ImageViewCreateInfo::default()
                .image(device_context.swapchain_context.depth_image)
                .view_type(image_view_type)
                .format(depth_format)
                .subresource_range(ImageSubresourceRange {
                    aspect_mask: ash::vk::ImageAspectFlags::DEPTH,
                    level_count: 1,
                    base_mip_level: 0,
                    base_array_layer: 0,
                    layer_count: array_layer_count,
                });
            device_context.swapchain_context.depth_image_view = device_context
                .logical_device
                .create_image_view(&depth_image_view_create_info, None)?;
        }

        Ok(())
    }

    /// Destroys the swapchain, images, image views, semaphores, and the depth buffer.
    pub(super) fn destroy_swapchain(&mut self) -> RendererResult<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        if device_context.swapchain_context.depth_image_view.is_null() {
            // There is no swapchain to destroy.
            return Ok(());
        }

        unsafe {
            device_context
                .logical_device
                .destroy_image_view(device_context.swapchain_context.depth_image_view, None);
            device_context.swapchain_context.depth_image_view = ash::vk::ImageView::null();

            device_context.vma_allocator.destroy_image(
                device_context.swapchain_context.depth_image,
                device_context
                    .swapchain_context
                    .depth_image_allocation
                    .as_mut()
                    .ok_or(RendererError::invalid_operation(
                        "Failed to destroy the swapchain.",
                    ))?,
            );
            device_context.swapchain_context.depth_image = ash::vk::Image::null();
            device_context.swapchain_context.depth_image_allocation = None;

            for i in 0..device_context.swapchain_context.images.len() {
                device_context
                    .logical_device
                    .destroy_semaphore(device_context.swapchain_context.semaphores[i], None);
                device_context
                    .logical_device
                    .destroy_image_view(device_context.swapchain_context.image_views[i], None);
            }
            device_context.swapchain_context.semaphores.clear();
            device_context.swapchain_context.image_views.clear();

            // Destroying the swapchain also destroys the images.
            device_context
                .swapchain_context
                .loader
                .destroy_swapchain(device_context.swapchain_context.swapchain, None);
            device_context.swapchain_context.images.clear();
            device_context.swapchain_context.swapchain = SwapchainKHR::null();
        }

        Ok(())
    }
}
