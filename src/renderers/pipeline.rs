use std::any::Any;

/// Represents a compiled pipeline.
pub trait RendererPipeline: Any + Send + Sync {
    /// Indicates whether the pipeline currently has an allocation.
    fn is_allocated(&self) -> bool;
}

/// Represents a compiled compute pipeline.
pub trait RendererComputePipeline: RendererPipeline {}

/// Represents a compiled graphics pipeline.
pub trait RendererGraphicsPipeline: RendererPipeline {}
