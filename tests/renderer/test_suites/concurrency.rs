use std::{sync::mpsc::Sender, time::Duration};

use heph_gl::{
    Error, Result,
    renderers::{Renderer, RendererTaskParam, Settings},
};

use crate::{heph_expect_err, heph_expect_success, renderer::RendererTests};

fn idle_task(
    _: &dyn Renderer,
    _: Option<&mut dyn RendererTaskParam>,
) -> Result<Option<Box<dyn RendererTaskParam>>> {
    Ok(None)
}

impl<TestRenderer: Renderer> RendererTests<TestRenderer> {
    pub(crate) fn test_calling_main_thread_only_fn_from_worker_thread(&self) {
        fn setup_task(
            renderer: &dyn Renderer,
            _: Option<&mut dyn RendererTaskParam>,
        ) -> Result<Option<Box<dyn RendererTaskParam>>> {
            heph_expect_err!(renderer.wait_idle(), Error::InvalidOperation(_));
            Ok(None)
        }

        let renderer = self.create_renderer_with_any_device(&[]);
        let mut worker = renderer.spawn_worker();
        heph_expect_success!(worker.work(&[setup_task], None, true).map_err(|(e, _)| e));
        let _ = heph_expect_success!(worker.finish());
    }

    pub(crate) fn test_calling_main_thread_only_fn_while_worker_active(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);
        let mut worker = renderer.spawn_worker();
        heph_expect_success!(worker.work(&[idle_task], None, true).map_err(|(e, _)| e));

        heph_expect_success!(renderer.wait_idle());
        let _ = heph_expect_success!(worker.finish());
        drop(worker);

        let debugger = heph_expect_success!(renderer.detach_debugger());
        heph_expect_success!(renderer.attach_debugger(debugger).map_err(|(e, _)| e));
        heph_expect_success!(renderer.set_settings(Settings::default()));
        heph_expect_success!(renderer.wait_idle());
    }

    pub(crate) fn test_reusing_worker(&self) {
        let renderer = self.create_renderer_with_any_device(&[]);
        let mut worker = renderer.spawn_worker();
        assert!(!worker.is_working());
        heph_expect_err!(worker.finish(), Error::InvalidOperation(_));
        heph_expect_err!(
            worker.work(&[], None, true).map_err(|(e, _)| e),
            Error::InvalidArgument(_)
        );

        heph_expect_success!(worker.work(&[idle_task], None, true).map_err(|(e, _)| e));
        assert!(worker.is_working());
        heph_expect_err!(
            worker.work(&[idle_task], None, true).map_err(|(e, _)| e),
            Error::InvalidOperation(_)
        );
        let _ = heph_expect_success!(worker.finish());
        assert!(!worker.is_working());

        heph_expect_success!(worker.work(&[idle_task], None, false).map_err(|(e, _)| e));
        assert!(worker.is_working());
        let _ = heph_expect_success!(worker.finish());
        assert!(!worker.is_working());
        heph_expect_err!(worker.finish(), Error::InvalidOperation(_));
    }

    pub(crate) fn test_workers_begin_before_main(&self) {
        const THREAD_COUNT: usize = 10;
        const N_FRAMES: usize = 25;
        const FRAMES_IN_FLIGHT: usize = 3;

        struct FrameSignal {
            begin_tx: Sender<()>,
        }
        impl RendererTaskParam for FrameSignal {}

        fn frames_task(
            renderer: &dyn Renderer,
            param: Option<&mut dyn RendererTaskParam>,
        ) -> Result<Option<Box<dyn RendererTaskParam>>> {
            let param = heph_expect_success!(
                heph_expect_success!(param.ok_or("Missing the frame signal."))
                    .downcast_ref::<FrameSignal>()
            );
            for _ in 0..N_FRAMES {
                heph_expect_success!(renderer.begin_frame());
                heph_expect_success!(param.begin_tx.send(()));
                heph_expect_success!(renderer.end_frame());
            }
            Ok(None)
        }

        let mut renderer = self.create_renderer_with_any_device(&[]);
        heph_expect_success!(renderer.set_settings(Settings {
            frames_in_flight: FRAMES_IN_FLIGHT,
            ..Default::default()
        }));

        let (begin_tx, begin_rx) = std::sync::mpsc::channel();
        let mut workers = (0..THREAD_COUNT)
            .map(|_| {
                let mut worker = renderer.spawn_worker();
                heph_expect_success!(
                    worker
                        .work(
                            &[frames_task],
                            Some(Box::new(FrameSignal {
                                begin_tx: begin_tx.clone(),
                            })),
                            true,
                        )
                        .map_err(|(e, _)| e)
                );
                worker
            })
            .collect::<Vec<_>>();

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

        for worker in &mut workers {
            let _ = heph_expect_success!(worker.finish());
        }

        assert!(
            early_begin_result.is_none(),
            "worker begin_frame returned before the main thread began its frame"
        );
    }

    pub(crate) fn test_excess_threads(&self) {
        // This should exceed `RENDERER_MAX_CONCURRENT_THREADS`.
        let thread_count = heph_gl::renderers::max_concurrent_threads() + 1;

        let renderer = self.create_renderer_with_any_device(&[]);
        let mut workers = Vec::with_capacity(thread_count);
        let mut fail_count = 0;
        for _ in 1..thread_count {
            let mut worker = renderer.spawn_worker();
            match worker.work(&[idle_task], None, true) {
                Ok(()) => workers.push(worker),
                Err((e, _)) => {
                    if matches!(e, Error::Fail(_)) {
                        fail_count += 1;
                    }
                }
            }
        }
        assert_ne!(fail_count, 0);

        for worker in &mut workers {
            let _ = heph_expect_success!(worker.finish());
        }
    }
}
