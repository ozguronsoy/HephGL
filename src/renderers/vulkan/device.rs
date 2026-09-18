use std::sync::Mutex;

use ash::vk::{PhysicalDeviceProperties2, QueueFamilyProperties2};

use crate::{
    graphics_device::GraphicsDevice,
    renderers::{
        RendererResult,
        error::RendererError,
        thread_context::ThreadContextMaskArray,
        vulkan::{
            VulkanRenderer,
            pipeline::VulkanComputePipeline,
            queue::{QueueContext, QueueFamily},
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

    pub physical_device: ash::vk::PhysicalDevice,
    pub logical_device: ash::Device,
    pub supports_timeline_semaphore: bool,

    /// A list of compute pipelines.
    ///
    /// ### Important
    /// New elements can only be **appended**.
    #[allow(clippy::vec_box)]
    pub compute_pipelines: Mutex<Vec<Box<VulkanComputePipeline>>>,

    /// The bitmasks indicating the availability of thread contexts.
    /// `0` means the context at that index is available, `1` means it is
    /// currently in use.
    pub thread_context_masks: Mutex<ThreadContextMaskArray>,
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
            let mut properties2 = PhysicalDeviceProperties2::default();
            let mut queue_family_properties2_vec = Vec::<QueueFamilyProperties2>::default();
            unsafe {
                instance.get_physical_device_properties2(physical_device, &mut properties2);
                if device.device_id != properties2.properties.device_id {
                    continue;
                }

                let queue_family_properties2_vec_size =
                    instance.get_physical_device_queue_family_properties2_len(physical_device);
                queue_family_properties2_vec.resize(
                    queue_family_properties2_vec_size,
                    QueueFamilyProperties2::default(),
                );
                instance.get_physical_device_queue_family_properties2(
                    physical_device,
                    &mut queue_family_properties2_vec,
                );
            };

            for (index, queue_family_properties2) in queue_family_properties2_vec.iter().enumerate()
            {
                queue_families.push(QueueFamily {
                    index: index as u32,
                    queue_count: queue_family_properties2.queue_family_properties.queue_count,
                    queue_flags: queue_family_properties2.queue_family_properties.queue_flags,
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
            self.destroy_swapchain()?;
            self.uninitialize_thread()?;
            self.destroy_frame_sync()?;
        }

        if let Some(device_context) = self.device_context.take() {
            unsafe {
                drop(device_context.vma_allocator);
                device_context.logical_device.destroy_device(None);
            }
        }

        Ok(())
    }
}
