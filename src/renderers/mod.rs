use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

use crate::graphics_device::GraphicsDevice;

pub trait Renderer {
    fn new() -> Self;
    fn initialize(&mut self, app_name: &str, window: RawWindowHandle, display: RawDisplayHandle);
    fn uninitialize(&mut self);
    fn enumerate_devices(&mut self) -> Vec<GraphicsDevice>;
    fn clear(&mut self, color: RGB<f32>);
}

pub mod vulkan_renderer;
