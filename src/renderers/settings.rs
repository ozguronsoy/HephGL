/// Defines the multisample anti-aliasing sample count used when rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Msaa {
    /// Disables MSAA and uses a single sample per pixel.
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

/// Represents the settings used throughout the lifetime of the renderer.
#[derive(Clone, Copy)]
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
    /// The multisample anti-aliasing sample count.
    pub msaa: Msaa,
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
#[derive(Clone, Copy)]
pub struct FeatureRequest {
    /// The feature that is being requested.
    pub feature: crate::graphics_device::Feature,
    /// Indicates whether the graphics device must support the requested
    /// feature.
    pub required: bool,
}

/// Defines the color blending modes supported by the renderers.
#[derive(Default, Clone, Copy)]
pub enum ColorBlending {
    /// Color blending is disabled.
    #[default]
    Disabled,
    /// Uses standard alpha blending, combining the source and destination colors based on the
    /// source alpha.
    Alpha,
}

/// Defines how vertices are assembled into primitives.
#[derive(Default, Clone, Copy)]
pub enum PrimitiveTopology {
    /// Each vertex represents an individual point.
    PointList,
    /// Every pair of vertices represents an independent line.
    LineList,
    /// Consecutive vertices form a connected sequence of lines.
    LineStrip,
    /// Every three vertices represent an independent triangle.
    #[default]
    TriangleList,
    /// Consecutive vertices form a connected sequence of triangles.
    TriangleStrip,
}

/// Defines which triangle faces are discarded during rasterization.
#[derive(Default, Clone, Copy)]
pub enum CullingMode {
    /// No faces are culled.
    #[default]
    Disabled,
    /// Front-facing triangles are culled.
    Front,
    /// Back-facing triangles are culled.
    Back,
}

/// Defines which vertex winding order is considered front-facing.
#[derive(Default, Clone, Copy)]
pub enum FrontFace {
    /// Vertices ordered clockwise are considered front-facing.
    Clockwise,
    /// Vertices ordered counter-clockwise are considered front-facing.
    #[default]
    CounterClockwise,
}

/// Provides the capabilities of a graphics pipeline.
#[derive(Clone, Copy)]
pub struct GraphicsPipelineOptions {
    /// The color blending mode.
    pub blending: ColorBlending,
    /// The primitive topology used to assemble vertices.
    pub topology: PrimitiveTopology,
    /// The face culling mode.
    pub culling: CullingMode,
    /// The vertex winding order considered front-facing.
    pub front_face: FrontFace,
    /// Indicates whether depth testing is enabled.
    pub depth_test: bool,
    /// Indicates whether fragments write their depth value to the depth buffer.
    pub depth_write: bool,
}

impl Default for GraphicsPipelineOptions {
    fn default() -> Self {
        Self {
            blending: ColorBlending::default(),
            topology: PrimitiveTopology::default(),
            culling: CullingMode::default(),
            front_face: FrontFace::default(),
            depth_test: true,
            depth_write: true,
        }
    }
}

impl FeatureRequest {
    /// Creates a new instance.
    pub fn new(feature: crate::graphics_device::Feature, required: bool) -> Self {
        Self { feature, required }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            frames_in_flight: 1,
            vsync: false,
            stereoscopic_3d_rendering: false,
            default_size: (1920, 1080),
            msaa: Msaa::X1,
        }
    }
}
