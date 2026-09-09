use crate::{
    graphics_device::GraphicsDevice,
    renderers::{
        thread_context::ThreadContextMaskArray,
        vulkan::{queue::QueueContext, swapchain::SwapchainContext},
    },
};

/// Encapsulates the Vulkan device state.
///
/// ### Note
/// Order of the fields matter as it determines the destruction order.
pub struct DeviceContext {
    /// The currently active graphics device.
    pub graphics_device: GraphicsDevice,

    /// The memory allocator.
    pub vma_allocator: vk_mem::Allocator,

    pub graphics_queue_context: QueueContext,
    pub transfer_queue_context: Option<QueueContext>,
    pub compute_queue_context: Option<QueueContext>,

    pub swapchain_context: SwapchainContext,

    pub physical_device: ash::vk::PhysicalDevice,
    /// The logical Vulkan device.
    pub logical_device: ash::Device,

    /// The bitmasks indicating the availability of thread contexts.
    /// `0` means the context at that index is available, `1` means it is
    /// currently in use.
    pub thread_context_masks: std::sync::Mutex<ThreadContextMaskArray>,
}
