use std::process::ExitCode;

use heph_gl::{
    Version,
    renderers::{Renderer, vulkan::VulkanRenderer},
};

use crate::renderer::{RendererTestFlags, RendererTestSettings, RendererTests};

mod renderer;
mod utils;

fn main() -> ExitCode {
    let settings = RendererTestSettings {
        api_versions: vec![
            Some(Version::new(1, 0, 0)),
            Some(Version::new(1, 1, 0)),
            Some(Version::new(1, 2, 0)),
            Some(Version::new(1, 3, 0)),
            Some(Version::new(1, 4, 0)),
            VulkanRenderer::LATEST_API_VERSION,
        ],
    };
    let flags = RendererTestFlags {
        todo_test_clear: true,
        ..Default::default()
    };
    RendererTests::<VulkanRenderer>::run(settings, flags)
}
