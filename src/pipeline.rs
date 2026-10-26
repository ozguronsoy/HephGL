use crate::{
    Result, Shader,
    renderers::{
        GraphicsPipelineOptions, Renderer, RendererComputePipeline, RendererGraphicsPipeline,
    },
};

/// An automatically managed compute pipeline created by a renderer.
pub struct ComputePipeline<'a> {
    renderer: &'a dyn Renderer,
    renderer_pipeline: Box<dyn RendererComputePipeline>,
}

/// An automatically managed graphics pipeline created by a renderer.
pub struct GraphicsPipeline<'a> {
    renderer: &'a dyn Renderer,
    renderer_pipeline: Box<dyn RendererGraphicsPipeline>,
}

impl<'a> GraphicsPipeline<'a> {
    /// Creates a renderer graphics pipeline.
    pub fn new(
        renderer: &'a dyn Renderer,
        shaders: &[&Shader],
        options: &GraphicsPipelineOptions,
    ) -> Result<Self> {
        let renderer_pipeline = renderer.create_graphics_pipeline(shaders, options)?;
        Ok(Self {
            renderer,
            renderer_pipeline,
        })
    }

    /// Wraps an existing graphics pipeline.
    pub fn from_renderer_graphics_pipeline(
        renderer: &'a dyn Renderer,
        renderer_pipeline: Box<dyn RendererGraphicsPipeline>,
    ) -> Self {
        Self {
            renderer,
            renderer_pipeline,
        }
    }

    /// Returns the renderer that owns this graphics pipeline.
    pub fn renderer(&self) -> &'a dyn Renderer {
        self.renderer
    }

    /// Returns the underlying renderer graphics pipeline.
    pub fn renderer_graphics_pipeline(&self) -> &dyn RendererGraphicsPipeline {
        self.renderer_pipeline.as_ref()
    }

    /// Returns the underlying renderer graphics pipeline mutably.
    pub fn renderer_graphics_pipeline_mut(&mut self) -> &mut dyn RendererGraphicsPipeline {
        self.renderer_pipeline.as_mut()
    }
}
impl<'a> Drop for GraphicsPipeline<'a> {
    fn drop(&mut self) {
        if self.renderer_pipeline.is_allocated() {
            self.renderer
                .destroy_graphics_pipeline(self.renderer_pipeline.as_mut())
                .unwrap();
        }
    }
}

impl<'a> ComputePipeline<'a> {
    /// Creates a renderer compute pipeline.
    pub fn new(renderer: &'a dyn Renderer, shader: &Shader) -> Result<Self> {
        let renderer_pipeline = renderer.create_compute_pipeline(shader)?;
        Ok(Self {
            renderer,
            renderer_pipeline,
        })
    }

    /// Wraps an existing compute pipeline.
    pub fn from_renderer_compute_pipeline(
        renderer: &'a dyn Renderer,
        renderer_pipeline: Box<dyn RendererComputePipeline>,
    ) -> Self {
        Self {
            renderer,
            renderer_pipeline,
        }
    }

    /// Returns the renderer that owns this compute pipeline.
    pub fn renderer(&self) -> &'a dyn Renderer {
        self.renderer
    }

    /// Returns the underlying renderer compute pipeline.
    pub fn renderer_compute_pipeline(&self) -> &dyn RendererComputePipeline {
        self.renderer_pipeline.as_ref()
    }

    /// Returns the underlying renderer compute pipeline mutably.
    pub fn renderer_compute_pipeline_mut(&mut self) -> &mut dyn RendererComputePipeline {
        self.renderer_pipeline.as_mut()
    }
}
impl<'a> Drop for ComputePipeline<'a> {
    fn drop(&mut self) {
        if self.renderer_pipeline.is_allocated() {
            self.renderer
                .destroy_compute_pipeline(self.renderer_pipeline.as_mut())
                .unwrap();
        }
    }
}
