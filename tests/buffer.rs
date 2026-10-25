mod utils;

use std::process::ExitCode;

use heph_gl::{
    Buffer, Error,
    renderers::{
        BufferUsage, DEFAULT_DEVICE, InitializeOptions, Renderer, RendererBuffer,
        ResourceBindingType, vulkan::VulkanRenderer,
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

    {
        const BUFFER_SIZE: usize = 5;
        let renderer_buffer =
            heph_expect_success!(renderer.create_buffer(BUFFER_SIZE, BUFFER_USAGE));
        let renderer_buffer_ptr =
            renderer_buffer.as_ref() as *const dyn RendererBuffer as *const ();
        let buffer = Buffer::from_renderer_buffer(&renderer, renderer_buffer);
        assert!(std::ptr::addr_eq(
            buffer.renderer_buffer(),
            renderer_buffer_ptr
        ));
    }

    {
        let data = [1u8, 2, 3, 4, 5];
        let buffer = heph_expect_success!(Buffer::from_slice(&renderer, &data, BUFFER_USAGE));

        let mut result = [0u8; 5];
        heph_expect_success!(buffer.read(0, &mut result));
        assert_eq!(result, data);

        let binding = buffer.binding(86);
        assert_eq!(binding.binding, 86);
        match binding.resource {
            ResourceBindingType::Buffer {
                handle,
                usage,
                offset,
                size,
            } => {
                assert!(std::ptr::addr_eq(handle, buffer.renderer_buffer()));
                assert_eq!(usage, BUFFER_USAGE);
                assert_eq!(offset, 0);
                assert_eq!(size, data.len());
            }
            _ => panic!("Expected a buffer resource binding."),
        }

        let binding = heph_expect_success!(buffer.partial_binding(37, 1, 3));
        assert_eq!(binding.binding, 37);
        match binding.resource {
            ResourceBindingType::Buffer {
                handle,
                usage,
                offset,
                size,
            } => {
                assert!(std::ptr::addr_eq(handle, buffer.renderer_buffer()));
                assert_eq!(usage, BUFFER_USAGE);
                assert_eq!(offset, 1);
                assert_eq!(size, 3);
            }
            _ => panic!("Expected a buffer resource binding."),
        }

        heph_expect_err!(
            buffer.partial_binding(0, data.len(), 1),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            buffer.partial_binding(0, data.len() + 1, 1),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            buffer.partial_binding(0, 0, data.len() + 1),
            Error::invalid_argument("")
        );
    }
}
