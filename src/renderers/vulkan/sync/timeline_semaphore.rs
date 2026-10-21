use ash::vk::{
    Fence, Queue, Semaphore, SemaphoreCreateInfo, SemaphoreType, SemaphoreTypeCreateInfo,
    SemaphoreWaitInfo, SubmitInfo, TimelineSemaphoreSubmitInfo,
};

use crate::{Result, renderers::vulkan::sync::VulkanFrameSync};

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
    fn new(device: &ash::Device, frames_in_flight: usize) -> Result<Self>
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
            frame_values: vec![0; frames_in_flight],
            new_value: 1,
        })
    }
    fn submit(
        &mut self,
        device: &ash::Device,
        frame_index: usize,
        queue: Queue,
        submits: &[SubmitInfo],
    ) -> Result<()> {
        if self.new_value == u64::MAX {
            unsafe {
                device.queue_wait_idle(queue)?;
                let temp = Self::new(device, self.frame_values.len())?;
                device.destroy_semaphore(self.semaphore, None);
                *self = temp;
            }
        }
        let next_value = self.new_value + 1;

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

        self.frame_values[frame_index] = self.new_value;
        self.new_value = next_value;

        Ok(())
    }
    fn wait(&mut self, device: &ash::Device, frame_index: usize) -> Result<()> {
        let value = self.frame_values[frame_index];
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
    fn destroy(&mut self, device: &ash::Device) -> Result<()> {
        for frame_index in 0..self.frame_values.len() {
            self.wait(device, frame_index)?;
        }
        unsafe {
            device.destroy_semaphore(self.semaphore, None);
        }
        Ok(())
    }
}

