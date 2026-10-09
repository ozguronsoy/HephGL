pub mod concurrency;
pub mod error;
pub mod resources;
pub mod settings;
mod thread_context;
pub mod version;

pub mod vulkan;

use renkrs::RGB;

use crate::{
    graphics_device::GraphicsDevice, renderers::error::*, renderers::resources::*,
    renderers::settings::*, shader::Shader,
};

// Represents the result of a renderer operation.
type RendererResult<T> = Result<T, RendererError>;

/// The core interface for a graphics renderer.
pub trait Renderer {
    /// Represents a block of memory on the GPU.
    type Buffer: GpuBuffer;
    /// Represents a texture allocated on the GPU.
    type Texture: GpuTexture;
    /// Represents a texture sampler.
    type Sampler: Copy + Clone + Send + Sync;
    /// Represents a compiled graphics pipeline.
    type GraphicsPipeline: Clone + Send + Sync;
    /// Represents a compiled compute pipeline.
    type ComputePipeline: Clone + Send + Sync;

    /// The minimum supported API version by the backend. Initializing a renderer with an earlier
    /// API version will result in an error.
    const MIN_SUPPORTED_API_VERSION: crate::Version;
    /// Represents an automatic version selection. When used, the renderer will initialize with the
    /// latest API version supported by the current hardware.
    const LATEST_API_VERSION: Option<crate::Version> = None;
    /// Represents an automatic device selection. When used, the renderer will set the default
    /// graphics device used in the current system.
    const DEFAULT_DEVICE: Option<&GraphicsDevice> = None;

    /// Creates an uninitialized instance of the renderer.
    fn new() -> Self;

    /// Gets the latest API version supported by the current hardware.
    fn latest_api_version(&self) -> RendererResult<crate::Version>;
    /// Checks whether the `api_version` is supported by the current hardware.
    fn is_api_version_supported(&self, api_version: crate::Version) -> RendererResult<bool> {
        Ok(api_version >= Self::MIN_SUPPORTED_API_VERSION
            && api_version <= self.latest_api_version()?)
    }

    /// Returns the current settings used by the renderer.
    fn get_settings(&self) -> &Settings;
    /// Updates the settings.
    ///
    /// ### Note
    /// This function can only be called from the main thread.
    fn set_settings(&mut self, settings: Settings) -> RendererResult<()>;

    /// Initializes the renderer using the provided options.
    ///
    /// ### Note
    /// This function can only be called from the main thread.
    fn initialize(&mut self, options: &InitializeOptions) -> RendererResult<()>;
    /// Frees all internal resources and shutdowns the internal API.
    ///
    /// ### Note
    /// This function can only be called from the main thread.
    fn uninitialize(&mut self) -> RendererResult<()>;

    /// Enumerates all available graphics devices on the system.
    fn enumerate_devices(&self) -> RendererResult<Vec<GraphicsDevice>>;
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
    ) -> RendererResult<()>;

    /// Gets a list of MSAA supported by the current device.
    fn supported_msaa_list(&self) -> RendererResult<&Vec<Msaa>>;

    /// Allocates a new buffer on the GPU with the specified size and usage.
    fn create_buffer(&self, size: usize, usage: BufferUsage) -> RendererResult<Self::Buffer>;
    /// Writes data to the buffer on the GPU.
    fn write_buffer(&self, buffer: &Self::Buffer, data: &[u8]) -> RendererResult<()>;
    /// Reads data from the buffer on the GPU.
    fn read_buffer(&self, buffer: &Self::Buffer, dest: &mut [u8]) -> RendererResult<()>;
    /// Frees the memory allocated for the provided buffer.
    fn destroy_buffer(&self, buffer: &mut Self::Buffer) -> RendererResult<()>;

    /// Creates a texture with the specified dimensions, format, and pixel data.
    fn create_texture(
        &mut self,
        width: u32,
        height: u32,
        format: TextureFormat,
        data: &[u8],
    ) -> RendererResult<Self::Texture>;
    /// Frees the resources allocated for the provided texture.
    fn destroy_texture(&self, texture: &mut Self::Texture) -> RendererResult<()>;

    /// Creates a texture sampler using the provided options.
    fn create_sampler(&self, options: &SamplerOptions) -> RendererResult<Self::Sampler>;
    /// Frees the resources allocated for the provided sampler.
    fn destroy_sampler(&self, sampler: &mut Self::Sampler) -> RendererResult<()>;

    /// Creates a compute pipeline using the provided shader.
    fn create_compute_pipeline(&self, shader: &Shader) -> RendererResult<Self::ComputePipeline>;
    /// Destroys the compute pipeline.
    fn destroy_compute_pipeline(&self, pipeline: &Self::ComputePipeline) -> RendererResult<()>;
    /// Creates a compute workload for the GPU.
    #[allow(clippy::type_complexity)]
    fn record_compute_command(
        &mut self,
        pipeline: &Self::ComputePipeline,
        binding_sets: &[&[ResourceBinding<Self::Buffer, Self::Texture, Self::Sampler>]],
        group_count: (u32, u32, u32),
    ) -> RendererResult<()>;

    /// Creates a graphics pipeline using the provided shaders.
    fn create_graphics_pipeline(
        &mut self,
        shaders: &[&Shader],
        options: &GraphicsPipelineOptions,
    ) -> RendererResult<Self::GraphicsPipeline>;
    /// Destroys the graphics pipeline.
    fn destroy_graphics_pipeline(
        &mut self,
        pipeline: &Self::GraphicsPipeline,
    ) -> RendererResult<()>;
    /// Creates a graphics workload for the GPU.
    #[allow(clippy::type_complexity)]
    fn record_graphics_command(
        &mut self,
        pipeline: &Self::GraphicsPipeline,
        binding_sets: &[&[ResourceBinding<Self::Buffer, Self::Texture, Self::Sampler>]],
        draw_count: u32,
        instance_count: u32,
    ) -> RendererResult<()>;

    /// Begins a new frame. `end_frame` must be called when the frame is done.
    fn begin_frame(&mut self) -> RendererResult<()>;
    /// Ends the frame. `begin_frame` must be called before calling this method.
    fn end_frame(&mut self) -> RendererResult<()>;

    /// Blocks the current CPU thread until the GPU has finished executing all
    /// pending commands.
    ///
    /// ### Note
    /// This function can only be called from the main thread.
    fn wait_idle(&self) -> RendererResult<()>;

    /// Clears the current render target with the specified color.
    fn clear(&mut self, color: RGB<f32>) -> RendererResult<()>;
}

/// Gets the maximum number of threads that can execute concurrently.
pub const fn max_concurrent_threads() -> usize {
    crate::renderers::thread_context::thread_context_count()
}
