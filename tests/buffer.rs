mod utils;

use std::process::ExitCode;

use heph_gl::{
    buffer::Buffer,
    renderers::{
        DEFAULT_DEVICE, Renderer, resources::BufferUsage, settings::InitializeOptions,
        vulkan::VulkanRenderer,
    },
};
use libtest_mimic::{Arguments, Trial};

use crate::utils::TestEnv;

fn main() -> ExitCode {
    if std::env::var("NEXTEST").is_err() {
        println!("Skipping integration tests, run via `cargo nextest run` instead.");
        return ExitCode::SUCCESS;
    }

    let args = Arguments::from_args();

    let tests = vec![Trial::test("test_buffer", move || {
        test_buffer();
        Ok(())
    })];

    libtest_mimic::run(&args, tests).exit_code()
}

fn test_buffer() {
    const BUFFER_USAGE: BufferUsage = BufferUsage::Storage;

    let env = TestEnv::default();
    let mut renderer = VulkanRenderer::new();
    let p_renderer = &renderer as *const VulkanRenderer;
    heph_expect_success!(renderer.initialize(&InitializeOptions {
        app_name: "Test Buffers",
        api_version: Some(renderer.min_api_version()),
        window_handle: env.raw_window_handle(),
        display_handle: env.raw_display_handle(),
    }));
    heph_expect_success!(renderer.set_device(DEFAULT_DEVICE, &[]));

    {
        const BUFFER_SIZE: usize = 1;
        let buffer = heph_expect_success!(Buffer::new(&renderer, BUFFER_SIZE, BUFFER_USAGE));
        assert!(std::ptr::addr_eq(buffer.renderer(), p_renderer));
        assert!(buffer.renderer_buffer().is_allocated());
        assert_eq!(buffer.renderer_buffer().size(), BUFFER_SIZE);
        assert_eq!(buffer.renderer_buffer().usage(), BUFFER_USAGE);
    }

    {
        let data = [5u8, 4, 3, 2, 1];
        let mut buffer = heph_expect_success!(Buffer::new(&renderer, data.len(), BUFFER_USAGE));
        heph_expect_success!(buffer.write(2, &data[..3]));

        let mut result = [255u8; 3];
        heph_expect_success!(buffer.read(1, &mut result));
        assert_eq!(result, [0, 5, 4]);
    }
}
