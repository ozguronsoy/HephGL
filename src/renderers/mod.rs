mod canvas;
mod concurrency;
pub mod debug;
mod id;
mod pipeline;
mod resources;
mod settings;
mod thread_context;
mod version;
#[cfg(feature = "vulkan")]
pub mod vulkan;

use std::any::{Any, type_name};

pub use crate::renderers::{
    canvas::*, concurrency::*, pipeline::*, resources::*, settings::*, version::*,
};
use crate::{
    Error, Result, graphics_device::GraphicsDevice, renderers::debug::Debugger, shader::Shader,
};

/// Represents an automatic version selection. When used, the renderer will initialize with the
/// latest API version supported by the current hardware.
pub const LATEST_API_VERSION: Option<crate::Version> = None;
/// Represents an automatic device selection. When used, the renderer will set the default graphics
/// device used in the current system.
pub const DEFAULT_DEVICE: Option<&GraphicsDevice> = None;
/// Represents the display as the render target.
pub const DISPLAY_CANVAS: Option<&mut dyn RendererCanvas> = None;
/// Represents the texture format of the display target.
pub const DISPLAY_TEXTURE_FORMAT: Option<TextureFormat> = None;

/// The core interface for a graphics renderer.
pub trait Renderer: Any {
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

    /// Checks whether the HDR mode is supported by the current device and window.
    fn is_hdr_mode_supported(&self, hdr_mode: HdrMode) -> Result<bool>;

    /// Gets the attached debugger if there is one.
    fn debugger(&self) -> Option<&Debugger>;
    /// Attaches a debugger. Any debugger that is already attached must be detached first.
    ///
    /// ### Note
    /// This function can only be called from the main thread.
    fn attach_debugger(&mut self, debugger: Debugger)
    -> std::result::Result<(), (Error, Debugger)>;
    /// Detaches the debugger.
    ///
    /// ### Note
    /// This function can only be called from the main thread.
    fn detach_debugger(&mut self) -> Result<Debugger>;

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

    /// Enumerates the graphics devices available on the system, with the most capable devices
    /// first.
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

    /// Creates a worker that executes tasks on a separate thread.
    fn spawn_worker(&self) -> Box<dyn RendererWorker + '_>;

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
        &self,
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
        &self,
        pipeline: &mut dyn RendererComputePipeline,
        binding_sets: &[&[ResourceBinding<'_>]],
        group_count: (u32, u32, u32),
    ) -> Result<()>;

    /// Creates a graphics pipeline using the provided shaders.
    fn create_graphics_pipeline(
        &self,
        shaders: &[&Shader],
        options: &GraphicsPipelineOptions,
    ) -> Result<Box<dyn RendererGraphicsPipeline>>;
    /// Destroys the graphics pipeline.
    fn destroy_graphics_pipeline(&self, pipeline: &mut dyn RendererGraphicsPipeline) -> Result<()>;
    /// Creates a graphics workload for the GPU that renders into the target canvas.
    fn record_graphics_command(
        &self,
        target_canvas: Option<&mut dyn RendererCanvas>,
        pipeline: &mut dyn RendererGraphicsPipeline,
        index_buffer: Option<&dyn RendererBuffer>,
        vertex_bindings: &[ResourceBinding<'_>],
        binding_sets: &[&[ResourceBinding<'_>]],
        draw_count: u32,
        instance_count: u32,
    ) -> Result<()>;

    /// Creates an off-screen canvas using the provided options, cleared to black.
    fn create_canvas(&self, options: &CanvasOptions) -> Result<Box<dyn RendererCanvas>>;
    /// Frees the resources allocated for the provided canvas.
    fn destroy_canvas(&self, canvas: &mut dyn RendererCanvas) -> Result<()>;

    /// Begins a new frame. `end_frame` must be called when the frame is done.
    fn begin_frame(&self) -> Result<()>;
    /// Ends the frame. `begin_frame` must be called before calling this method.
    fn end_frame(&self) -> Result<()>;

    /// Blocks the current CPU thread until the GPU has finished executing all
    /// pending commands.
    ///
    /// ### Note
    /// This function can only be called from the main thread.
    fn wait_idle(&self) -> Result<()>;

    /// Clears the provided render target with the specified color.
    fn clear(&self, target_canvas: Option<&mut dyn RendererCanvas>, color: [f32; 3]) -> Result<()>;
}

/// Gets the maximum number of threads that can execute concurrently.
pub const fn max_concurrent_threads() -> usize {
    crate::renderers::thread_context::thread_context_count()
}

/// Downcasts the reference to the provided type.
fn downcast_ref<T: Any>(value: &dyn Any) -> Result<&T> {
    value.downcast_ref::<T>().ok_or_else(|| {
        Error::invalid_argument(format!("Type mismatch: expected {}.", type_name::<T>()))
    })
}

/// Downcasts the mutable reference to the provided type.
fn downcast_mut<T: Any>(value: &mut dyn Any) -> Result<&mut T> {
    value.downcast_mut::<T>().ok_or_else(|| {
        Error::invalid_argument(format!("Type mismatch: expected {}.", type_name::<T>()))
    })
}
