use crate::{
    graphics_device::GraphicsDevice,
    renderers::{
        Renderer, RendererResult,
        error::RendererError,
        settings::Msaa,
        thread_context::ThreadContextMaskArray,
        vulkan::{
            VulkanRenderer,
            queue::{QueueContext, QueueFamily},
            rendering::VulkanRendering,
            swapchain::SwapchainContext,
        },
    },
};

/// Encapsulates the Vulkan device state.
///
/// ### Note
/// Order of the fields matter as it determines the destruction order.
pub struct DeviceContext {
    /// The currently active graphics device.
    pub graphics_device: GraphicsDevice,

    pub vma_allocator: vk_mem::Allocator,

    pub graphics_queue_context: QueueContext,
    pub transfer_queue_context: Option<QueueContext>,
    pub compute_queue_context: Option<QueueContext>,

    pub swapchain_context: SwapchainContext,
    pub rendering: Option<Box<dyn VulkanRendering>>,

    pub physical_device: ash::vk::PhysicalDevice,
    pub logical_device: ash::Device,
    pub supports_timeline_semaphore: bool,
    pub supports_dynamic_rendering: bool,
    pub supported_msaa_list: Vec<Msaa>,

    /// TODO: Remove this when the custom samplers are enabled.
    pub default_sampler: ash::vk::Sampler,

    /// The bitmasks indicating the availability of thread contexts.
    /// `0` means the context at that index is available, `1` means it is
    /// currently in use.
    pub thread_context_masks: std::sync::Mutex<ThreadContextMaskArray>,
}

impl VulkanRenderer {
    /// Populates the internal queue family info for each available device.
    pub(super) fn get_device_queue_families(
        &self,
        device: &GraphicsDevice,
    ) -> RendererResult<Vec<QueueFamily>> {
        let instance = self
            .instance
            .as_ref()
            .ok_or(RendererError::invalid_operation(
                "Renderer is not initialized",
            ))?;
        let window_surface_loader =
            self.window_surface_loader
                .as_ref()
                .ok_or(RendererError::invalid_operation(
                    "Renderer is not initialize.",
                ))?;
        let window_surface =
            self.window_surface
                .as_ref()
                .ok_or(RendererError::invalid_operation(
                    "Renderer is not initialize.",
                ))?;

        let physical_devices = unsafe { instance.enumerate_physical_devices()? };

        let mut queue_families = Vec::default();
        for physical_device in physical_devices {
            let properties;
            let queue_family_properties_vec;
            unsafe {
                properties = instance.get_physical_device_properties(physical_device);
                if device.device_id != properties.device_id {
                    continue;
                }
                queue_family_properties_vec =
                    instance.get_physical_device_queue_family_properties(physical_device);
            };

            for (index, queue_family_properties) in queue_family_properties_vec.iter().enumerate() {
                queue_families.push(QueueFamily {
                    index: index as u32,
                    queue_count: queue_family_properties.queue_count,
                    queue_flags: queue_family_properties.queue_flags,
                    present_supported: unsafe {
                        window_surface_loader.get_physical_device_surface_support(
                            physical_device,
                            index as u32,
                            *window_surface,
                        )?
                    },
                });
            }
        }

        if queue_families.is_empty() {
            Err(RendererError::fail("Requested device is not available."))
        } else {
            Ok(queue_families)
        }
    }

    /// Frees the resources used by the graphics device, and sets the currently
    /// active device to `None`.
    pub(super) fn uninitialize_device(&mut self) -> RendererResult<()> {
        if self.device_context.is_some() {
            self.wait_idle()?;
            self.destroy_rendering()?;
            self.destroy_swapchain()?;
            self.destroy_frame_sync()?;
            self.uninitialize_thread()?;
        }

        if let Some(device_context) = self.device_context.take() {
            unsafe {
                // TODO: Remove this when the custom samplers are enabled.
                device_context
                    .logical_device
                    .destroy_sampler(device_context.default_sampler, None);
                drop(device_context.vma_allocator);
                device_context.logical_device.destroy_device(None);
            }
        }

        Ok(())
    }
}
