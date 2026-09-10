use std::{cell::Cell, thread::LocalKey};

use crate::renderers::{RendererError, RendererResult};

/// Dummy trait for selecting the optimal ThreadContextMask type.
#[cfg(not(test))]
pub trait MaskType {
    type Type;
}
/// Dummy struct for selecting the optimal ThreadContextMask type.
#[cfg(not(test))]
pub struct MaskSelector<const BITS: usize>;
#[cfg(not(test))]
impl MaskType for MaskSelector<128> {
    type Type = u128;
}
#[cfg(not(test))]
impl MaskType for MaskSelector<64> {
    type Type = u64;
}
#[cfg(not(test))]
impl MaskType for MaskSelector<32> {
    type Type = u32;
}
#[cfg(not(test))]
impl MaskType for MaskSelector<16> {
    type Type = u16;
}
#[cfg(not(test))]
impl MaskType for MaskSelector<8> {
    type Type = u8;
}

/// The type used as a bitmask to track thread context allocation states.
#[cfg(test)]
pub type ThreadContextMask = u8;
#[cfg(not(test))]
pub type ThreadContextMask = <MaskSelector<
    {
        if THREAD_CONTEXT_COUNT > 64 {
            128
        } else if THREAD_CONTEXT_COUNT > 32 {
            64
        } else if THREAD_CONTEXT_COUNT > 16 {
            32
        } else if THREAD_CONTEXT_COUNT > 8 {
            16
        } else {
            8
        }
    },
> as MaskType>::Type;
/// The type used as a thread-local index to track the location of the current thread's context.
pub struct ThreadContextIndex(Cell<usize>);
/// A static array of thread contexts.
pub type ThreadContextArray<ThreadContext> = [ThreadContext; THREAD_CONTEXT_COUNT];
/// A static array of [`ThreadContextMask`].
pub type ThreadContextMaskArray = [ThreadContextMask; THREAD_CONTEXT_MASK_COUNT];

/// The size of the `ThreadContextMask` in bits.
const THREAD_CONTEXT_MASK_BIT_SIZE: usize = std::mem::size_of::<ThreadContextMask>() * 8;
/// Indicates that the thread context index is invalid.
const INVALID_THREAD_CONTEXT_INDEX: usize = usize::MAX;
/// The maximum number of threads that we can concurrently operate including the main thread.
const THREAD_CONTEXT_COUNT: usize = {
    if let Some(val) = option_env!("HEPHGL_RENDERER_MAX_CONCURRENT_THREADS") {
        const_str::parse!(val, usize)
    } else {
        128
    }
};
/// The number of mask variables needed to track `THREAD_CONTEXT_COUNT` concurrent threads.
const THREAD_CONTEXT_MASK_COUNT: usize =
    THREAD_CONTEXT_COUNT.div_ceil(THREAD_CONTEXT_MASK_BIT_SIZE);
/// The index reserved for the main thread.
const MAIN_THREAD_CONTEXT_INDEX: usize = THREAD_CONTEXT_COUNT - 1;
/// The index of the mask that contains the index reserved for the main thread.
const MAIN_THREAD_CONTEXT_MASK_INDEX: usize =
    MAIN_THREAD_CONTEXT_INDEX / THREAD_CONTEXT_MASK_BIT_SIZE;
// static asserts
#[cfg(not(test))]
const _: () = assert!(
    (THREAD_CONTEXT_COUNT > 64 && std::mem::size_of::<ThreadContextMask>() == 16)
        || (THREAD_CONTEXT_COUNT > 32 && std::mem::size_of::<ThreadContextMask>() == 8)
        || (THREAD_CONTEXT_COUNT > 16 && std::mem::size_of::<ThreadContextMask>() == 4)
        || (THREAD_CONTEXT_COUNT > 8 && std::mem::size_of::<ThreadContextMask>() == 2)
        || (THREAD_CONTEXT_COUNT <= 8 && std::mem::size_of::<ThreadContextMask>() == 1)
);
const _: () = assert!(THREAD_CONTEXT_COUNT > 0);
const _: () = assert!(MAIN_THREAD_CONTEXT_INDEX < THREAD_CONTEXT_COUNT);

impl ThreadContextIndex {
    /// Creates a new instance and sets it to `INVALID_THREAD_CONTEXT_INDEX`.
    pub const fn new() -> Self {
        Self(Cell::new(INVALID_THREAD_CONTEXT_INDEX))
    }

    /// Gets the index if vaild, otherwise returns an error.
    pub fn try_get(&self) -> RendererResult<usize> {
        let index = self.0.get();
        if index == INVALID_THREAD_CONTEXT_INDEX {
            Err(RendererError::invalid_operation(
                "Current thread is not initialized.",
            ))
        } else {
            Ok(index)
        }
    }
}

