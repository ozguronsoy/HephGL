use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};

use crate::renderers::{
    Error, RendererTask, RendererTaskParam, RendererWorker, Result, vulkan::VulkanRenderer,
};

pub(super) struct VulkanWorker {
    renderer: *mut VulkanRenderer,
    is_working: AtomicBool,
    thread_handle: Option<std::thread::JoinHandle<Option<Box<dyn RendererTaskParam>>>>,
}

impl RendererWorker for VulkanWorker {
    fn is_working(&self) -> bool {
        self.is_working.load(Ordering::Relaxed)
    }

    fn work(
        &mut self,
        tasks: &[RendererTask],
        mut param: Option<Box<dyn RendererTaskParam>>,
        once: bool,
    ) -> std::result::Result<(), (Error, Option<Box<dyn RendererTaskParam>>)> {
        if self.is_working() {
            return Err((
                Error::invalid_operation("The worker is already working."),
                param,
            ));
        }
        if tasks.is_empty() {
            return Err((Error::invalid_argument("`tasks` cannot be empty."), param));
        }

        let tasks = tasks.to_vec();
        let error = Arc::new(Mutex::new(None));
        let error_worker = error.clone();
        let worker = self as *mut VulkanWorker as usize;
        let owner = std::thread::current();
        let thread_handle = Some(std::thread::spawn(move || {
            let worker = unsafe { &mut *(worker as *mut VulkanWorker) };
            let renderer = unsafe { &mut *worker.renderer };
            if let Err(error) = renderer.initialize_thread() {
                let _ = error_worker
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(error);
                owner.unpark();
                return param;
            }

            worker.is_working.store(true, Ordering::Relaxed);
            drop(error_worker);
            owner.unpark();
            let mut abort = false;
            while !abort && worker.is_working() {
                for task in &tasks {
                    match task(
                        renderer,
                        param
                            .as_deref_mut()
                            .map(|param| param as &mut dyn RendererTaskParam),
                    ) {
                        Ok(p) => {
                            if p.is_some() {
                                param = p;
                            }
                        }
                        Err(error) => {
                            eprintln!("Worker task failed: {error}");
                            abort = true;
                        }
                    }
                }
                if once {
                    break;
                }
            }
            // Keep the thread resources alive until the worker is finished.
            while worker.is_working() {
                std::thread::park();
            }
            owner.unpark();
            if let Err(error) = renderer.uninitialize_thread() {
                eprintln!(
                    "Failed to release the thread-local resources of the renderer worker: {error}"
                );
            }
            param
        }));

        while !self.is_working() && thread_handle.as_ref().is_some_and(|th| !th.is_finished()) {
            std::thread::park();
        }
        if let Some(error) = error.lock().unwrap_or_else(PoisonError::into_inner).take() {
            return Err((
                error,
                thread_handle.and_then(|thread_handle| thread_handle.join().ok().flatten()),
            ));
        }
        self.thread_handle = thread_handle;

        Ok(())
    }

    fn finish(&mut self) -> Result<Option<Box<dyn RendererTaskParam>>> {
        let Some(thread_handle) = self.thread_handle.take() else {
            return Err(Error::invalid_operation(
                "Cannot finish a worker that's not working.",
            ));
        };
        self.is_working.store(false, Ordering::Relaxed);
        thread_handle.thread().unpark();

        thread_handle
            .join()
            .map_err(|_| Error::fail("Failed to join the worker thread."))
    }
}

impl Drop for VulkanWorker {
    fn drop(&mut self) {
        if self.thread_handle.is_some()
            && let Err(error) = self.finish()
        {
            eprintln!("Failed to finish a worker on drop: {error}");
            std::process::abort();
        }
    }
}

impl VulkanWorker {
    pub(super) fn new(renderer: &mut VulkanRenderer) -> Self {
        Self {
            renderer,
            is_working: AtomicBool::new(false),
            thread_handle: None,
        }
    }
}
