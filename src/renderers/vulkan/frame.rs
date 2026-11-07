use crate::renderers::{thread_context::ThreadContextArray, vulkan::thread::VulkanThreadContext};

/// Defines possible frame states.
#[derive(Default, PartialEq)]
pub enum FrameState {
    /// The frame is not started.
    #[default]
    Idle,
    /// The frame is started via `begin_frame` function.
    Started,
    /// The frame is finished via `end_frame` function.
    Finished,
}

/// Represents the resources and synchronization state for a single frame.
pub struct Frame<ThreadContext: VulkanThreadContext> {
    /// Contains the resources used for recording commands per thread for this
    /// frame.
    pub thread_contexts: ThreadContextArray<ThreadContext>,
}

impl<ThreadContext: VulkanThreadContext> Default for Frame<ThreadContext> {
    fn default() -> Self {
        Self {
            thread_contexts: std::array::from_fn(|_| ThreadContext::default()),
        }
    }
}
