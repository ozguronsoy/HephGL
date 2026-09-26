use std::process::ExitCode;

use heph_gl::{Version, renderers::vulkan::VulkanRenderer};

use crate::renderer::{RendererTestFlags, RendererTests, RendererVersionedTestSuite};

mod renderer;
mod utils;

fn main() -> ExitCode {
    let vulkan_13_flags = RendererTestFlags {
        todo_test_clear: true,
        ..Default::default()
    };
    let mut vulkan_10_flags = vulkan_13_flags;
    vulkan_10_flags.skip_all_tests = true;
    RendererTests::<VulkanRenderer>::run(&[
        RendererVersionedTestSuite {
            api_version: Some(Version::new(1, 0, 0)),
            flags: vulkan_10_flags,
        },
        RendererVersionedTestSuite {
            api_version: Some(Version::new(1, 1, 0)),
            flags: vulkan_10_flags,
        },
        RendererVersionedTestSuite {
            api_version: Some(Version::new(1, 2, 0)),
            flags: vulkan_10_flags,
        },
        RendererVersionedTestSuite {
            api_version: Some(Version::new(1, 3, 0)),
            flags: vulkan_13_flags,
        },
        RendererVersionedTestSuite {
            api_version: Some(Version::new(1, 4, 0)),
            flags: vulkan_13_flags,
        },
    ])
}
