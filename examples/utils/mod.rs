use heph_gl::renderers::*;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

#[cfg(feature = "vulkan")]
pub type ExampleRenderer = vulkan_renderer::VulkanRenderer;

#[cfg(not(any(
    feature = "vulkan",
    feature = "directx",
    feature = "metal",
    feature = "opengl"
)))]
pub type ExampleRenderer = vulkan_renderer::VulkanRenderer;

pub type ExampleFn = fn(&mut ExampleRenderer) -> ();

struct App {
    name: String,
    example: ExampleFn,
    window: Option<Window>,
    renderer: Option<ExampleRenderer>,
}

impl App {
    fn new(name: &str, example: ExampleFn) -> Self {
        Self {
            name: name.to_owned(),
            example,
            window: None,
            renderer: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_attributes = Window::default_attributes()
            .with_title(format!("HephGL - {}", self.name))
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));

        let window = event_loop.create_window(window_attributes).unwrap();
        println!("OS Window created successfully.");
        let mut renderer = ExampleRenderer::new();
        println!("Renderer created successfully.");

        let init_options = heph_gl::renderers::InitializeOptions {
            app_name: &self.name,
            window_handle: window.window_handle().unwrap().as_raw(),
            display_handle: window.display_handle().unwrap().as_raw(),
        };
        renderer.initialize(&init_options).unwrap();

        (self.example)(&mut renderer);

        self.window = Some(window);
        self.renderer = Some(renderer);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if event.physical_key == PhysicalKey::Code(KeyCode::KeyQ) {
                    event_loop.exit();
                }
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            _ => (),
        }
    }
}

pub fn run_example(name: &str, example: ExampleFn) {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::new(name, example);
    let _ = event_loop.run_app(&mut app);
}