impl TimelineSemaphoreFrameSync {
    const TIMEOUT_NS: u64 = 1.0e10 as u64;
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, ffi::c_void};

    use ash::vk::{self, Handle};

    use super::*;

    #[derive(Default)]
    struct Calls {
        events: Vec<&'static str>,
        wait_result: vk::Result,
        create_result: vk::Result,
        submit_result: vk::Result,
        signal: Option<(Semaphore, u64)>,
        destroyed: Option<Semaphore>,
    }

    thread_local! {
        static CALLS: RefCell<Calls> = RefCell::new(Calls::default());
    }

    unsafe extern "system" fn queue_wait_idle(_: Queue) -> vk::Result {
        CALLS.with_borrow_mut(|calls| {
            calls.events.push("wait");
            calls.wait_result
        })
    }

    unsafe extern "system" fn create_semaphore(
        _: vk::Device,
        _: *const SemaphoreCreateInfo<'_>,
        _: *const vk::AllocationCallbacks<'_>,
        semaphore: *mut Semaphore,
    ) -> vk::Result {
        CALLS.with_borrow_mut(|calls| {
            calls.events.push("create");
            if calls.create_result == vk::Result::SUCCESS {
                unsafe {
                    *semaphore = Semaphore::from_raw(2);
                }
            }
            calls.create_result
        })
    }

    unsafe extern "system" fn destroy_semaphore(
        _: vk::Device,
        semaphore: Semaphore,
        _: *const vk::AllocationCallbacks<'_>,
    ) {
        CALLS.with_borrow_mut(|calls| {
            calls.events.push("destroy");
            calls.destroyed = Some(semaphore);
        });
    }

    unsafe extern "system" fn queue_submit(
        _: Queue,
        count: u32,
        submits: *const SubmitInfo<'_>,
        _: Fence,
    ) -> vk::Result {
        let submit = unsafe { &*submits.add(count as usize - 1) };
        let timeline = unsafe { &*submit.p_next.cast::<TimelineSemaphoreSubmitInfo<'_>>() };
        CALLS.with_borrow_mut(|calls| {
            calls.events.push("submit");
            calls.signal = Some(unsafe {
                (
                    *submit.p_signal_semaphores,
                    *timeline.p_signal_semaphore_values,
                )
            });
            calls.submit_result
        })
    }

    fn device() -> ash::Device {
        unsafe {
            ash::Device::load_with(
                |name| match name.to_bytes() {
                    b"vkQueueWaitIdle" => queue_wait_idle as *const c_void,
                    b"vkCreateSemaphore" => create_semaphore as *const c_void,
                    b"vkDestroySemaphore" => destroy_semaphore as *const c_void,
                    b"vkQueueSubmit" => queue_submit as *const c_void,
                    _ => std::ptr::null(),
                },
                vk::Device::null(),
            )
        }
    }

    #[test]
    fn test_counter_exhaustion() {
        CALLS.set(Calls::default());
        let device = device();
        let mut sync = TimelineSemaphoreFrameSync {
            semaphore: Semaphore::from_raw(1),
            frame_values: vec![u64::MAX - 2, 0],
            new_value: u64::MAX - 1,
        };

        sync.submit(&device, 1, Queue::null(), &[]).unwrap();
        assert_eq!(sync.new_value, u64::MAX);
        CALLS.with_borrow(|calls| {
            assert_eq!(calls.events, ["submit"]);
            assert_eq!(calls.signal, Some((Semaphore::from_raw(1), u64::MAX - 1)));
        });
        CALLS.set(Calls::default());

        sync.submit(&device, 0, Queue::null(), &[]).unwrap();
        assert_eq!(sync.semaphore, Semaphore::from_raw(2));
        assert_eq!(sync.frame_values, [1, 0]);
        assert_eq!(sync.new_value, 2);
        CALLS.with_borrow(|calls| {
            assert_eq!(calls.events, ["wait", "create", "destroy", "submit"]);
            assert_eq!(calls.destroyed, Some(Semaphore::from_raw(1)));
            assert_eq!(calls.signal, Some((Semaphore::from_raw(2), 1)));
        });

        sync.submit(&device, 1, Queue::null(), &[]).unwrap();
        assert_eq!(sync.frame_values, [1, 2]);
        assert_eq!(sync.new_value, 3);
    }

    #[test]
    fn test_recreation_failure() {
        let device = device();
        for fail_wait in [true, false] {
            CALLS.set(Calls {
                wait_result: if fail_wait {
                    vk::Result::ERROR_DEVICE_LOST
                } else {
                    vk::Result::SUCCESS
                },
                create_result: vk::Result::ERROR_OUT_OF_HOST_MEMORY,
                ..Default::default()
            });
            let mut sync = TimelineSemaphoreFrameSync {
                semaphore: Semaphore::from_raw(1),
                frame_values: vec![u64::MAX - 1],
                new_value: u64::MAX,
            };
            assert!(sync.submit(&device, 0, Queue::null(), &[]).is_err());
            assert_eq!(sync.semaphore, Semaphore::from_raw(1));
            assert_eq!(sync.frame_values, [u64::MAX - 1]);
            assert_eq!(sync.new_value, u64::MAX);
            CALLS.with_borrow(|calls| {
                assert!(calls.destroyed.is_none());
                assert!(calls.signal.is_none());
            });

            CALLS.set(Calls::default());
            sync.submit(&device, 0, Queue::null(), &[]).unwrap();
            assert_eq!(sync.frame_values, [1]);
            assert_eq!(sync.new_value, 2);
        }
    }

    #[test]
    fn test_submission_failure_after_recreation() {
        CALLS.set(Calls {
            submit_result: vk::Result::ERROR_OUT_OF_HOST_MEMORY,
            ..Default::default()
        });
        let device = device();
        let mut sync = TimelineSemaphoreFrameSync {
            semaphore: Semaphore::from_raw(1),
            frame_values: vec![u64::MAX - 1],
            new_value: u64::MAX,
        };
        assert!(sync.submit(&device, 0, Queue::null(), &[]).is_err());
        assert_eq!(sync.semaphore, Semaphore::from_raw(2));
        assert_eq!(sync.frame_values, [0]);
        assert_eq!(sync.new_value, 1);

        CALLS.set(Calls::default());
        sync.submit(&device, 0, Queue::null(), &[]).unwrap();
        assert_eq!(sync.frame_values, [1]);
        assert_eq!(sync.new_value, 2);
        CALLS.with_borrow(|calls| assert_eq!(calls.events, ["submit"]));
    }
}