/// Tries to find an available slot. Sets the index and updates the mask if found, otherwise returns
/// an error.
pub fn register(
    index: &'static LocalKey<ThreadContextIndex>,
    masks: &mut ThreadContextMaskArray,
    is_main_thread: bool,
) -> RendererResult<()> {
    index.with(|index| {
        if is_main_thread {
            masks[MAIN_THREAD_CONTEXT_MASK_INDEX] |=
                1 << (MAIN_THREAD_CONTEXT_INDEX % THREAD_CONTEXT_MASK_BIT_SIZE);
            index.0.set(MAIN_THREAD_CONTEXT_INDEX);
            return Ok(());
        }

        let mut new_index = 0;
        #[allow(clippy::needless_range_loop)]
        for i in 0..THREAD_CONTEXT_MASK_COUNT {
            let mask = &mut masks[i];
            let current_mask_index = if i == MAIN_THREAD_CONTEXT_MASK_INDEX {
                // Always set the bit at `MAIN_THREAD_CONTEXT_INDEX` to prevent assigning it
                // to a worker.
                (*mask | (1 << (MAIN_THREAD_CONTEXT_INDEX % THREAD_CONTEXT_MASK_BIT_SIZE)))
                    .trailing_ones() as usize
            } else {
                mask.trailing_ones() as usize
            };

            new_index += current_mask_index;
            if new_index >= THREAD_CONTEXT_COUNT {
                new_index = INVALID_THREAD_CONTEXT_INDEX;
                break;
            }
            if current_mask_index >= THREAD_CONTEXT_MASK_BIT_SIZE {
                // This mask is full.
                continue;
            }

            *mask |= 1 << current_mask_index;
            break;
        }

        index.0.set(new_index);
        if new_index == INVALID_THREAD_CONTEXT_INDEX {
            Err(RendererError::Fail(
                "Failed to initialize frames: maximum threads reached".to_string(),
            ))
        } else {
            Ok(())
        }
    })
}

/// Sets the index to `INVALID_THREAD_CONTEXT_INDEX` and updates the mask.
pub fn unregister(
    index: &'static LocalKey<ThreadContextIndex>,
    masks: &mut ThreadContextMaskArray,
) {
    index.with(|index| {
        let index_val = index.0.get();
        let mask = &mut masks[index_val / THREAD_CONTEXT_MASK_BIT_SIZE];
        *mask &= !(1 << (index_val % THREAD_CONTEXT_MASK_BIT_SIZE));
        index.0.set(INVALID_THREAD_CONTEXT_INDEX);
    })
}

/// Gets the maximum number of threads that can execute concurrently.
pub const fn thread_context_count() -> usize {
    THREAD_CONTEXT_COUNT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_context_index() {
        let index = ThreadContextIndex::new();
        assert_eq!(index.0.get(), INVALID_THREAD_CONTEXT_INDEX);
        let try_get = index.try_get();
        assert!(try_get.is_err());
        assert_eq!(try_get.err().unwrap(), RendererError::invalid_operation(""));
    }

    #[test]
    fn test_register_main_thread() {
        thread_local! {
          static INDEX: ThreadContextIndex = const { ThreadContextIndex::new() };
        }
        let mut masks: ThreadContextMaskArray =
            std::array::from_fn(|_| ThreadContextMask::default());

        assert!(register(&INDEX, &mut masks, true).is_ok());
        INDEX.with(|index| {
            assert_eq!(index.0.get(), MAIN_THREAD_CONTEXT_INDEX);
        });
        assert_eq!(
            masks[MAIN_THREAD_CONTEXT_MASK_INDEX],
            1 << (MAIN_THREAD_CONTEXT_INDEX % THREAD_CONTEXT_MASK_BIT_SIZE)
        );
    }

    #[test]
    fn test_register_worker_thread() {
        thread_local! {
          static INDEX: ThreadContextIndex = const { ThreadContextIndex::new() };
        }

        let mut masks: ThreadContextMaskArray =
            std::array::from_fn(|_| ThreadContextMask::default());
        assert!(register(&INDEX, &mut masks, false).is_ok());
        INDEX.with(|index| {
            assert_ne!(index.0.get(), INVALID_THREAD_CONTEXT_INDEX);
        });
        assert_eq!(masks[0], 1);
    }

    #[test]
    fn test_unregister() {
        thread_local! {
          static INDEX: ThreadContextIndex = const { ThreadContextIndex::new() };
        }

        let mut masks: ThreadContextMaskArray =
            std::array::from_fn(|_| ThreadContextMask::default());
        assert!(register(&INDEX, &mut masks, false).is_ok());
        unregister(&INDEX, &mut masks);
        INDEX.with(|index| {
            assert_eq!(index.0.get(), INVALID_THREAD_CONTEXT_INDEX);
        });
        assert_eq!(masks[0], 0);
    }

    #[test]
    fn test_multiple_masks() {
        const { assert!(THREAD_CONTEXT_MASK_COUNT > 1) };

        thread_local! {
          static INDEX: ThreadContextIndex = const { ThreadContextIndex::new() };
        }

        let masks: std::sync::Mutex<ThreadContextMaskArray> =
            std::sync::Mutex::new(std::array::from_fn(|_| ThreadContextMask::default()));
        std::thread::scope(|s| {
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(THREAD_CONTEXT_COUNT + 1));
            for i in 0..THREAD_CONTEXT_COUNT {
                let barrier = barrier.clone();
                let masks = &masks;
                s.spawn(move || {
                    let result = register(
                        &INDEX,
                        &mut masks.lock().unwrap(),
                        i == MAIN_THREAD_CONTEXT_INDEX,
                    );
                    assert!(result.is_ok());
                    barrier.wait();
                    barrier.wait();
                    unregister(&INDEX, &mut masks.lock().unwrap());
                    barrier.wait();
                });
            }
            barrier.wait();
            for mask in *masks.lock().unwrap() {
                assert_eq!(mask, ThreadContextMask::MAX);
            }
            barrier.wait();
            barrier.wait();
            for mask in *masks.lock().unwrap() {
                assert_eq!(mask, 0);
            }
        });
    }
}
