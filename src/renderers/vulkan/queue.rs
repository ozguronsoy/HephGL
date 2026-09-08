use ash::vk::{Queue, QueueFlags};

use crate::renderers::vulkan::frame::Frame;

/// Represents the Vulkan queue type.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QueueType {
    Graphics,
    Transfer,
    Compute,
}

/// Represents a Vulkan queue family.
pub struct QueueFamily {
    /// The index of the graphics family.
    pub index: u32,
    /// The number of queues available for this family.
    pub queue_count: u32,
    pub queue_flags: QueueFlags,
    /// Indicates whether this family supports presenting to a device.
    pub present_supported: bool,
}

/// Represents context and state for a Vulkan queue.
pub struct QueueContext {
    /// The Vulkan queue instance.
    pub queue: Queue,
    /// The Vulkan queue type.
    pub queue_type: QueueType,
    /// The index of the queue family.
    pub queue_family_index: u32,
    /// Contains the resources per frame.
    ///
    /// ### Note
    /// Length of this must always be equal to `settings.frames_in_flight`.
    pub frames: Vec<Frame>,
}
