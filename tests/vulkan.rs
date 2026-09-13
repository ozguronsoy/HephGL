use std::process::ExitCode;

use heph_gl::{Version, renderers::vulkan::VulkanRenderer};

use crate::renderer::{RendererTestFlags, RendererTests, RendererVersionedTestSuite};

mod renderer;
mod utils;

fn main() -> ExitCode {
    let flags = RendererTestFlags {
        todo_test_clear: true,
        ..Default::default()
    };
    RendererTests::<VulkanRenderer>::run(&[
        RendererVersionedTestSuite {
            api_version: Some(Version::new(1, 0, 0)),
            flags,
        },
        RendererVersionedTestSuite {
            api_version: Some(Version::new(1, 1, 0)),
            flags,
        },
        RendererVersionedTestSuite {
            api_version: Some(Version::new(1, 2, 0)),
            flags,
        },
        RendererVersionedTestSuite {
            api_version: Some(Version::new(1, 3, 0)),
            flags,
        },
        RendererVersionedTestSuite {
            api_version: Some(Version::new(1, 4, 0)),
            flags,
        },
    ])
}
