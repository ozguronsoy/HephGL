use ash::vk::{
    CommandBuffer, CommandBufferAllocateInfo, CommandBufferLevel, CommandPool,
    CommandPoolCreateFlags, CommandPoolCreateInfo, DescriptorPool, DescriptorPoolCreateInfo,
    DescriptorPoolSize, DescriptorType,
};

use crate::renderers::{
    RendererResult,
    error::RendererError,
    thread_context::ThreadContextArray,
    vulkan::{VulkanRenderer, queue::QueueContext},
};

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
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            thread_contexts: std::array::from_fn(|_| ThreadContext::default()),
        }
    }
}

impl VulkanRenderer {
    /// Creates a command pool for the current thread of each frame.
    pub(super) fn create_command_pools(&mut self) -> RendererResult<()> {
        self.destroy_command_pools()?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let fif = self.settings.frames_in_flight as usize;
        let thread_context_index = Self::thread_context_index()?;

        let create_command_pool =
            |queue_context: &mut QueueContext, frame_index: usize| -> RendererResult<()> {
                let pool_info = CommandPoolCreateInfo::default()
                    .flags(CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                    .queue_family_index(queue_context.queue_family_index);
                unsafe {
                    queue_context.frames[frame_index].thread_contexts[thread_context_index]
                        .command_pool = device_context
                        .logical_device
                        .create_command_pool(&pool_info, None)?;
                }
                Ok(())
            };

        for frame_index in 0..fif {
            create_command_pool(&mut device_context.graphics_queue_context, frame_index)?;
            if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
                create_command_pool(transfer_queue_context, frame_index)?;
            }
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                create_command_pool(compute_queue_context, frame_index)?;
            }
        }

        Ok(())
    }

    /// Creates a command buffer for the current thread of each frame.
    pub(super) fn create_command_buffers(&mut self) -> RendererResult<()> {
        self.destroy_command_buffers()?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let fif = self.settings.frames_in_flight as usize;
        let thread_context_index = Self::thread_context_index()?;

        let allocate_buffers =
            |queue_context: &mut QueueContext, frame_index: usize| -> RendererResult<()> {
                let frame = &mut queue_context.frames[frame_index];
                let alloc_info = CommandBufferAllocateInfo::default()
                    .command_pool(frame.thread_contexts[thread_context_index].command_pool)
                    .level(CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1);
                unsafe {
                    frame.thread_contexts[thread_context_index].command_buffer = device_context
                        .logical_device
                        .allocate_command_buffers(&alloc_info)?[0];
                }
                Ok(())
            };

        for frame_index in 0..fif {
            allocate_buffers(&mut device_context.graphics_queue_context, frame_index)?;
            if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
                allocate_buffers(transfer_queue_context, frame_index)?;
            }
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                allocate_buffers(compute_queue_context, frame_index)?;
            }
        }

        Ok(())
    }

    /// Creates descriptor pools for the current thread of each frame.
    pub(super) fn create_descriptor_pools(&mut self) -> RendererResult<()> {
        const DESCRIPTOR_COUNT: u32 = 1000;
        const DESCRIPTOR_MAX_SETS: u32 = 1000;

        self.destroy_descriptor_pools()?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let fif = self.settings.frames_in_flight as usize;
        let thread_context_index = Self::thread_context_index()?;

        let pool_sizes = [
            DescriptorPoolSize::default()
                .ty(DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(DESCRIPTOR_COUNT),
            DescriptorPoolSize::default()
                .ty(DescriptorType::STORAGE_BUFFER)
                .descriptor_count(DESCRIPTOR_COUNT),
        ];

        let pool_info = DescriptorPoolCreateInfo::default()
            .max_sets(DESCRIPTOR_MAX_SETS)
            .pool_sizes(&pool_sizes);

        let allocate_pool =
            |queue_context: &mut QueueContext, frame_index: usize| -> RendererResult<()> {
                unsafe {
                    queue_context.frames[frame_index].thread_contexts[thread_context_index]
                        .descriptor_pool = device_context
                        .logical_device
                        .create_descriptor_pool(&pool_info, None)?;
                    Ok(())
                }
            };

        for frame_index in 0..fif {
            allocate_pool(&mut device_context.graphics_queue_context, frame_index)?;
            if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
                allocate_pool(transfer_queue_context, frame_index)?;
            }
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                allocate_pool(compute_queue_context, frame_index)?;
            }
        }

        Ok(())
    }

    /// Destroys the command pools of the current thread if there are any.
    pub(super) fn destroy_command_pools(&mut self) -> RendererResult<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let fif = self.settings.frames_in_flight as usize;
        let thread_context_index = Self::thread_context_index()?;

        let destroy_command_pool = |queue_context: &mut QueueContext, frame_index: usize| unsafe {
            let frame = &mut queue_context.frames[frame_index];
            device_context.logical_device.destroy_command_pool(
                frame.thread_contexts[thread_context_index].command_pool,
                None,
            );
            frame.thread_contexts[thread_context_index].command_pool = CommandPool::default();
        };
        for frame_index in 0..fif {
            destroy_command_pool(&mut device_context.graphics_queue_context, frame_index);
            if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
                destroy_command_pool(transfer_queue_context, frame_index);
            }
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                destroy_command_pool(compute_queue_context, frame_index);
            }
        }

        Ok(())
    }

    /// Destroys the command buffers of the current thread if there are any.
    pub(super) fn destroy_command_buffers(&mut self) -> RendererResult<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let fif = self.settings.frames_in_flight as usize;
        let thread_context_index = Self::thread_context_index()?;

        let destroy_command_buffer = |queue_context: &mut QueueContext, frame_index: usize| unsafe {
            let frame = &mut queue_context.frames[frame_index];
            device_context.logical_device.free_command_buffers(
                frame.thread_contexts[thread_context_index].command_pool,
                &[frame.thread_contexts[thread_context_index].command_buffer],
            );
            frame.thread_contexts[thread_context_index].command_buffer = CommandBuffer::default();
        };

        for frame_index in 0..fif {
            destroy_command_buffer(&mut device_context.graphics_queue_context, frame_index);
            if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
                destroy_command_buffer(transfer_queue_context, frame_index);
            }
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                destroy_command_buffer(compute_queue_context, frame_index);
            }
        }

        Ok(())
    }

    /// Destroys the descriptor pools of the current thread if there are any.
    pub(super) fn destroy_descriptor_pools(&mut self) -> RendererResult<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let fif = self.settings.frames_in_flight as usize;
        let thread_context_index = Self::thread_context_index()?;

        let destroy_desc_pool = |queue_context: &mut QueueContext, frame_index: usize| unsafe {
            let frame = &mut queue_context.frames[frame_index];
            device_context.logical_device.destroy_descriptor_pool(
                frame.thread_contexts[thread_context_index].descriptor_pool,
                None,
            );
            frame.thread_contexts[thread_context_index].descriptor_pool = DescriptorPool::default();
        };

        for frame_index in 0..fif {
            destroy_desc_pool(&mut device_context.graphics_queue_context, frame_index);
            if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
                destroy_desc_pool(transfer_queue_context, frame_index);
            }
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                destroy_desc_pool(compute_queue_context, frame_index);
            }
        }

        Ok(())
    }
}
