use std::any::Any;

use ash::vk::{
    CommandBuffer, CommandBufferAllocateInfo, CommandBufferLevel, CommandPool,
    CommandPoolCreateFlags, CommandPoolCreateInfo,
};

use crate::{
    Error, Result,
    renderers::vulkan::{
        THREAD_CONTEXT_INDEX, VulkanRenderer,
        descriptor_pool::DescriptorPoolManager,
        frame::{FrameState, VulkanFrame},
        queue::QueueContext,
        resources::VulkanBuffer,
    },
};

/// Represents the base struct per-thread resources used for recording commands.
#[derive(Default)]
pub struct ThreadContextBase {
    /// The command pool allocated exclusively for this thread.
    pub command_pool: CommandPool,
    /// The primary command buffer used to record commands for this thread.
    pub command_buffer: CommandBuffer,
    /// Indicates whether any command has been recorded in this thread.
    pub recorded: bool,
    /// Indicates whether the current thread began the current frame.
    pub sync_state: (std::sync::Mutex<FrameState>, std::sync::Condvar),
}

/// Represents the per-thread resources used for recording commands to the compute queue.
#[derive(Default)]
pub struct ComputeThreadContext {
    /// The base per-thread resources.
    pub base: ThreadContextBase,
    /// Manages descriptor pools for allocating this thread's descriptor sets. Pools are created on
    /// demand and grow as needed.
    pub descriptor_pool_manager: DescriptorPoolManager,
    /// The pipeline currently bound to the command buffer.
    pub bound_pipeline: Option<ash::vk::Pipeline>,
}

/// Represents the per-thread resources used for recording commands to the graphics queue.
#[derive(Default)]
pub struct GraphicsThreadContext {
    /// The base per-thread resources.
    pub base: ThreadContextBase,
    /// Manages descriptor pools for allocating this thread's descriptor sets. Pools are created on
    /// demand and grow as needed.
    pub descriptor_pool_manager: DescriptorPoolManager,
    /// The pipeline currently bound to the command buffer.
    pub bound_pipeline: Option<ash::vk::Pipeline>,
    /// The texture image of the canvas that the command buffer is rendering into, or `None` if it
    /// is not rendering.
    pub render_target: Option<ash::vk::Image>,
}

/// Represents the per-thread resources used for recording commands to the transfer queue.
#[derive(Default)]
pub struct TransferThreadContext {
    /// The base per-thread resources.
    pub base: ThreadContextBase,
    /// The temporary buffers used for transfering data to the GPU.
    pub transfer_buffers: Vec<VulkanBuffer>,
}

/// Provides access to the base per-thread resources of a queue's thread context.
pub trait VulkanThreadContext: Any + Default {
    /// Gets the base per-thread resources.
    fn base(&self) -> &ThreadContextBase;
    /// Gets the base per-thread resources as mutable.
    fn base_mut(&mut self) -> &mut ThreadContextBase;
}

impl VulkanThreadContext for TransferThreadContext {
    fn base(&self) -> &ThreadContextBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut ThreadContextBase {
        &mut self.base
    }
}

impl VulkanThreadContext for GraphicsThreadContext {
    fn base(&self) -> &ThreadContextBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut ThreadContextBase {
        &mut self.base
    }
}

impl VulkanThreadContext for ComputeThreadContext {
    fn base(&self) -> &ThreadContextBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut ThreadContextBase {
        &mut self.base
    }
}

impl VulkanRenderer {
    /// Checks whether the call is being made from the main thread.
    pub(super) fn is_in_main_thread(&self) -> bool {
        self.main_thread_id == std::thread::current().id()
    }

    /// Checks whether the call is being made from the main thread. If not,
    /// returns an error.
    pub(super) fn main_thread_only(&self) -> Result<()> {
        if self.is_in_main_thread() {
            Ok(())
        } else {
            Err(Error::invalid_operation(
                "This action can only be performed in the main thread.",
            ))
        }
    }

