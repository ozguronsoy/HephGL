pub mod fence;
pub mod timeline_semaphore;

use ash::vk::{Queue, SubmitInfo};

use crate::renderers::RendererResult;

/// Provides frame synchronization operations.
pub trait VulkanFrameSync {
    /// Creates the resources required for frame synchronization.
    fn new(device: &ash::Device, frames_in_flight: usize) -> RendererResult<Self>
    where
        Self: Sized;
    /// Submits the command buffers to the queue.
    fn submit(
        &mut self,
        device: &ash::Device,
        frame_index: usize,
        queue: Queue,
        submits: &[SubmitInfo],
    ) -> RendererResult<()>;
    /// Waits for GPU to finish processing the frame.
    fn wait(&mut self, device: &ash::Device, frame_index: usize) -> RendererResult<()>;
    /// Frees the allocated resources.
    fn destroy(&mut self, device: &ash::Device) -> RendererResult<()>;
}
