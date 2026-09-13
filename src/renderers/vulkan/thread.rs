use crate::renderers::{
    RendererResult,
    error::RendererError,
    vulkan::{THREAD_CONTEXT_INDEX, VulkanRenderer},
};

impl VulkanRenderer {
    /// Checks whether the call is being made from the main thread. If not,
    /// returns an error.
    pub(super) fn main_thread_only(&self) -> RendererResult<()> {
        (self.main_thread_id == std::thread::current().id())
            .then_some(())
            .ok_or_else(|| {
                RendererError::invalid_operation(
                    "This action can only be performed in the main thread.",
                )
            })
    }

    /// Gets the current thread's context index.
    pub(super) fn thread_context_index() -> RendererResult<usize> {
        THREAD_CONTEXT_INDEX.with(|index| index.try_get())
    }

    /// Initializes the per-thread execution resources for the renderer.
    ///
    /// ### Important
    /// This function must be run **once per worker thread** that will be
    /// recording commands.
    pub(super) fn initialize_thread(&mut self) -> RendererResult<()> {
        if let Ok(current_thread_index) = Self::thread_context_index() {
            return Err(RendererError::InvalidOperation(format!(
                "Current thread ({}) already has a worker.",
                current_thread_index
            )));
        }

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        crate::renderers::thread_context::register(
            &THREAD_CONTEXT_INDEX,
            &mut *device_context.thread_context_masks.lock()?,
            self.main_thread_id == std::thread::current().id(),
        )?;

        self.create_command_pools()?;
        self.create_command_buffers()?;
        self.create_descriptor_pools()?;

        Ok(())
    }

    /// Frees and destroys all per-frame execution resources allocated during
    /// `initialize_frames`.
    pub(super) fn uninitialize_thread(&mut self) -> RendererResult<()> {
        if Self::thread_context_index().is_err() {
            // Err means the index is set to `INVALID_THREAD_CONTEXT_INDEX`, which means the
            // thread is already uninitialized.
            return Ok(());
        };

        // TODO: Wait for fences to avoid destroying resources before commands
        // dispatched to GPU finishes.
        self.destroy_descriptor_pools()?;
        self.destroy_command_buffers()?;
        self.destroy_command_pools()?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        crate::renderers::thread_context::unregister(
            &THREAD_CONTEXT_INDEX,
            &mut *device_context.thread_context_masks.lock()?,
        );

        Ok(())
    }
}
