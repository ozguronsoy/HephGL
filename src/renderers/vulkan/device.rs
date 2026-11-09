use std::{sync::Mutex, time::Instant};

use crate::{
    Error, Result,
    graphics_device::{GraphicsDevice, GraphicsDeviceFeature},
    renderers::{
        Renderer,
        settings::Msaa,
        thread_context::ThreadContextMaskArray,
        vulkan::{
            VulkanRenderer,
            canvas::VulkanCanvas,
            frame::{ComputeFrame, GraphicsFrame, TransferFrame},
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
pub(super) struct DeviceContext {
    /// The currently active graphics device.
    pub(super) graphics_device: GraphicsDevice,
    /// The features enabled for the graphics device.
    pub(super) enabled_features: Vec<GraphicsDeviceFeature>,

    pub(super) vma_allocator: vk_mem::Allocator,

    pub(super) graphics_queue_context: QueueContext<GraphicsFrame>,
    pub(super) transfer_queue_context: QueueContext<TransferFrame>,
    pub(super) compute_queue_context: Option<QueueContext<ComputeFrame>>,

    pub(super) swapchain_context: SwapchainContext,
    pub(super) rendering: Option<Box<dyn VulkanRendering>>,
    pub(super) display_canvas: Option<VulkanCanvas>,

    pub(super) physical_device: ash::vk::PhysicalDevice,
    pub(super) logical_device: ash::Device,
    pub(super) supports_timeline_semaphore: bool,
    pub(super) supports_dynamic_rendering: bool,
    pub(super) supported_msaa_list: Vec<Msaa>,
    pub(super) sampling_stages: ash::vk::PipelineStageFlags,

    pub(super) current_frame_index: usize,
    pub(super) current_frame_index_sync: Mutex<()>,
    pub(super) next_frame_deadline: Option<Instant>,

    /// The bitmasks indicating the availability of thread contexts.
    /// `0` means the context at that index is available, `1` means it is
    /// currently in use.
    pub(super) thread_context_masks: Mutex<ThreadContextMaskArray>,
}

impl VulkanRenderer {
    /// Populates the internal queue family info for each available device.
    pub(super) fn get_device_queue_families(
        &self,
        device: &GraphicsDevice,
    ) -> Result<Vec<QueueFamily>> {
        let instance = self
            .instance
            .as_ref()
            .ok_or(Error::invalid_operation("Renderer is not initialized"))?;
        let window_surface_loader = self
            .window_surface_loader
            .as_ref()
            .ok_or(Error::invalid_operation("Renderer is not initialize."))?;
        let window_surface = self
            .window_surface
            .as_ref()
            .ok_or(Error::invalid_operation("Renderer is not initialize."))?;

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
            Err(Error::fail("Requested device is not available."))
        } else {
            Ok(queue_families)
        }
    }

    /// Frees the resources used by the graphics device, and sets the currently
    /// active device to `None`.
    pub(super) fn uninitialize_device(&mut self) -> Result<()> {
        if self.device_context.is_some() {
            self.wait_idle()?;
            self.destroy_rendering()?;
            self.destroy_swapchain()?;
            self.destroy_frame_sync()?;
            self.uninitialize_thread()?;
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
