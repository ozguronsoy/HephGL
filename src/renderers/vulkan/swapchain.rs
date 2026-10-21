use ash::vk::{
    ColorSpaceKHR, CompositeAlphaFlagsKHR, Extent3D, ImageCreateInfo, ImageLayout,
    ImageSubresourceRange, ImageTiling, ImageUsageFlags, ImageViewCreateInfo, PresentModeKHR,
    SampleCountFlags, SemaphoreCreateInfo, SharingMode, SwapchainCreateInfoKHR, SwapchainKHR,
};
use vk_mem::Alloc;

use crate::{
    Error, Result,
    renderers::{Renderer, vulkan::VulkanRenderer},
};

/// Represents the Vulkan swapchain and its associated resources required for presentation.
pub struct SwapchainContext {
    /// The swapchain extension loader.
    pub loader: ash::khr::swapchain::Device,
    /// The Vulkan swapchain instance.
    pub swapchain: ash::vk::SwapchainKHR,
    /// Indicates whether the swapchain must be recreated before acquiring another image.
    pub invalidated: bool,
    /// The pixel format of the surface images.
    pub format: ash::vk::Format,
    /// The width and height of the swapchain images.
    pub extent: ash::vk::Extent2D,
    /// The Vulkan images.
    pub images: Vec<ash::vk::Image>,
    /// The views required to draw into the Vulkan images as color attachments.
    pub image_views: Vec<ash::vk::ImageView>,
    /// The image available and render finished semaphores.
    pub semaphores: Vec<(ash::vk::Semaphore, ash::vk::Semaphore)>,
    /// The depth buffer.
    pub depth_image: ash::vk::Image,
    /// The allocation of depth buffer.
    pub depth_image_allocation: Option<vk_mem::Allocation>,
    /// The depth buffer view.
    pub depth_image_view: ash::vk::ImageView,
    /// The color image that will be used if MSAA is enabled.
    pub msaa_color_image: ash::vk::Image,
    /// The allocation of color image that will be used if MSAA is enabled.
    pub msaa_color_image_allocation: Option<vk_mem::Allocation>,
    /// The color image view that will be used if MSAA is enabled.
    pub msaa_color_image_view: ash::vk::ImageView,
    /// The index of the image currently used for rendering.
    pub current_image_index: usize,
    /// The index of the semaphore used for acquiring an image.
    pub image_avaliable_semaphore_index: usize,
}

impl SwapchainContext {
    /// Indicates an invalid index.
    pub const INVALID_INDEX: usize = usize::MAX;
}

impl VulkanRenderer {
    /// Creates the swapchain, images, image views, semaphores, and the depth buffer for the
    /// renderer.
    pub(super) fn create_swapchain(&mut self) -> Result<()> {
        let window_surface_loader = self
            .window_surface_loader
            .as_ref()
            .ok_or(Error::invalid_operation("Renderer is not initialized."))?;
        let window_surface = self
            .window_surface
            .ok_or(Error::invalid_operation("Renderer is not initialized."))?;
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let frames_in_flight = self.settings.frames_in_flight as u32;

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
            return Err(Error::fail(
                "Current device does not support rendering to the target window.",
            ));
        }

        // TODO: Add an enum for enabling HDR.
        let format = ash::vk::Format::B8G8R8A8_SRGB;
        let color_space = ColorSpaceKHR::SRGB_NONLINEAR;

        let extent = match surface_capabilities.current_extent.width == u32::MAX {
            true => ash::vk::Extent2D {
                width: self.settings.default_size.0.clamp(
                    surface_capabilities.min_image_extent.width,
                    surface_capabilities.max_image_extent.width,
                ),
                height: self.settings.default_size.1.clamp(
                    surface_capabilities.min_image_extent.height,
                    surface_capabilities.max_image_extent.height,
                ),
            },
            false => surface_capabilities.current_extent,
        };
        if extent.width == 0 || extent.height == 0 {
            return Err(Error::invalid_operation(
                "Cannot render to a zero-sized window surface. Retry after the window is restored.",
            ));
        }

