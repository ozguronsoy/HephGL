use std::any::Any;

use crate::renderers::{resources::RendererTexture, settings::CanvasOptions};

/// Represents an off-screen render target.
pub trait RendererCanvas: Any + Send + Sync {
    /// Returns the canvas width in pixels.
    fn width(&self) -> u32;
    /// Returns the canvas height in pixels.
    fn height(&self) -> u32;
    /// Returns the options the canvas was created with.
    fn options(&self) -> &CanvasOptions;
    /// Returns the texture that contains the rendered image.
    fn texture(&self) -> &dyn RendererTexture;
}
