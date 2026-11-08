use ash::vk::{Queue, QueueFlags};

use crate::renderers::vulkan::{frame::VulkanFrame, sync::VulkanFrameSync};

/// Represents a Vulkan queue family.
pub(super) struct QueueFamily {
    /// The index of the graphics family.
    pub(super) index: u32,
    /// The number of queues available for this family.
    pub(super) queue_count: u32,
    pub(super) queue_flags: QueueFlags,
    /// Indicates whether this family supports presenting to a device.
    pub(super) present_supported: bool,
}

/// Represents context and state for a Vulkan queue.
pub(super) struct QueueContext<Frame: VulkanFrame> {
    /// The Vulkan queue instance.
    pub(super) queue: Queue,
    /// The index of the queue family.
    pub(super) queue_family_index: u32,
    /// Contains the resources per frame.
    ///
    /// ### Note
    /// Length of this must always be equal to `settings.frames_in_flight`.
    pub(super) frames: Vec<Frame>,
    /// Provides frame synchronization.
    pub(super) frame_sync: Option<Box<dyn VulkanFrameSync>>,
}
