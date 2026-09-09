/// Represents the settings used throughout the lifetime of the renderer.
#[derive(Debug, Clone, Copy)]
pub struct Settings {
    /// The maximum number of frames that can be processed concurrently by the
    /// CPU and GPU.
    pub frames_in_flight: u32,
}

/// Represents the options used while initializing the renderer.
pub struct InitializeOptions<'a> {
    /// The name of the application using the renderer.
    pub app_name: &'a str,
    /// Handle to the native window.
    pub window_handle: raw_window_handle::RawWindowHandle,
    /// Handle to the display device.
    pub display_handle: raw_window_handle::RawDisplayHandle,
}

/// Represents a request for a specific graphics feature, indicating whether it
/// is strictly required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FeatureRequest {
    /// The feature that is being requested.
    pub feature: crate::graphics_device::Feature,
    /// Indicates whether the graphics device must support the requested
    /// feature.
    pub required: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            frames_in_flight: 1,
        }
    }
}
