use crate::renderers::{
    thread_context::ThreadContextArray,
    vulkan::thread::{
        ComputeThreadContext, GraphicsThreadContext, TransferThreadContext, VulkanThreadContext,
    },
};

/// Defines possible frame states.
#[derive(Default, PartialEq)]
pub(super) enum FrameState {
    /// The frame is not started.
    #[default]
    Idle,
    /// The frame is started via `begin_frame` function.
    Started,
    /// The frame is finished via `end_frame` function.
    Finished,
}

/// Represents the base struct of the resources and synchronization state for a single frame.
pub(super) struct FrameBase<ThreadContext: VulkanThreadContext> {
    /// Contains the resources used for recording commands per thread for this
    /// frame.
    pub(super) thread_contexts: ThreadContextArray<ThreadContext>,
}

/// Represents the resources and synchronization state of the compute queue for a single frame.
#[derive(Default)]
pub(super) struct ComputeFrame {
    /// The base frame resources.
    pub(super) base: FrameBase<ComputeThreadContext>,
}

/// Represents the resources and synchronization state of the graphics queue for a single frame.
#[derive(Default)]
pub(super) struct GraphicsFrame {
    /// The base frame resources.
    pub(super) base: FrameBase<GraphicsThreadContext>,
}

/// Represents the resources and synchronization state of the transfer queue for a single frame.
#[derive(Default)]
pub(super) struct TransferFrame {
    /// The base frame resources.
    pub(super) base: FrameBase<TransferThreadContext>,
    /// The binary semaphore signaled by the transfer submission and waited by the graphics
    /// submission, or `None` when the transfer and graphics queues are the same queue.
    pub(super) semaphore: Option<ash::vk::Semaphore>,
}

/// Provides access to the base resources of a queue's frame.
pub(super) trait VulkanFrame: Default {
    /// The per-thread resources of the queue.
    type ThreadContext: VulkanThreadContext;

    /// Gets the base frame resources.
    fn base(&self) -> &FrameBase<Self::ThreadContext>;
    /// Gets the base frame resources as mutable.
    fn base_mut(&mut self) -> &mut FrameBase<Self::ThreadContext>;
}

impl<ThreadContext: VulkanThreadContext> Default for FrameBase<ThreadContext> {
    fn default() -> Self {
        Self {
            thread_contexts: std::array::from_fn(|_| ThreadContext::default()),
        }
    }
}

impl VulkanFrame for TransferFrame {
    type ThreadContext = TransferThreadContext;

    fn base(&self) -> &FrameBase<TransferThreadContext> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut FrameBase<TransferThreadContext> {
        &mut self.base
    }
}

impl VulkanFrame for GraphicsFrame {
    type ThreadContext = GraphicsThreadContext;

    fn base(&self) -> &FrameBase<GraphicsThreadContext> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut FrameBase<GraphicsThreadContext> {
        &mut self.base
    }
}

impl VulkanFrame for ComputeFrame {
    type ThreadContext = ComputeThreadContext;

    fn base(&self) -> &FrameBase<ComputeThreadContext> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut FrameBase<ComputeThreadContext> {
        &mut self.base
    }
}
