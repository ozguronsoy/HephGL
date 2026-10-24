pub mod concurrency;
pub mod resources;
pub mod settings;
mod thread_context;
pub mod version;

pub mod vulkan;

use renkrs::RGB;

use crate::{
    Result, graphics_device::GraphicsDevice, renderers::resources::*, renderers::settings::*,
    shader::Shader,
};

/// Represents an automatic version selection. When used, the renderer will initialize with the
/// latest API version supported by the current hardware.
pub const LATEST_API_VERSION: Option<crate::Version> = None;
/// Represents an automatic device selection. When used, the renderer will set the default graphics
/// device used in the current system.
pub const DEFAULT_DEVICE: Option<&GraphicsDevice> = None;

/// The core interface for a graphics renderer.
pub trait Renderer {
    /// Creates an uninitialized instance of the renderer.
    fn new() -> Self
    where
        Self: Sized;

    /// The minimum supported API version by the backend. Initializing a renderer with an earlier
    /// API version will result in an error.
    fn min_api_version(&self) -> crate::Version;
    /// Gets the latest API version supported by the current hardware.
    fn latest_api_version(&self) -> Result<crate::Version>;
    /// Checks whether the `api_version` is supported by the current hardware.
    fn is_api_version_supported(&self, api_version: crate::Version) -> Result<bool> {
        Ok(api_version >= self.min_api_version() && api_version <= self.latest_api_version()?)
    }

    /// Returns the current settings used by the renderer.
    fn get_settings(&self) -> &Settings;
    /// Updates the settings.
    ///
    /// ### Note
    /// This function can only be called from the main thread.
    fn set_settings(&mut self, settings: Settings) -> Result<()>;

    /// Initializes the renderer using the provided options.
    ///
    /// ### Note
    /// This function can only be called from the main thread.
    fn initialize(&mut self, options: &InitializeOptions) -> Result<()>;
    /// Frees all internal resources and shutdowns the internal API.
    ///
    /// ### Note
    /// This function can only be called from the main thread.
    fn uninitialize(&mut self) -> Result<()>;

    /// Enumerates all available graphics devices on the system.
    fn enumerate_devices(&self) -> Result<Vec<GraphicsDevice>>;
    /// Returns the currently active device, or `None` if there is no active
    /// device.
    fn get_device(&self) -> Option<&GraphicsDevice>;
    /// Sets the active graphics device and initializes it with the requested
    /// features.
    ///
    /// ### Note
    /// This function can only be called from the main thread.
    fn set_device(
        &mut self,
        device: Option<&GraphicsDevice>,
        requested_features: &[FeatureRequest],
    ) -> Result<()>;

    /// Gets a list of MSAA supported by the current device.
    fn supported_msaa_list(&self) -> Result<&Vec<Msaa>>;

    /// Allocates a new buffer on the GPU with the specified size and usage.
    fn create_buffer(&self, size: usize, usage: BufferUsage) -> Result<Box<dyn RendererBuffer>>;
    /// Writes data to the buffer on the GPU.
    fn write_buffer(
        &self,
        buffer: &mut dyn RendererBuffer,
        offset: usize,
        src: &[u8],
    ) -> Result<()>;
    /// Reads data from the buffer on the GPU.
    fn read_buffer(
        &self,
        buffer: &dyn RendererBuffer,
        offset: usize,
        dest: &mut [u8],
    ) -> Result<()>;
    /// Frees the memory allocated for the provided buffer.
    fn destroy_buffer(&self, buffer: &mut dyn RendererBuffer) -> Result<()>;

    /// Creates a texture with the specified dimensions, format, and pixel data.
    fn create_texture(
        &mut self,
        options: &TextureOptions,
        data: &[u8],
    ) -> Result<Box<dyn RendererTexture>>;
    /// Frees the resources allocated for the provided texture.
    fn destroy_texture(&self, texture: &mut dyn RendererTexture) -> Result<()>;

    /// Creates a texture sampler using the provided options.
    fn create_sampler(&self, options: &SamplerOptions) -> Result<Box<dyn RendererSampler>>;
    /// Frees the resources allocated for the provided sampler.
    fn destroy_sampler(&self, sampler: &mut dyn RendererSampler) -> Result<()>;

    /// Creates a compute pipeline using the provided shader.
    fn create_compute_pipeline(&self, shader: &Shader) -> Result<Box<dyn RendererComputePipeline>>;
    /// Destroys the compute pipeline.
    fn destroy_compute_pipeline(&self, pipeline: &mut dyn RendererComputePipeline) -> Result<()>;
    /// Creates a compute workload for the GPU.
    fn record_compute_command(
        &mut self,
        pipeline: &mut dyn RendererComputePipeline,
        binding_sets: &[&[ResourceBinding<'_>]],
        group_count: (u32, u32, u32),
    ) -> Result<()>;

    /// Creates a graphics pipeline using the provided shaders.
    fn create_graphics_pipeline(
        &mut self,
        shaders: &[&Shader],
        options: &GraphicsPipelineOptions,
    ) -> Result<Box<dyn RendererGraphicsPipeline>>;
    /// Destroys the graphics pipeline.
    fn destroy_graphics_pipeline(
        &mut self,
        pipeline: &mut dyn RendererGraphicsPipeline,
    ) -> Result<()>;
    /// Creates a graphics workload for the GPU.
    fn record_graphics_command(
        &mut self,
        pipeline: &mut dyn RendererGraphicsPipeline,
        binding_sets: &[&[ResourceBinding<'_>]],
        draw_count: u32,
        instance_count: u32,
    ) -> Result<()>;

    /// Begins a new frame. `end_frame` must be called when the frame is done.
    fn begin_frame(&mut self) -> Result<()>;
    /// Ends the frame. `begin_frame` must be called before calling this method.
    fn end_frame(&mut self) -> Result<()>;

    /// Blocks the current CPU thread until the GPU has finished executing all
    /// pending commands.
    ///
    /// ### Note
    /// This function can only be called from the main thread.
    fn wait_idle(&self) -> Result<()>;

    /// Clears the current render target with the specified color.
    fn clear(&mut self, color: RGB<f32>) -> Result<()>;
}

/// Gets the maximum number of threads that can execute concurrently.
pub const fn max_concurrent_threads() -> usize {
    crate::renderers::thread_context::thread_context_count()
}
