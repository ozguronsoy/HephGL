pub mod concurrency;
pub mod error;
pub mod resources;
pub mod settings;
mod thread_context;

pub mod vulkan;

use renkrs::RGB;

use crate::{
    graphics_device::GraphicsDevice, renderers::error::*, renderers::resources::*,
    renderers::settings::*, shader::ShaderSource,
};

// Represents the result of a renderer operation.
type RendererResult<T> = Result<T, RendererError>;

/// The core interface for a graphics renderer.
pub trait Renderer {
    /// Represents a compiled shader module on the GPU.
    type ShaderHandle: Copy + Clone + Send + Sync;
    /// Represents a block of memory on the GPU.
    type BufferHandle: GpuBuffer;
    /// Represents a compiled graphics pipeline.
    type GraphicsPipelineHandle: Copy + Clone + Send + Sync;
    /// Represents a compiled compute pipeline.
    type ComputePipelineHandle: Copy + Clone + Send + Sync;
    /// Represents a resource set.
    type ResourceSetHandle: Copy + Clone + Send + Sync;
    /// Represents a recorded command.
    type RecordedCommand: Copy + Clone + Send + Sync;

    /// Indicates the default graphics device used in current system.
    const DEFAULT_DEVICE: Option<&GraphicsDevice> = None;

    /// Creates an uninitialized instance of the renderer.
    fn new() -> Self;

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

    /// Compiles the shader from the provided source.
    fn create_shader(&self, source: &ShaderSource) -> RendererResult<Self::ShaderHandle>;
    /// Destroys the shader and frees the resources.
    fn destroy_shader(&self, shader: &Self::ShaderHandle) -> RendererResult<()>;

    /// Creates a resource set.
    fn create_resource_set(
        &self,
        pipeline_handle: &PipelineHandle<Self::GraphicsPipelineHandle, Self::ComputePipelineHandle>,
        bindings: &[ResourceBinding<Self::BufferHandle>],
    ) -> RendererResult<Self::ResourceSetHandle>;

    /// Allocates a new buffer on the GPU with the specified size and usage.
    fn create_buffer(&self, size: usize, usage: BufferUsage) -> RendererResult<Self::BufferHandle>;
    /// Writes data to the buffer on the GPU.
    fn write_buffer(&self, buffer: &Self::BufferHandle, data: &[u8]) -> RendererResult<()>;
    /// Reads data from the buffer on the GPU.
    fn read_buffer(&self, buffer: &Self::BufferHandle, dest: &mut [u8]) -> RendererResult<()>;
    /// Frees the memory allocated for the provided buffer.
    fn destroy_buffer(&self, buffer: &mut Self::BufferHandle) -> RendererResult<()>;

    /// Creates a compute pipeline using the provided shader.
    fn create_compute_pipeline(
        &self,
        shader: &Self::ShaderHandle,
    ) -> RendererResult<Self::ComputePipelineHandle>;
    /// Destroys the compute pipeline.
    fn destroy_compute_pipeline(
        &self,
        pipeline: &Self::ComputePipelineHandle,
    ) -> RendererResult<()>;
    /// Dispatches a compute workload to the GPU.
    fn record_compute_pass(
        &mut self,
        pipeline: &Self::ComputePipelineHandle,
        resource_sets: &[&Self::ResourceSetHandle],
        group_count: (u32, u32, u32),
    ) -> RendererResult<Self::RecordedCommand>;

    /// Submits the recorded commands to the GPU.
    ///
    /// ### Note
    /// This function can only be called from the main thread.
    fn submit_commands(
        &mut self,
        recorded_commands: &[Self::RecordedCommand],
    ) -> RendererResult<()>;

    /// Begins a new frame. `end_frame` must be called when the frame is done.
    ///
    /// ### Note
    /// This function can only be called from the main thread.
    fn begin_frame(&mut self) -> RendererResult<()>;
    /// Ends the frame. `begin_frame` must be called before calling this method.
    ///
    /// ### Note
    /// This function can only be called from the main thread.
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
