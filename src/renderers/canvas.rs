use std::any::Any;

use crate::renderers::resources::RendererTexture;

/// Represents an off-screen render target.
pub trait RendererCanvas: Any + Send + Sync {
    /// Returns the canvas width in pixels.
    fn width(&self) -> u32;
    /// Returns the canvas height in pixels.
    fn height(&self) -> u32;
    /// Returns the texture that contains the rendered image.
    fn texture(&self) -> &dyn RendererTexture;
}
