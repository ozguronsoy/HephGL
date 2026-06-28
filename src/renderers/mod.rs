pub trait Renderer {
    fn create() -> Self;
    fn initialize(&mut self);
    fn clear(&mut self, color: renkrs::RGB<u8>);
}

pub mod vulkan_renderer;
