use std::sync::{LazyLock, atomic::AtomicU32};

/// Represents a unique ID of a renderer instance.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct RendererId(u32, u32);

impl RendererId {
    pub(super) fn new() -> Self {
        Self(generate_id(), 0)
    }

    pub(super) fn advance_device_generation(&mut self) {
        self.1 += 1;
    }
}

/// Generates a unique ID for the renderer using an always increasing thread-safe counter.
///
/// ### NOTE
/// Uniqueness is not guaranteed when the `u32` counter overflows, and the earlier renderers are
/// still alive. But if one creates more than `u32::MAX` renderers, they deserve the ID clashes...
fn generate_id() -> u32 {
    static COUNTER: LazyLock<AtomicU32> = LazyLock::new(|| AtomicU32::new(0));
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
