use crate::renderers::{
    DISPLAY_TEXTURE_FORMAT, TextureFormat, debug::RendererDebugObject, resources::RendererTexture,
};

/// Defines the properties of a canvas.
#[derive(Clone)]
pub struct CanvasOptions {
    /// The width of the canvas in pixels.
    pub width: u32,
    /// The height of the canvas in pixels.
    pub height: u32,
    /// Indicates whether the canvas has only a depth buffer. The texture of a depth-only canvas
    /// contains the depth values, and only depth-only pipelines can render into it.
    pub depth_only: bool,
    /// The formats of the textures the canvas renders into. Ignored for depth-only canvases.
    pub texture_formats: Vec<Option<TextureFormat>>,
}

/// Represents an off-screen render target.
pub trait RendererCanvas: RendererDebugObject {
    /// Indicates whether the canvas currently has an allocation.
    fn is_allocated(&self) -> bool;
    /// Returns the canvas width in pixels.
    fn width(&self) -> u32 {
        self.options().width
    }
    /// Returns the canvas height in pixels.
    fn height(&self) -> u32 {
        self.options().height
    }
    /// Checks whether the canvas is depth-only.
    fn depth_only(&self) -> bool {
        self.options().depth_only
    }
    /// Returns the formats of the textures the canvas renders into.
    fn texture_formats(&self) -> &[Option<TextureFormat>] {
        &self.options().texture_formats
    }
    /// Returns the options the canvas was created with.
    fn options(&self) -> &CanvasOptions;
    /// Returns the textures the canvas renders into.
    fn textures(&self) -> Box<dyn Iterator<Item = &dyn RendererTexture> + '_>;
}

impl Default for CanvasOptions {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            depth_only: false,
            texture_formats: vec![DISPLAY_TEXTURE_FORMAT],
        }
    }
}
