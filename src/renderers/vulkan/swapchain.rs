use ash::vk::{
    AccessFlags, ColorSpaceKHR, CompositeAlphaFlagsKHR, Extent3D, ImageAspectFlags, ImageCopy,
    ImageLayout, ImageSubresourceLayers, ImageUsageFlags, PipelineStageFlags, PresentModeKHR,
    SemaphoreCreateInfo, SharingMode, SwapchainCreateInfoKHR, SwapchainKHR,
};

use crate::{
    Error, Result,
    renderers::{
        Renderer, RendererCanvas,
        vulkan::{VulkanRenderer, canvas::VulkanCanvas},
    },
};

/// Represents the Vulkan swapchain and its associated resources required for presentation.
pub(super) struct SwapchainContext {
    /// The swapchain extension loader.
    pub(super) loader: ash::khr::swapchain::Device,
    /// The Vulkan swapchain instance.
    pub(super) swapchain: ash::vk::SwapchainKHR,
    /// Indicates whether the swapchain must be recreated before acquiring another image.
    pub(super) invalidated: bool,
    /// The pixel format of the surface images.
    pub(super) format: ash::vk::Format,
    /// The width and height of the swapchain images.
    pub(super) extent: ash::vk::Extent2D,
    /// The Vulkan images, which the display canvas is copied into.
    pub(super) images: Vec<ash::vk::Image>,
    /// The image available and render finished semaphores.
    pub(super) semaphores: Vec<(ash::vk::Semaphore, ash::vk::Semaphore)>,
    /// The index of the image currently used for rendering.
    pub(super) current_image_index: usize,
    /// The index of the semaphore used for acquiring an image.
    pub(super) image_avaliable_semaphore_index: usize,
}

impl SwapchainContext {
    /// Indicates an invalid index.
    pub(super) const INVALID_INDEX: usize = usize::MAX;
}

impl VulkanRenderer {
    /// Creates the swapchain, images, and semaphores for the renderer.
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
        // The display canvas is copied into the swapchain images.
        if !surface_capabilities
            .supported_usage_flags
            .contains(ImageUsageFlags::TRANSFER_DST)
        {
            return Err(Error::fail(
                "Current device does not support copying images into the target window.",
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

        let array_layer_count = self
            .settings
            .array_layer_count()
            .min(surface_capabilities.max_image_array_layers);
        // `0` means there is no limit for number of images.
        let requested_image_count = match surface_capabilities.max_image_count == 0 {
            true => frames_in_flight.max(surface_capabilities.min_image_count),
            false => frames_in_flight.clamp(
                surface_capabilities.min_image_count,
                surface_capabilities.max_image_count,
            ),
        };
        if requested_image_count < frames_in_flight {
            return Err(Error::Fail(format!(
                "Failed to create `{}` amount of frames.",
                frames_in_flight
            )));
        }

        // Create the swapchain and semaphores.
        unsafe {
            let swapchain_create_info = SwapchainCreateInfoKHR::default()
                .surface(window_surface)
                .min_image_count(requested_image_count)
                .image_format(device_context.swapchain_context.format)
                .image_color_space(color_space)
                .image_extent(device_context.swapchain_context.extent)
                .image_array_layers(array_layer_count)
                .image_usage(ImageUsageFlags::TRANSFER_DST)
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
            device_context.swapchain_context.semaphores = Vec::with_capacity(image_count);
            for _ in 0..image_count {
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

            // The image is already acquired, `begin_frame` is called again without `end_frame`.
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

    /// Destroys the swapchain, images, and semaphores.
    pub(super) fn destroy_swapchain(&mut self) -> Result<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;

        let swapchain = &mut device_context.swapchain_context;
        unsafe {
            for (image_available, render_finished) in swapchain.semaphores.drain(..) {
                device_context
                    .logical_device
                    .destroy_semaphore(image_available, None);
                device_context
                    .logical_device
                    .destroy_semaphore(render_finished, None);
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

    /// Copies the display canvas into the swapchain image, and prepares the image for presentation.
    pub(super) fn copy_display_canvas(
        device: &ash::Device,
        command_buffer: ash::vk::CommandBuffer,
        display_canvas: &VulkanCanvas,
        swapchain_image: ash::vk::Image,
        layer_count: u32,
        sampling_stages: PipelineStageFlags,
    ) {
        Self::transition_image_layout(
            device,
            command_buffer,
            display_canvas.texture.image,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ImageLayout::TRANSFER_SRC_OPTIMAL,
            AccessFlags::empty(),
            AccessFlags::TRANSFER_READ,
            sampling_stages,
            PipelineStageFlags::TRANSFER,
            ImageAspectFlags::COLOR,
            0,
            1,
            layer_count,
        );
        Self::transition_image_layout(
            device,
            command_buffer,
            swapchain_image,
            ImageLayout::UNDEFINED,
            ImageLayout::TRANSFER_DST_OPTIMAL,
            AccessFlags::empty(),
            AccessFlags::TRANSFER_WRITE,
            PipelineStageFlags::TRANSFER,
            PipelineStageFlags::TRANSFER,
            ImageAspectFlags::COLOR,
            0,
            1,
            layer_count,
        );

        let subresource = ImageSubresourceLayers::default()
            .aspect_mask(ImageAspectFlags::COLOR)
            .mip_level(0)
            .base_array_layer(0)
            .layer_count(layer_count);
        let region = ImageCopy::default()
            .src_subresource(subresource)
            .dst_subresource(subresource)
            .extent(Extent3D {
                width: display_canvas.width(),
                height: display_canvas.height(),
                depth: 1,
            });
        unsafe {
            device.cmd_copy_image(
                command_buffer,
                display_canvas.texture.image,
                ImageLayout::TRANSFER_SRC_OPTIMAL,
                swapchain_image,
                ImageLayout::TRANSFER_DST_OPTIMAL,
                std::slice::from_ref(&region),
            );
        }

        Self::transition_image_layout(
            device,
            command_buffer,
            swapchain_image,
            ImageLayout::TRANSFER_DST_OPTIMAL,
            ImageLayout::PRESENT_SRC_KHR,
            AccessFlags::TRANSFER_WRITE,
            AccessFlags::empty(),
            PipelineStageFlags::TRANSFER,
            PipelineStageFlags::BOTTOM_OF_PIPE,
            ImageAspectFlags::COLOR,
            0,
            1,
            layer_count,
        );
        // Restore the layout the canvas has while it is not rendered into.
        Self::transition_image_layout(
            device,
            command_buffer,
            display_canvas.texture.image,
            ImageLayout::TRANSFER_SRC_OPTIMAL,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            AccessFlags::empty(),
            AccessFlags::SHADER_READ,
            PipelineStageFlags::TRANSFER,
            sampling_stages,
            ImageAspectFlags::COLOR,
            0,
            1,
            layer_count,
        );
    }

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