    /// Gets the current thread's context index.
    pub(super) fn thread_context_index() -> Result<usize> {
        THREAD_CONTEXT_INDEX.with(|index| index.try_get())
    }

    /// Initializes the per-thread execution resources for the renderer.
    ///
    /// ### Important
    /// This function must be run **once per worker thread** that will be
    /// recording commands.
    pub(super) fn initialize_thread(&mut self) -> Result<()> {
        if let Ok(current_thread_index) = Self::thread_context_index() {
            return Err(Error::InvalidOperation(format!(
                "Current thread ({}) already has a worker.",
                current_thread_index
            )));
        }

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;

        crate::renderers::thread_context::register(
            &THREAD_CONTEXT_INDEX,
            &mut *device_context.thread_context_masks.lock()?,
            self.main_thread_id == std::thread::current().id(),
        )?;

        self.create_command_pools()?;
        self.create_command_buffers()?;

        Ok(())
    }

    /// Creates a command pool for the current thread of each frame.
    pub(super) fn create_command_pools(&mut self) -> Result<()> {
        self.destroy_command_pools()?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;

        fn create_command_pool<Frame: VulkanFrame>(
            logical_device: &ash::Device,
            queue_context: &mut QueueContext<Frame>,
            frame_index: usize,
            thread_context_index: usize,
        ) -> Result<()> {
            let pool_info = CommandPoolCreateInfo::default()
                .flags(CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(queue_context.queue_family_index);
            unsafe {
                queue_context.frames[frame_index].base_mut().thread_contexts
                    [thread_context_index]
                    .base_mut()
                    .command_pool = logical_device.create_command_pool(&pool_info, None)?;
            }
            Ok(())
        }
        for frame_index in 0..self.settings.frames_in_flight {
            create_command_pool(
                &device_context.logical_device,
                &mut device_context.graphics_queue_context,
                frame_index,
                thread_context_index,
            )?;
            create_command_pool(
                &device_context.logical_device,
                &mut device_context.transfer_queue_context,
                frame_index,
                thread_context_index,
            )?;
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                create_command_pool(
                    &device_context.logical_device,
                    compute_queue_context,
                    frame_index,
                    thread_context_index,
                )?;
            }
        }

        Ok(())
    }

    /// Creates a command buffer for the current thread of each frame.
    pub(super) fn create_command_buffers(&mut self) -> Result<()> {
        self.destroy_command_buffers()?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;

        fn allocate_buffers<Frame: VulkanFrame>(
            logical_device: &ash::Device,
            queue_context: &mut QueueContext<Frame>,
            frame_index: usize,
            thread_context_index: usize,
        ) -> Result<()> {
            let thread_context = queue_context.frames[frame_index].base_mut().thread_contexts
                [thread_context_index]
                .base_mut();
            let alloc_info = CommandBufferAllocateInfo::default()
                .command_pool(thread_context.command_pool)
                .level(CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);
            unsafe {
                thread_context.command_buffer =
                    logical_device.allocate_command_buffers(&alloc_info)?[0];
            }
            Ok(())
        }
        for frame_index in 0..self.settings.frames_in_flight {
            allocate_buffers(
                &device_context.logical_device,
                &mut device_context.graphics_queue_context,
                frame_index,
                thread_context_index,
            )?;
            allocate_buffers(
                &device_context.logical_device,
                &mut device_context.transfer_queue_context,
                frame_index,
                thread_context_index,
            )?;
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                allocate_buffers(
                    &device_context.logical_device,
                    compute_queue_context,
                    frame_index,
                    thread_context_index,
                )?;
            }
        }

