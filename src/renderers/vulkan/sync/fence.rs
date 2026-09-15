use ash::vk::{Fence, SubmitInfo};

use crate::renderers::{RendererResult, vulkan::sync::VulkanFrameSync};

/// Implements frame synchronization using fences.
pub struct FenceFrameSync {
    /// A list of Vulkan fence objects and `is_in_flight` flags indicating whether the current
    /// frame is submitted to the GPU for processing.
    ///
    /// ### Note
    /// Length of this must always be equal to `renderer.settings.frames_in_flight`.
    fences: Vec<(Fence, bool)>,
}

impl VulkanFrameSync for FenceFrameSync {
    fn new(device: &ash::Device, frames_in_flight: usize) -> RendererResult<Self>
    where
        Self: Sized,
    {
        let mut instance = FenceFrameSync {
            fences: vec![(Fence::default(), false); frames_in_flight],
        };
        let fence_create_info = ash::vk::FenceCreateInfo::default();
        for (fence, _) in &mut instance.fences {
            *fence = unsafe { device.create_fence(&fence_create_info, None)? };
        }
        Ok(instance)
    }
    fn submit(
        &mut self,
        device: &ash::Device,
        frame_index: usize,
        queue: ash::vk::Queue,
        submits: &[SubmitInfo],
    ) -> RendererResult<()> {
        // No need to check if `frame_index` is out of bounds, `VulkanRenderer` should guarantee it.
        let (fence, in_flight) = &mut self.fences[frame_index];
        unsafe {
            device.queue_submit(queue, submits, *fence)?;
            *in_flight = true;
        }
        Ok(())
    }
    fn wait(&mut self, device: &ash::Device, frame_index: usize) -> RendererResult<()> {
        // No need to check if `frame_index` is out of bounds, `VulkanRenderer` should guarantee it.
        let (fence, in_flight) = self.fences[frame_index];
        if in_flight {
            unsafe {
                device.wait_for_fences(&[fence], true, Self::TIMEOUT_NS)?;
                device.reset_fences(&[fence])?;
            }
        }
        Ok(())
    }
    fn destroy(&mut self, device: &ash::Device) -> RendererResult<()> {
        for (fence, in_flight) in &mut self.fences {
            unsafe {
                if *in_flight {
                    device.wait_for_fences(&[*fence], true, Self::TIMEOUT_NS)?;
                }
                device.destroy_fence(*fence, None);
                *fence = Fence::null();
            }
        }
        Ok(())
    }
}

impl FenceFrameSync {
    const TIMEOUT_NS: u64 = 1.0e9 as u64;
}
