use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use heph_gl::renderers::Renderer;
use heph_gl::renderers::vulkan_renderer::VulkanRenderer;

use renkrs::RGB;

pub struct App {
    window: Option<Window>,
    renderer: Option<VulkanRenderer>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            renderer: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_title("HephGL - Window Sandbox")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));

        self.window = Some(event_loop.create_window(window_attributes).unwrap());
        println!("OS Window created successfully.");

        let active_renderer = self.renderer.insert(VulkanRenderer::create());
        active_renderer.initialize();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("Close signal received. Shutting down HephGL.");
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                println!("{:?}", physical_size);
                if let Some(ref mut active_renderer) = self.renderer {
                    active_renderer.clear(RGB::<u8> { r: 0, g: 0, b: 255 });
                }
            }
            _ => (),
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    let _ = event_loop.run_app(&mut app);
}