        Ok(())
    }

    /// Frees and destroys all per-frame execution resources allocated during
    /// `initialize_frames`.
    pub(super) fn uninitialize_thread(&mut self) -> Result<()> {
        if Self::thread_context_index().is_err() {
            // Err means the index is set to `INVALID_THREAD_CONTEXT_INDEX`, which means the
            // thread is already uninitialized.
            return Ok(());
        };

        // TODO: Wait for fences to avoid destroying resources before commands
        // dispatched to GPU finishes.
        self.destroy_transfer_buffers()?;
        self.destroy_descriptor_pools()?;
        self.destroy_command_buffers()?;
        self.destroy_command_pools()?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;

        crate::renderers::thread_context::unregister(
            &THREAD_CONTEXT_INDEX,
            &mut *device_context.thread_context_masks.lock()?,
        );

        Ok(())
    }

    /// Destroys the command pools of the current thread if there are any.
    pub(super) fn destroy_command_pools(&mut self) -> Result<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;

        fn destroy_command_pool<Frame: VulkanFrame>(
            logical_device: &ash::Device,
            queue_context: &mut QueueContext<Frame>,
            frame_index: usize,
            thread_context_index: usize,
        ) {
            let thread_context = queue_context.frames[frame_index].base_mut().thread_contexts
                [thread_context_index]
                .base_mut();
            unsafe {
                logical_device.destroy_command_pool(thread_context.command_pool, None);
            }
            thread_context.command_pool = CommandPool::default();
        }
        for frame_index in 0..self.settings.frames_in_flight {
            destroy_command_pool(
                &device_context.logical_device,
                &mut device_context.graphics_queue_context,
                frame_index,
                thread_context_index,
            );
            destroy_command_pool(
                &device_context.logical_device,
                &mut device_context.transfer_queue_context,
                frame_index,
                thread_context_index,
            );
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                destroy_command_pool(
                    &device_context.logical_device,
                    compute_queue_context,
                    frame_index,
                    thread_context_index,
                );
            }
        }

        Ok(())
    }

    /// Destroys the command buffers of the current thread if there are any.
    pub(super) fn destroy_command_buffers(&mut self) -> Result<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;

        fn destroy_command_buffer<Frame: VulkanFrame>(
            logical_device: &ash::Device,
            queue_context: &mut QueueContext<Frame>,
            frame_index: usize,
            thread_context_index: usize,
        ) {
            let thread_context = queue_context.frames[frame_index].base_mut().thread_contexts
                [thread_context_index]
                .base_mut();
            unsafe {
                logical_device.free_command_buffers(
                    thread_context.command_pool,
                    &[thread_context.command_buffer],
                );
            }
            thread_context.command_buffer = CommandBuffer::default();
        }
        for frame_index in 0..self.settings.frames_in_flight {
            destroy_command_buffer(
                &device_context.logical_device,
                &mut device_context.graphics_queue_context,
                frame_index,
                thread_context_index,
            );
            destroy_command_buffer(
                &device_context.logical_device,
                &mut device_context.transfer_queue_context,
                frame_index,
                thread_context_index,
            );
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                destroy_command_buffer(
                    &device_context.logical_device,
                    compute_queue_context,
                    frame_index,
                    thread_context_index,
                );
            }
        }

        Ok(())
    }

    /// Destroys the descriptor pools of the current thread if there are any.
    pub(super) fn destroy_descriptor_pools(&mut self) -> Result<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;

        for frame_index in 0..self.settings.frames_in_flight {
            device_context.graphics_queue_context.frames[frame_index]
                .base
                .thread_contexts[thread_context_index]
                .descriptor_pool_manager
                .destroy(&device_context.logical_device);
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                compute_queue_context.frames[frame_index]
                    .base
                    .thread_contexts[thread_context_index]
                    .descriptor_pool_manager
                    .destroy(&device_context.logical_device);
            }
        }

        Ok(())
    }

    /// Destroys the transfer buffers of the current thread if there are any.
    pub(super) fn destroy_transfer_buffers(&mut self) -> Result<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;

        for frame_index in 0..self.settings.frames_in_flight {
            for mut transfer_buffer in device_context.transfer_queue_context.frames[frame_index]
                .base
                .thread_contexts[thread_context_index]
                .transfer_buffers
                .drain(..)
            {
                unsafe {
                    device_context.vma_allocator.destroy_buffer(
                        transfer_buffer.buffer,
                        &mut transfer_buffer.vma_allocation,
                    );
                }
            }
        }

        Ok(())
    }
}
