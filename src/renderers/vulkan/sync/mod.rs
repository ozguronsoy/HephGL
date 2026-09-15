pub mod fence;
pub mod timeline_semaphore;

use ash::vk::{Queue, SubmitInfo};

use crate::renderers::RendererResult;

/// Provides frame synchronization operations.
pub trait VulkanFrameSync {
    /// Creates the resources required for frame synchronization.
    fn new(device: &ash::Device, frames_in_flight: u32) -> RendererResult<Self>
    where
        Self: Sized;
    /// Submits the command buffers to the queue.
    fn submit(
        &mut self,
        device: &ash::Device,
        frame_index: u32,
        queue: Queue,
        submits: &[SubmitInfo],
    ) -> RendererResult<()>;
    /// Waits for GPU to finish processing the frame.
    fn wait(&mut self, device: &ash::Device, frame_index: u32) -> RendererResult<()>;
    /// Frees the allocated resources.
    fn destroy(&mut self, device: &ash::Device) -> RendererResult<()>;
}
