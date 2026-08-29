use heph_gl::renderers::{Renderer, vulkan_renderer::VulkanRenderer};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

const APP_NAME: &str = "Enumerate Devices";

#[cfg(feature = "vulkan")]
type ExampleRenderer = VulkanRenderer;

#[cfg(not(any(
    feature = "vulkan",
    feature = "directx",
    feature = "metal",
    feature = "opengl"
)))]
pub type ExampleRenderer = VulkanRenderer;

#[derive(Default)]
pub struct App {
    window: Option<Window>,
    renderer: Option<ExampleRenderer>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_title(format!("HephGL - {}", APP_NAME))
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));

        let window = event_loop.create_window(window_attributes).unwrap();
        println!("OS Window created successfully.");
        let mut renderer = ExampleRenderer::new();
        println!("Renderer created successfully.");

        let init_options = heph_gl::renderers::InitializeOptions {
            app_name: APP_NAME,
            window_handle: window.window_handle().unwrap().as_raw(),
            display_handle: window.display_handle().unwrap().as_raw(),
        };
        renderer.initialize(&init_options).unwrap();

        let devices = renderer.enumerate_devices().unwrap();
        for device in &devices {
            println!("{}", device);
        }

        self.window = Some(window);
        self.renderer = Some(renderer);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        #[allow(clippy::single_match)]
        match event {
            WindowEvent::CloseRequested => {
                println!("Close signal received. Shutting down HephGL.");
                event_loop.exit();
            }
            _ => (),
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    let _ = event_loop.run_app(&mut app);
}
