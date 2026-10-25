use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use heph_gl::{
    Error,
    renderers::{
        LATEST_API_VERSION, Renderer,
        concurrency::{RendererHandle, RendererWorkerFactory},
        settings::{InitializeOptions, Settings},
    },
};

use crate::{
    dummy_display_handle, dummy_window_handle, heph_expect_err, heph_expect_success,
    renderer::RendererTests,
};

impl<TestRenderer: Renderer> RendererTests<TestRenderer>
where
    RendererHandle<TestRenderer>: RendererWorkerFactory<TestRenderer>,
{
    pub(crate) fn test_calling_main_thread_only_fn_from_worker_thread(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);
        let renderer_handle = RendererHandle::from(&mut renderer);
        std::thread::scope(|s| {
            let _ = s.spawn(move || {
                let mut renderer_worker = heph_expect_success!(renderer_handle.spawn_worker());
                heph_expect_err!(
                    renderer_worker.set_settings(Settings::default()),
                    Error::invalid_operation("")
                );
                heph_expect_err!(
                    renderer_worker.initialize(&InitializeOptions {
                        app_name: "",
                        window_handle: dummy_window_handle!(),
                        display_handle: dummy_display_handle!(),
                        api_version: LATEST_API_VERSION,
                    }),
                    Error::invalid_operation("")
                );
                heph_expect_err!(renderer_worker.uninitialize(), Error::invalid_operation(""));
                heph_expect_err!(
                    renderer_worker.set_device(None, &[]),
                    Error::invalid_operation("")
                );
                heph_expect_err!(renderer_worker.wait_idle(), Error::invalid_operation(""));
            });
        });
    }

    pub(crate) fn test_multiple_workers_per_thread(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);
        let renderer_handle = RendererHandle::from(&mut renderer);

        std::thread::scope(|s| {
            let _ = s.spawn(move || {
                // We can spawn another worker after the first one is dropped.
                let _ = heph_expect_success!(renderer_handle.spawn_worker());
                let _ = heph_expect_success!(renderer_handle.spawn_worker());
            });
        });

        std::thread::scope(|s| {
            let _ = s.spawn(move || {
                let _w1 = heph_expect_success!(renderer_handle.spawn_worker());
                heph_expect_err!(renderer_handle.spawn_worker(), Error::invalid_operation(""));
            });
        });
    }

    pub(crate) fn test_workers_begin_before_main(&self) {
        const THREAD_COUNT: usize = 10;
        const N_FRAMES: usize = 25;
        const FRAMES_IN_FLIGHT: usize = 3;

        let mut renderer = self.create_renderer_with_any_device(&[]);
        let renderer_handle = RendererHandle::from(&mut renderer);

        heph_expect_success!(renderer.set_settings(Settings {
            frames_in_flight: FRAMES_IN_FLIGHT,
            ..Default::default()
        }));

        std::thread::scope(|s| {
            let (ready_tx, ready_rx) = std::sync::mpsc::channel();
            let (begin_tx, begin_rx) = std::sync::mpsc::channel();

            for _ in 0..THREAD_COUNT {
                let ready_tx = ready_tx.clone();
                let begin_tx = begin_tx.clone();
                let _ = s.spawn(move || {
                    let mut renderer_worker = heph_expect_success!(renderer_handle.spawn_worker());
                    heph_expect_success!(ready_tx.send(()));
                    for _ in 0..N_FRAMES {
                        heph_expect_success!(renderer_worker.begin_frame());
                        heph_expect_success!(begin_tx.send(()));
                        heph_expect_success!(renderer_worker.end_frame());
                    }
                });
            }

            for _ in 0..THREAD_COUNT {
                heph_expect_success!(ready_rx.recv_timeout(Duration::from_secs(1)));
            }
            let mut early_begin_result = None;
            for frame in 0..N_FRAMES {
                if frame == 0 {
                    early_begin_result = begin_rx.recv_timeout(Duration::from_secs(1)).ok();
                }
                heph_expect_success!(renderer.begin_frame());
                for _ in usize::from(frame == 0 && early_begin_result.is_some())..THREAD_COUNT {
                    heph_expect_success!(begin_rx.recv_timeout(Duration::from_secs(1)));
                }
                heph_expect_success!(renderer.end_frame());
            }

            assert!(
                early_begin_result.is_none(),
                "worker begin_frame returned before the main thread began its frame"
            );
        });
    }

    pub(crate) fn test_excess_threads(&self) {
        // This should exceed `RENDERER_MAX_CONCURRENT_THREADS`.
        let thread_count = heph_gl::renderers::max_concurrent_threads() + 1;

        let mut renderer = self.create_renderer_with_any_device(&[]);
        let renderer_handle = RendererHandle::from(&mut renderer);
        let fail_count = AtomicUsize::new(0);
        std::thread::scope(|s| {
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(thread_count));
            for _ in 1..thread_count {
                let barrier = barrier.clone();
                let fail_count_ref = &fail_count;
                let _ = s.spawn(move || {
                    let result = renderer_handle.spawn_worker();
                    if let Err(e) = result {
                        if e == Error::fail("") {
                            let _ = fail_count_ref.fetch_add(1, Ordering::Relaxed);
                        }
                        let _ = barrier.wait();
                    } else {
                        let _worker = heph_expect_success!(result);
                        let _ = barrier.wait();
                    }
                });
            }
            let _ = barrier.wait();
            assert_ne!(fail_count.load(Ordering::Relaxed), 0);
        });
    }
}
