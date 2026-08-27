use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
#[cfg(target_os = "linux")]
use winit::platform::wayland::EventLoopBuilderExtWayland;
#[cfg(target_os = "linux")]
use winit::platform::x11::EventLoopBuilderExtX11;
use winit::{
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

#[macro_export]
macro_rules! heph_expect_success {
    ($expr: expr) => {
        match $expr {
            Ok(val) => val,
            Err(err) => panic!("Expected success, got error: {:?}", err),
        }
    };
}

pub const SHADERS_DIR: &str = "shaders";

pub struct TestEnv {
    _event_loop: EventLoop<()>,
    window: Window,
}

impl Default for TestEnv {
    fn default() -> Self {
        let window_attributes = Window::default_attributes()
            .with_title("Test Window")
            .with_inner_size(winit::dpi::LogicalSize::new(1920.0, 1080.0));

        let mut builder = EventLoop::builder();
        #[cfg(target_os = "linux")]
        {
            EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
            EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
        }
        let event_loop = heph_expect_success!(builder.build());
        event_loop.set_control_flow(ControlFlow::Wait);

        #[allow(deprecated)]
        let window = heph_expect_success!(event_loop.create_window(window_attributes));
        Self {
            _event_loop: event_loop,
            window,
        }
    }
}

impl TestEnv {
    pub fn raw_window_handle(&self) -> RawWindowHandle {
        heph_expect_success!(self.window.window_handle()).as_raw()
    }
    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        heph_expect_success!(self.window.display_handle()).as_raw()
    }
}

unsafe impl Send for TestEnv {}
unsafe impl Sync for TestEnv {}
