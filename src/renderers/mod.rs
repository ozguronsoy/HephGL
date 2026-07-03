use renkrs::RGB;
use winit::window::Window;

use crate::graphics_device::GraphicsDevice;

pub trait Renderer {
    fn new() -> Self;
    fn initialize(&mut self, app_name: &str, window: &Window);
    fn enumerate_devices(&mut self) -> Vec<GraphicsDevice>;
    fn clear(&mut self, color: RGB<f32>);
}

pub mod vulkan_renderer;
