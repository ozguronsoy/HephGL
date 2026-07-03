use heph_gl::renderers::Renderer;
use heph_gl::renderers::vulkan_renderer::VulkanRenderer;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use renkrs::RGB;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

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

        let active_window = self
            .window
            .insert(event_loop.create_window(window_attributes).unwrap());
        println!("OS Window created successfully.");

        let active_renderer = self.renderer.insert(VulkanRenderer::new());
        let display_handle = active_window.display_handle().unwrap().as_raw();
        let window_handle = active_window.window_handle().unwrap().as_raw();
        active_renderer.initialize("Sandbox", window_handle, display_handle);
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
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed
                    && let Some(ref mut active_renderer) = self.renderer
                {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::KeyE) => {
                            let graphics_devices = active_renderer.enumerate_devices();
                            println!("Graphics device count: {}", graphics_devices.len());
                            for dev in &graphics_devices {
                                println!("{}", dev);
                            }
                        }
                        PhysicalKey::Code(KeyCode::KeyR) => {
                            active_renderer.clear(RGB::<f32> {
                                r: 1.0,
                                g: 0.0,
                                b: 0.0,
                            });
                        }
                        PhysicalKey::Code(KeyCode::KeyG) => {
                            active_renderer.clear(RGB::<f32> {
                                r: 0.0,
                                g: 1.0,
                                b: 0.0,
                            });
                        }
                        PhysicalKey::Code(KeyCode::KeyB) => {
                            active_renderer.clear(RGB::<f32> {
                                r: 0.0,
                                g: 0.0,
                                b: 1.0,
                            });
                        }
                        _ => (),
                    }
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
