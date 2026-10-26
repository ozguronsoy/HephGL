use crate::{
    Result,
    renderers::{Renderer, RendererSampler, ResourceBinding, ResourceBindingType, SamplerOptions},
};

/// An automatically managed sampler created by a renderer.
pub struct Sampler<'a> {
    renderer: &'a dyn Renderer,
    renderer_sampler: Box<dyn RendererSampler>,
}

impl<'a> Sampler<'a> {
    /// Creates a renderer sampler with the provided options.
    pub fn new(renderer: &'a dyn Renderer, options: &SamplerOptions) -> Result<Self> {
        Ok(Self {
            renderer,
            renderer_sampler: renderer.create_sampler(options)?,
        })
    }

    /// Wraps an existing sampler.
    pub fn from_renderer_sampler(
        renderer: &'a dyn Renderer,
        renderer_sampler: Box<dyn RendererSampler>,
    ) -> Self {
        Self {
            renderer,
            renderer_sampler,
        }
    }

    /// Returns the renderer that owns this sampler.
    pub fn renderer(&self) -> &'a dyn Renderer {
        self.renderer
    }

    /// Returns the underlying renderer sampler.
    pub fn renderer_sampler(&self) -> &dyn RendererSampler {
        self.renderer_sampler.as_ref()
    }

    /// Returns the underlying renderer sampler mutably.
    pub fn renderer_sampler_mut(&mut self) -> &mut dyn RendererSampler {
        self.renderer_sampler.as_mut()
    }

    /// Creates a resource binding for the renderer sampler.
    pub fn binding(&self, binding: u32) -> ResourceBinding<'_> {
        ResourceBinding {
            binding,
            resource: ResourceBindingType::Sampler {
                handle: self.renderer_sampler.as_ref(),
            },
        }
    }
}

impl<'a> Drop for Sampler<'a> {
    fn drop(&mut self) {
        if self.renderer_sampler.is_allocated() {
            self.renderer
                .destroy_sampler(self.renderer_sampler.as_mut())
                .unwrap();
        }
    }
}
