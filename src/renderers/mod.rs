use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

use crate::graphics_device::GraphicsDevice;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FeatureRequest {
    pub feature: crate::graphics_device::Feature,
    pub required: bool,
}

pub struct InitializeOptions<'a> {
    pub app_name: &'a str,
    pub window_handle: RawWindowHandle,
    pub display_handle: RawDisplayHandle,
}

pub trait Renderer {
    fn new() -> Self;

    fn initialize(&mut self, options: &InitializeOptions);
    fn uninitialize(&mut self);

    fn enumerate_devices(&mut self) -> Vec<GraphicsDevice>;
    fn get_device(&self) -> &Option<GraphicsDevice>;
    fn set_device(&mut self, device: &GraphicsDevice);

    fn clear(&mut self, color: RGB<f32>);
}

pub mod vulkan_renderer;
