use crate::graphics_device::GraphicsDeviceFeature;

/// Defines the multisample anti-aliasing sample count used when rendering.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Msaa {
    /// Disables MSAA and uses a single sample per pixel.
    #[default]
    X1,
    /// Uses 2 samples per pixel.
    X2,
    /// Uses 4 samples per pixel.
    X4,
    /// Uses 8 samples per pixel.
    X8,
    /// Uses 16 samples per pixel.
    X16,
    /// Uses 32 samples per pixel.
    X32,
    /// Uses 64 samples per pixel.
    X64,
}

/// Defines the HDR output modes used when presenting.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum HdrMode {
    /// Disables HDR and presents in the sRGB color space.
    #[default]
    Disabled,
    /// Presents 10-bit color in the Rec.2020 color space, encoded with the PQ (ST.2084) transfer
    /// function.
    Hdr10,
    /// Presents 16-bit floating-point linear color with Rec.709 primaries, where 1.0 corresponds
    /// to 80 nits and values outside the 0.0 to 1.0 range are allowed.
    Scrgb,
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
    /// Indicates whether the debugging is enabled.
    pub debug: bool,
}

/// Represents the settings used throughout the lifetime of the renderer.
#[derive(Clone, Copy)]
pub struct Settings {
    /// The maximum number of frames that can be processed concurrently by the
    /// CPU and GPU.
    pub frames_in_flight: usize,
    /// Indicates whether the vsync is enabled.
    pub vsync: bool,
    /// The maximum number of frames rendered per second.
    pub max_fps: Option<u32>,
    /// Indicates whether stereoscopic 3D rendering is enabled.
    pub stereoscopic_3d_rendering: bool,
    /// The size that will be used as fallback when the renderer fails to fetch the target window's
    /// size.
    pub default_size: (u32, u32),
    /// The multisample anti-aliasing sample count.
    pub msaa: Msaa,
    /// The HDR output mode.
    ///
    /// ### Note
    /// If the mode is not supported by the current device and window, the renderer falls back to
    /// [`HdrMode::Disabled`].
    pub hdr_mode: HdrMode,
}

/// Represents a request for a specific graphics feature, indicating whether it
/// is strictly required.
#[derive(Clone, Copy)]
pub struct FeatureRequest {
    /// The feature that is being requested.
    pub feature: GraphicsDeviceFeature,
    /// Indicates whether the graphics device must support the requested
    /// feature.
    pub required: bool,
}

impl FeatureRequest {
    /// Creates a new instance.
    pub fn new(feature: GraphicsDeviceFeature, required: bool) -> Self {
        Self { feature, required }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            frames_in_flight: 1,
            vsync: false,
            max_fps: None,
            stereoscopic_3d_rendering: false,
            default_size: (1920, 1080),
            msaa: Msaa::X1,
            hdr_mode: HdrMode::Disabled,
        }
    }
}
impl Settings {
    /// Gets the number of array layers the render targets need.
    pub(super) fn array_layer_count(&self) -> u32 {
        match self.stereoscopic_3d_rendering {
            true => 2,
            false => 1,
        }
    }
}
