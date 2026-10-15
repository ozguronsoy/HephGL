pub mod fence;
pub mod timeline_semaphore;

use ash::vk::{Queue, SubmitInfo};

use crate::{
    Error, Result,
    renderers::vulkan::{
        VulkanRenderer,
        queue::QueueContext,
        sync::{fence::FenceFrameSync, timeline_semaphore::TimelineSemaphoreFrameSync},
    },
};

/// Provides frame synchronization operations.
pub trait VulkanFrameSync {
    /// Creates the resources required for frame synchronization.
    fn new(device: &ash::Device, frames_in_flight: usize) -> Result<Self>
    where
        Self: Sized;
    /// Submits the command buffers to the queue.
    fn submit(
        &mut self,
        device: &ash::Device,
        frame_index: usize,
        queue: Queue,
        submits: &[SubmitInfo],
    ) -> Result<()>;
    /// Waits for GPU to finish processing the frame.
    fn wait(&mut self, device: &ash::Device, frame_index: usize) -> Result<()>;
    /// Frees the allocated resources.
    fn destroy(&mut self, device: &ash::Device) -> Result<()>;
}

impl VulkanRenderer {
    /// Creates frame synchronization resources for each queue.
    pub(super) fn create_frame_sync(&mut self) -> Result<()> {
        self.destroy_frame_sync()?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;

        let create_queue_frame_sync = |queue_context: &mut QueueContext| -> Result<()> {
            if device_context.supports_timeline_semaphore {
                queue_context.frame_sync = Some(Box::new(TimelineSemaphoreFrameSync::new(
                    &device_context.logical_device,
                    self.settings.frames_in_flight,
                )?));
            } else {
                queue_context.frame_sync = Some(Box::new(FenceFrameSync::new(
                    &device_context.logical_device,
                    self.settings.frames_in_flight,
                )?));
            }
            Ok(())
        };

        create_queue_frame_sync(&mut device_context.graphics_queue_context)?;
        create_queue_frame_sync(&mut device_context.transfer_queue_context)?;
        if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
            create_queue_frame_sync(compute_queue_context)?;
        }
        Ok(())
    }

    /// Destroys the frame synchronization resources of each queue.
    pub(super) fn destroy_frame_sync(&mut self) -> Result<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;

        let destroy_queue_frame_sync = |queue_context: &mut QueueContext| -> Result<()> {
            if let Some(mut frame_sync) = queue_context.frame_sync.take() {
                frame_sync.destroy(&device_context.logical_device)?;
            }
            Ok(())
        };

        destroy_queue_frame_sync(&mut device_context.graphics_queue_context)?;
        destroy_queue_frame_sync(&mut device_context.transfer_queue_context)?;
        if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
            destroy_queue_frame_sync(compute_queue_context)?;
        }
        Ok(())
    }
}
