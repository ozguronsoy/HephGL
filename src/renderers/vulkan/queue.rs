use ash::vk::{Queue, QueueFlags};

use crate::renderers::{
    RendererResult,
    error::RendererError,
    vulkan::{
        VulkanRenderer,
        frame::Frame,
        sync::{
            VulkanFrameSync, fence::FenceFrameSync, timeline_semaphore::TimelineSemaphoreFrameSync,
        },
    },
};

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
    /// Provides frame synchronization.
    pub frame_sync: Option<Box<dyn VulkanFrameSync>>,
}

impl VulkanRenderer {
    /// Creates frame synchronization resources for each queue.
    pub(super) fn create_frame_sync(&mut self) -> RendererResult<()> {
        self.destroy_frame_sync()?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let fif = self.settings.frames_in_flight as usize;

        let create_queue_frame_sync = |queue_context: &mut QueueContext| -> RendererResult<()> {
            if device_context.supports_timeline_semaphore {
                queue_context.frame_sync = Some(Box::new(TimelineSemaphoreFrameSync::new(
                    &device_context.logical_device,
                    fif,
                )?));
            } else {
                queue_context.frame_sync = Some(Box::new(FenceFrameSync::new(
                    &device_context.logical_device,
                    fif,
                )?));
            }
            Ok(())
        };

        create_queue_frame_sync(&mut device_context.graphics_queue_context)?;
        if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
            create_queue_frame_sync(transfer_queue_context)?;
        }
        if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
            create_queue_frame_sync(compute_queue_context)?;
        }
        Ok(())
    }

    /// Destroys the frame synchronization resources of each queue.
    pub(super) fn destroy_frame_sync(&mut self) -> RendererResult<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        let destroy_queue_frame_sync = |queue_context: &mut QueueContext| -> RendererResult<()> {
            if let Some(mut frame_sync) = queue_context.frame_sync.take() {
                frame_sync.destroy(&device_context.logical_device)?;
            }
            Ok(())
        };

        destroy_queue_frame_sync(&mut device_context.graphics_queue_context)?;
        if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
            destroy_queue_frame_sync(transfer_queue_context)?;
        }
        if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
            destroy_queue_frame_sync(compute_queue_context)?;
        }
        Ok(())
    }
}
