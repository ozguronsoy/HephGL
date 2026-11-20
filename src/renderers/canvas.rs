use crate::renderers::{
    CanvasOptions, TextureFormat, debug::RendererDebugObject, resources::RendererTexture,
};

/// Represents an off-screen render target.
pub trait RendererCanvas: RendererDebugObject {
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
