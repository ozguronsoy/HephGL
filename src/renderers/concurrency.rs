use crate::renderers::{Renderer, RendererResult};

/// An opaque handle used for sharing the [`Renderer`] instance safely across threads.
pub struct RendererHandle<T: Renderer> {
    /// Raw pointer to the renderer instance.
    pub(super) p_renderer: usize,
    pub(super) _marker: std::marker::PhantomData<T>,
}

/// A scoped wrapper that manages the thread-local resources of a [`Renderer`].
pub struct RendererWorker<T: Renderer> {
    /// Raw pointer to the renderer instance.
    pub(super) p_renderer: usize,
    /// The function used to release the thread-local resources.
    pub(super) uninitialize: fn(&mut T) -> RendererResult<()>,
    pub(super) _marker: std::marker::PhantomData<T>,
}

/// A factory for generating thread-specific rendering workers.
pub trait RendererWorkerFactory<T: Renderer> {
    /// Creates a [`RendererWorker`] for the current thread.
    fn spawn_worker(&self) -> RendererResult<RendererWorker<T>>;
}

impl<T: Renderer> Copy for RendererHandle<T> {}
impl<T: Renderer> Clone for RendererHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
unsafe impl<T: Renderer> Send for RendererHandle<T> {}
unsafe impl<T: Renderer> Sync for RendererHandle<T> {}
impl<T: Renderer> From<&mut T> for RendererHandle<T> {
    fn from(value: &mut T) -> Self {
        Self {
            p_renderer: (value as *mut T) as usize,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: Renderer> std::ops::Deref for RendererWorker<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*(self.p_renderer as *const T) }
    }
}
impl<T: Renderer> std::ops::DerefMut for RendererWorker<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *(self.p_renderer as *mut T) }
    }
}
impl<T: Renderer> Drop for RendererWorker<T> {
    fn drop(&mut self) {
        let result = (self.uninitialize)(unsafe { &mut *(self.p_renderer as *mut T) });
        if let Err(e) = result {
            eprintln!("Failed to uninitialze worker on drop: {}", e);
        }
    }
}