        self.destroy_swapchain()?;
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        device_context.swapchain_context.format = format;
        device_context.swapchain_context.extent = extent;

        // `0` means there is no limit for number of images.
        let requested_image_count = match surface_capabilities.max_image_count == 0 {
            true => frames_in_flight.max(surface_capabilities.min_image_count),
            false => frames_in_flight.clamp(
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

        if requested_image_count < frames_in_flight {
            return Err(Error::Fail(format!(
                "Failed to create `{}` amount of frames.",
                frames_in_flight
            )));
        }

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
                    if !self.settings.vsync && present_modes.contains(&PresentModeKHR::IMMEDIATE) {
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
                device_context.swapchain_context.semaphores.push((
                    device_context
                        .logical_device
                        .create_semaphore(&semaphore_create_info, None)?,
                    device_context
                        .logical_device
                        .create_semaphore(&semaphore_create_info, None)?,
                ));
            }
        }

        if !device_context
            .supported_msaa_list
            .contains(&self.settings.msaa)
        {
            return Err(Error::invalid_argument(
                "Requested MSAA sample count is not supported by the selected device.",
            ));
        }
        let sample_count: SampleCountFlags = self.settings.msaa.into();

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
                .samples(sample_count)
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

        // Create MSAA image.
        if sample_count != SampleCountFlags::TYPE_1 {
            let color_image_create_info = ImageCreateInfo::default()
                .image_type(ash::vk::ImageType::TYPE_2D)
                .format(device_context.swapchain_context.format)
                .extent(Extent3D {
                    width: device_context.swapchain_context.extent.width,
                    height: device_context.swapchain_context.extent.height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(array_layer_count)
                .samples(sample_count)
                .tiling(ImageTiling::OPTIMAL)
                .usage(ImageUsageFlags::COLOR_ATTACHMENT | ImageUsageFlags::TRANSIENT_ATTACHMENT)
                .initial_layout(ImageLayout::UNDEFINED);
            let allocation_info = vk_mem::AllocationCreateInfo {
                usage: vk_mem::MemoryUsage::AutoPreferDevice,
                ..Default::default()
            };
            let (image, allocation) = unsafe {
                device_context
                    .vma_allocator
                    .create_image(&color_image_create_info, &allocation_info)?
            };
            device_context.swapchain_context.msaa_color_image = image;
            device_context.swapchain_context.msaa_color_image_allocation = Some(allocation);

            let image_view_create_info = ImageViewCreateInfo::default()
                .image(image)
                .view_type(image_view_type)
                .format(device_context.swapchain_context.format)
                .subresource_range(
                    ImageSubresourceRange::default()
                        .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
                        .base_mip_level(0)
                        .level_count(1)
                        .base_array_layer(0)
                        .layer_count(array_layer_count),
                );
            device_context.swapchain_context.msaa_color_image_view = unsafe {
                device_context
                    .logical_device
                    .create_image_view(&image_view_create_info, None)?
            };
        }

        device_context
            .swapchain_context
            .image_avaliable_semaphore_index = SwapchainContext::INVALID_INDEX;

        Ok(())
    }

    /// Acquires a swapchain image, recreating invalidated presentation resources before recording
    /// begins.
    pub(super) fn acquire_swapchain_image(p_self: *mut Self) -> Result<()> {
        macro_rules! this {
            () => {
                unsafe { &mut *p_self }
            };
        }

        // Retry once so repeated surface changes return control to the application's event loop.
        for attempt in 0..2 {
            let device_context = this!()
                .device_context
                .as_mut()
                .ok_or(Error::invalid_operation("Device is not set."))?;
            let swapchain_context = &mut device_context.swapchain_context;

            if swapchain_context.image_avaliable_semaphore_index != SwapchainContext::INVALID_INDEX
            {
                return Ok(());
            }

            if swapchain_context.invalidated {
                this!().wait_idle()?;
                this!().destroy_rendering()?;
                this!().create_swapchain()?;
                this!().create_rendering()?;
                swapchain_context.invalidated = false;
            }

            let result = unsafe {
                swapchain_context.loader.acquire_next_image(
                    swapchain_context.swapchain,
                    1.0e9 as u64,
                    swapchain_context.semaphores[device_context.current_frame_index].0,
                    ash::vk::Fence::null(),
                )
            };
            match result {
                Ok((image_index, suboptimal)) => {
                    swapchain_context.current_image_index = image_index as usize;
                    swapchain_context.image_avaliable_semaphore_index =
                        device_context.current_frame_index;
                    swapchain_context.invalidated = suboptimal;
                    return Ok(());
                }
                Err(ash::vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    swapchain_context.invalidated = true;
                    if attempt == 1 {
                        return Err(ash::vk::Result::ERROR_OUT_OF_DATE_KHR.into());
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }

        Ok(())
    }

    /// Destroys the swapchain, images, image views, semaphores, and the depth buffer.
    pub(super) fn destroy_swapchain(&mut self) -> Result<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;

        let swapchain = &mut device_context.swapchain_context;
        unsafe {
            device_context
                .logical_device
                .destroy_image_view(swapchain.depth_image_view, None);
            swapchain.depth_image_view = ash::vk::ImageView::null();
            if let Some(mut allocation) = swapchain.depth_image_allocation.take() {
                device_context
                    .vma_allocator
                    .destroy_image(swapchain.depth_image, &mut allocation);
            }
            swapchain.depth_image = ash::vk::Image::null();

            device_context
                .logical_device
                .destroy_image_view(swapchain.msaa_color_image_view, None);
            swapchain.msaa_color_image_view = ash::vk::ImageView::null();
            if let Some(mut allocation) = swapchain.msaa_color_image_allocation.take() {
                device_context
                    .vma_allocator
                    .destroy_image(swapchain.msaa_color_image, &mut allocation);
            }
            swapchain.msaa_color_image = ash::vk::Image::null();

            for (image_available, render_finished) in swapchain.semaphores.drain(..) {
                device_context
                    .logical_device
                    .destroy_semaphore(image_available, None);
                device_context
                    .logical_device
                    .destroy_semaphore(render_finished, None);
            }

            for view in swapchain.image_views.drain(..) {
                device_context.logical_device.destroy_image_view(view, None);
            }
            swapchain
                .loader
                .destroy_swapchain(swapchain.swapchain, None);
            swapchain.swapchain = SwapchainKHR::null();
            swapchain.images.clear();
        }

        swapchain.current_image_index = 0;
        swapchain.image_avaliable_semaphore_index = SwapchainContext::INVALID_INDEX;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn transition_image_layout(
        device: &ash::Device,
        command_buffer: ash::vk::CommandBuffer,
        image: ash::vk::Image,
        old_layout: ash::vk::ImageLayout,
        new_layout: ash::vk::ImageLayout,
        src_access_mask: ash::vk::AccessFlags,
        dst_access_mask: ash::vk::AccessFlags,
        src_stage_mask: ash::vk::PipelineStageFlags,
        dst_stage_mask: ash::vk::PipelineStageFlags,
        aspect_mask: ash::vk::ImageAspectFlags,
        base_mip_level: u32,
        mip_level_count: u32,
        layer_count: u32,
    ) {
        let barrier = ash::vk::ImageMemoryBarrier::default()
            .src_access_mask(src_access_mask)
            .dst_access_mask(dst_access_mask)
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(ash::vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(ash::vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(
                ash::vk::ImageSubresourceRange::default()
                    .aspect_mask(aspect_mask)
                    .base_mip_level(base_mip_level)
                    .level_count(mip_level_count)
                    .base_array_layer(0)
                    .layer_count(layer_count),
            );

        unsafe {
            device.cmd_pipeline_barrier(
                command_buffer,
                src_stage_mask,
                dst_stage_mask,
                ash::vk::DependencyFlags::empty(),
                &[],
                &[],
                std::slice::from_ref(&barrier),
            );
        }
    }
}
