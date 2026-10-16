pub mod buffer;
pub mod compute;
pub mod concurrency;
pub mod frame;
pub mod render;
pub mod setup;

use std::sync::{LazyLock, Mutex};

use heph_gl::renderers::{
    Renderer,
    concurrency::{RendererHandle, RendererWorkerFactory},
    settings::{FeatureRequest, InitializeOptions},
};

use crate::{heph_expect_success, renderer::RendererTests, utils::TestEnv};

#[macro_export]
macro_rules! dummy_window_handle {
    () => {
        raw_window_handle::RawWindowHandle::Orbital(raw_window_handle::OrbitalWindowHandle::new(
            std::ptr::NonNull::<std::ffi::c_void>::dangling(),
        ))
    };
}
#[macro_export]
macro_rules! dummy_display_handle {
    () => {
        raw_window_handle::RawDisplayHandle::Orbital(raw_window_handle::OrbitalDisplayHandle::new())
    };
}

pub(super) static TEST_ENV: LazyLock<Mutex<TestEnv>> =
    LazyLock::new(|| Mutex::new(TestEnv::default()));
#[macro_export]
macro_rules! test_env {
    () => {
        heph_expect_success!(TEST_ENV.lock())
    };
}

impl<TestRenderer: Renderer> RendererTests<TestRenderer>
where
    RendererHandle<TestRenderer>: RendererWorkerFactory<TestRenderer>,
{
    pub(super) fn create_renderer(&self) -> TestRenderer {
        let test_env = test_env!();
        let app_name = format!("{} Tests", std::any::type_name::<TestRenderer>());
        let mut renderer = TestRenderer::new();
        heph_expect_success!(renderer.initialize(&InitializeOptions {
            app_name: app_name.as_str(),
            window_handle: test_env.raw_window_handle(),
            display_handle: test_env.raw_display_handle(),
            api_version: self.api_version,
        }));
        renderer
    }

    pub(super) fn create_renderer_with_target_device(
        &self,
        target_device_type: heph_gl::graphics_device::Type,
        requested_features: &[FeatureRequest],
        target_device_type_required: bool,
    ) -> Option<TestRenderer> {
        let mut renderer = self.create_renderer();
        let devices = heph_expect_success!(renderer.enumerate_devices());
        let target_device = devices.iter().find(|d| d.device_type == target_device_type);
        if target_device.is_none() {
            if target_device_type_required {
                panic!("Invalid Test Env: No {} found.", target_device_type);
            } else {
                return None;
            }
        }
        heph_expect_success!(renderer.set_device(target_device, requested_features));

        let result = renderer.get_device();
        assert!(
            result.is_some(),
            "Renderer successfully set the graphics device, but `get_device()` returned `None`."
        );
        assert_eq!(
            heph_expect_success!(target_device.ok_or("Failed to find the target device.")),
            heph_expect_success!(result.ok_or("Failed to get renderer device."))
        );

        Some(renderer)
    }

    pub(super) fn create_renderer_with_any_device(
        &self,
        requested_features: &[FeatureRequest],
    ) -> TestRenderer {
        let mut renderer = self.create_renderer();
        let devices = heph_expect_success!(renderer.enumerate_devices());
        assert!(!devices.is_empty(), "Invalid Test Env: No device found.",);
        heph_expect_success!(renderer.set_device(None, requested_features));

        let result = renderer.get_device();
        assert!(
            result.is_some(),
            "Renderer successfully set the graphics device, but `get_device()` returned `None`."
        );
        assert_eq!(
            heph_expect_success!(result.ok_or("Failed to get renderer device.")),
            heph_expect_success!(devices.first().ok_or("No device found."))
        );

        renderer
    }
}
