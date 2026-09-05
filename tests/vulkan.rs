use heph_gl::renderers::vulkan_renderer::VulkanRenderer;

use crate::renderer::RendererTests;

mod renderer;
mod utils;

fn main() {
    RendererTests::<VulkanRenderer>::run();
}
