use crate::renderers::{
    DISPLAY_TEXTURE_FORMAT, debug::RendererDebugObject, resources::TextureFormat,
};

/// Defines the color blending modes supported by the renderers.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum ColorBlending {
    /// Color blending is disabled.
    #[default]
    Disabled,
    /// Uses standard alpha blending, combining the source and destination colors based on the
    /// source alpha.
    Alpha,
}

/// Defines how vertices are assembled into primitives.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
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
    /// Every `control_points` vertices represent a patch, which is subdivided by the tessellation
    /// shaders.
    PatchList {
        /// The number of control points in each patch.
        control_points: u32,
    },
}

/// Defines which triangle faces are discarded during rasterization.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum CullingMode {
    /// No faces are culled.
    #[default]
    Disabled,
    /// Front-facing triangles are culled.
    Front,
    /// Back-facing triangles are culled.
    Back,
}

/// Defines how polygons are rasterized.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum PolygonMode {
    /// Polygons are filled.
    #[default]
    Fill,
    /// Only the polygon edges are drawn as lines.
    Line,
    /// Only the polygon vertices are drawn as points.
    Point,
}

/// Defines which vertex winding order is considered front-facing.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum FrontFace {
    /// Vertices ordered clockwise are considered front-facing.
    Clockwise,
    /// Vertices ordered counter-clockwise are considered front-facing.
    #[default]
    CounterClockwise,
}

/// Defines the offset that is added to the depth value of fragments.
#[derive(Clone, Copy)]
pub struct DepthBias {
    /// The constant offset, in units of the smallest depth difference the depth buffer can
    /// resolve.
    pub constant: f32,
    /// The offset that is scaled by the depth slope of the polygon.
    pub slope: f32,
    /// The largest offset that can be added, or the smallest one if the value is negative.
    pub clamp: Option<f32>,
}

/// Provides the capabilities of a graphics pipeline.
#[derive(Clone)]
pub struct GraphicsPipelineOptions {
    /// The color blending mode.
    pub blending: ColorBlending,
    /// The primitive topology used to assemble vertices.
    pub topology: PrimitiveTopology,
    /// The face culling mode.
    pub culling: CullingMode,
    /// The polygon rasterization mode.
    pub polygon_mode: PolygonMode,
    /// The vertex winding order considered front-facing.
    pub front_face: FrontFace,
    /// Indicates whether depth testing is enabled.
    pub depth_test: bool,
    /// Indicates whether fragments write their depth value to the depth buffer.
    pub depth_write: bool,
    /// Indicates whether fragment depth values are clamped to the depth range instead of clipping
    /// primitives against the near and far planes.
    pub depth_clamp: bool,
    /// Indicates whether the pipeline renders into depth-only canvases.
    pub depth_only: bool,
    /// The offset added to the depth value of fragments.
    pub depth_bias: Option<DepthBias>,
    /// The minimum fraction of samples in each pixel that are shaded individually. The value must
    /// be between `0.0` and `1.0`.
    pub min_sample_shading: Option<f32>,
    /// The formats of the textures the pipeline renders into.
    pub texture_formats: Vec<Option<TextureFormat>>,
}

/// Represents a compiled pipeline.
pub trait RendererPipeline: RendererDebugObject {
    /// Indicates whether the pipeline currently has an allocation.
    fn is_allocated(&self) -> bool;
}

/// Represents a compiled compute pipeline.
pub trait RendererComputePipeline: RendererPipeline {
    /// Creates a shallow copy of the pipeline.
    fn clone_pipeline(&self) -> Box<dyn RendererComputePipeline>;
}

/// Represents a compiled graphics pipeline.
pub trait RendererGraphicsPipeline: RendererPipeline {
    /// Creates a shallow copy of the pipeline.
    fn clone_pipeline(&self) -> Box<dyn RendererGraphicsPipeline>;
}

impl Clone for Box<dyn RendererGraphicsPipeline> {
    fn clone(&self) -> Self {
        self.clone_pipeline()
    }
}

impl Clone for Box<dyn RendererComputePipeline> {
    fn clone(&self) -> Self {
        self.clone_pipeline()
    }
}

impl Default for GraphicsPipelineOptions {
    fn default() -> Self {
        Self {
            blending: ColorBlending::default(),
            topology: PrimitiveTopology::default(),
            polygon_mode: PolygonMode::default(),
            culling: CullingMode::default(),
            front_face: FrontFace::default(),
            depth_test: true,
            depth_write: true,
            depth_clamp: false,
            depth_only: false,
            depth_bias: None,
            min_sample_shading: None,
            texture_formats: vec![DISPLAY_TEXTURE_FORMAT],
        }
    }
}
