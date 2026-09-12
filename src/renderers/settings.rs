/// Represents the settings used throughout the lifetime of the renderer.
#[derive(Debug, Clone, Copy)]
pub struct Settings {
    /// The maximum number of frames that can be processed concurrently by the
    /// CPU and GPU.
    pub frames_in_flight: u32,
    /// Indicates whether the vsync is enabled.
    pub vsync: bool,
    /// Indicates whether stereoscopic 3D rendering is enabled.
    pub stereoscopic_3d_rendering: bool,
    /// The size that will be used as fallback when the renderer fails to fetch the target window's
    /// size.
    pub default_size: (u32, u32),
}

/// Represents the options used while initializing the renderer.
pub struct InitializeOptions<'a> {
    /// The name of the application using the renderer.
    pub app_name: &'a str,
    /// Handle to the native window.
    pub window_handle: raw_window_handle::RawWindowHandle,
    /// Handle to the display device.
    pub display_handle: raw_window_handle::RawDisplayHandle,
    /// The underlying backend's API version. Set this to `None` for using the latest available
    /// version.
    pub api_version: Option<crate::Version>,
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
            vsync: false,
            stereoscopic_3d_rendering: false,
            default_size: (1920, 1080),
        }
    }
}
