use ash::vk::{
    Fence, Queue, Semaphore, SemaphoreCreateInfo, SemaphoreType, SemaphoreTypeCreateInfo,
    SemaphoreWaitInfo, SubmitInfo, TimelineSemaphoreSubmitInfo,
};

use crate::renderers::{RendererResult, error::RendererError, vulkan::sync::VulkanFrameSync};

/// Implements frame synchronization using a timeline semaphore.
pub struct TimelineSemaphoreFrameSync {
    /// The timeline semaphore.
    semaphore: Semaphore,
    /// The last signaled value for each frame.
    frame_values: Vec<u64>,
    /// The value used for the next submission.
    new_value: u64,
}

impl VulkanFrameSync for TimelineSemaphoreFrameSync {
    fn new(device: &ash::Device, frames_in_flight: u32) -> RendererResult<Self>
    where
        Self: Sized,
    {
        let mut type_info = SemaphoreTypeCreateInfo::default()
            .semaphore_type(SemaphoreType::TIMELINE)
            .initial_value(0);
        let create_info = SemaphoreCreateInfo::default().push_next(&mut type_info);
        let semaphore = unsafe { device.create_semaphore(&create_info, None)? };
        Ok(Self {
            semaphore,
            frame_values: vec![0; frames_in_flight as usize],
            new_value: 1,
        })
    }
    fn submit(
        &mut self,
        device: &ash::Device,
        frame_index: u32,
        queue: Queue,
        submits: &[SubmitInfo],
    ) -> RendererResult<()> {
        // TODO: Recreate the semaphore and use that.
        let next_value = self
            .new_value
            .checked_add(1)
            .ok_or(RendererError::fail("Timeline semaphore counter exhausted."))?;

        let semaphores = [self.semaphore];
        let values = [self.new_value];
        let mut timeline_info =
            TimelineSemaphoreSubmitInfo::default().signal_semaphore_values(&values);
        let timeline_submit = SubmitInfo::default()
            .signal_semaphores(&semaphores)
            .push_next(&mut timeline_info);
        let mut queue_submits = Vec::with_capacity(submits.len() + 1);
        queue_submits.extend_from_slice(submits);
        queue_submits.push(timeline_submit);
        unsafe {
            device.queue_submit(queue, &queue_submits, Fence::null())?;
        }

        self.frame_values[frame_index as usize] = self.new_value;
        self.new_value = next_value;

        Ok(())
    }
    fn wait(&mut self, device: &ash::Device, frame_index: u32) -> RendererResult<()> {
        let value = self.frame_values[frame_index as usize];
        if value == 0 {
            // Frame is not submitted.
            return Ok(());
        }

        let semaphores = [self.semaphore];
        let values = [value];
        let wait_info = SemaphoreWaitInfo::default()
            .semaphores(&semaphores)
            .values(&values);
        unsafe {
            device.wait_semaphores(&wait_info, Self::TIMEOUT_NS)?;
        }
        Ok(())
    }
    fn destroy(&mut self, device: &ash::Device) -> RendererResult<()> {
        for frame_index in 0..self.frame_values.len() as u32 {
            self.wait(device, frame_index)?;
        }
        unsafe {
            device.destroy_semaphore(self.semaphore, None);
        }
        Ok(())
    }
}

impl TimelineSemaphoreFrameSync {
    const TIMEOUT_NS: u64 = 1.0e9 as u64;
}
