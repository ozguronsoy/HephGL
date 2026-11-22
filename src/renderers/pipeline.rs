use crate::renderers::debug::RendererDebugObject;

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
