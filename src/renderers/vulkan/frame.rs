use ash::vk::{CommandBuffer, CommandPool, DescriptorPool, Fence};

use crate::renderers::thread_context::ThreadContextArray;

/// Represents the per-thread resources used for recording commands.
#[derive(Default)]
pub struct ThreadContext {
    /// The command pool allocated exclusively for this thread.
    pub command_pool: CommandPool,
    /// The primary command buffer used to record commands for this thread.
    pub command_buffer: CommandBuffer,
    /// The descriptor pool allocated for resources used during this thread.
    pub descriptor_pool: DescriptorPool,
}

/// Represents the resources and synchronization state for a single frame.
pub struct Frame {
    /// Contains the resources used for recording commands per thread for this
    /// frame.
    pub thread_contexts: ThreadContextArray<ThreadContext>,
    /// The fence used to synchronize CPU and GPU execution for this frame.
    pub fence: Fence,
    /// Indicates whether the frame is currently executing on the GPU and has an
    /// active fence in flight.
    pub is_in_flight: bool,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            thread_contexts: std::array::from_fn(|_| ThreadContext::default()),
            fence: Fence::default(),
            is_in_flight: false,
        }
    }
}
