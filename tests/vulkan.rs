use std::process::ExitCode;

use heph_gl::renderers::vulkan_renderer::VulkanRenderer;

use crate::renderer::{RendererTestFlags, RendererTests};

mod renderer;
mod utils;

fn main() -> ExitCode {
    RendererTests::<VulkanRenderer>::run(RendererTestFlags {
        todo_test_clear: true,
        ..Default::default()
    })
}
