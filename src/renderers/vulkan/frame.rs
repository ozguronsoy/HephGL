use std::{
    cell::UnsafeCell,
    sync::{Condvar, Mutex},
};

use crate::{
    Error, Result,
    renderers::{
        thread_context::ThreadContextArray,
        vulkan::{
            VulkanRenderer,
            thread::{
                ComputeThreadContext, GraphicsThreadContext, TransferThreadContext,
                VulkanThreadContext,
            },
        },
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
    pub(super) thread_contexts: ThreadContextArray<UnsafeCell<ThreadContext>>,
    /// The frame state of each thread, which other threads wait on while the thread is recording.
    pub(super) sync_states: ThreadContextArray<(Mutex<FrameState>, Condvar)>,
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
}

impl<ThreadContext: VulkanThreadContext> Default for FrameBase<ThreadContext> {
    fn default() -> Self {
        Self {
            thread_contexts: std::array::from_fn(|_| UnsafeCell::default()),
            sync_states: std::array::from_fn(|_| Default::default()),
        }
    }
}

impl VulkanFrame for TransferFrame {
    type ThreadContext = TransferThreadContext;

    fn base(&self) -> &FrameBase<TransferThreadContext> {
        &self.base
    }
}

impl VulkanFrame for GraphicsFrame {
    type ThreadContext = GraphicsThreadContext;

    fn base(&self) -> &FrameBase<GraphicsThreadContext> {
        &self.base
    }
}

impl VulkanFrame for ComputeFrame {
    type ThreadContext = ComputeThreadContext;

    fn base(&self) -> &FrameBase<ComputeThreadContext> {
        &self.base
    }
}

impl VulkanRenderer {
    /// Checks whether the call is being made inside a frame. If not, returns an error.
    pub(super) fn frame_only(&self, fn_name: &str) -> Result<()> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;
        // All queues start at the same time, so we can use any queue context here to check the
        // state.
        let state = device_context.graphics_queue_context.frames
            [device_context.current_frame_index.get()]
        .base
        .sync_states[thread_context_index]
            .0
            .lock()?;
        match *state != FrameState::Started {
            true => Err(Error::InvalidOperation(format!(
                "Cannot call `Renderer::{fn_name}` outside of a frame."
            ))),
            false => Ok(()),
        }
    }
}
