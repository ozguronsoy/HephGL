use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

use crate::{graphics_device::GraphicsDevice, shader::ShaderSource};

/// Represents the settings used throughout the lifetime of the renderer.
#[derive(Debug, Clone, Copy)]
pub struct Settings {
    /// The maximum number of frames that can be processed concurrently by the
    /// CPU and GPU.
    pub frames_in_flight: u32,
}

/// Represents a request for a specific graphics feature, indicating whether it
/// is strictly required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FeatureRequest {
    /// The feature that is being requested.
    pub feature: crate::graphics_device::Feature,
    /// Indicates whether the graphics device must support the requested
    /// feature.
    pub required: bool,
}

/// Represents the options used while initializing the renderer.
pub struct InitializeOptions<'a> {
    /// The name of the application using the renderer.
    pub app_name: &'a str,
    /// Handle to the native window.
    pub window_handle: RawWindowHandle,
    /// Handle to the display device.
    pub display_handle: RawDisplayHandle,
}

/// Represents a resource binding.
#[derive(Debug, Clone, Copy)]
pub struct ResourceBinding<B> {
    /// The binding number specified in the shader (e.g., `layout(binding =
    /// 0)`).
    pub binding: u32,

    /// The actual resource this slot binds to.
    pub resource: ResourceBindingType<B>,
}

/// Defines the possible usages of a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferUsage {
    /// The buffer is used to store general purpose data.
    Storage,
    /// The buffer is used to pass read-only data, such as transformation
    /// matrices or material properties, to shaders.
    Uniform,
    /// The buffer is used to store vertex data for 3D geometry.
    Vertex,
    /// The buffer is used to store index data for drawing geometry.
    Index,
}

/// Defines the types of resources being bound to a shader slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceBindingType<BufferHandle> {
    /// A block of GPU memory.
    Buffer {
        /// The handle referencing the allocated buffer.
        handle: BufferHandle,
        /// Specifies how the shader intends to use this buffer.
        usage: BufferUsage,
        /// The starting byte offset within the buffer where the binding begins.
        offset: usize,
        /// The size in bytes of the buffer region being bound.
        size: usize,
    },
}

pub enum PipelineHandle<G, C> {
    Graphics(G),
    Compute(C),
}

/// Defines the errors that may occur while using the renderer.
#[derive(Debug, Clone)]
pub enum RendererError {
    /// An invalid app name is entered when initializing the renderer.
    InvalidAppName,
    /// An invalid argument was provided to a renderer function.
    InvalidArgument(String),
    /// An invalid or unsupported operation was attempted.
    InvalidOperation(String),
    /// A general error or failure occurred during a renderer operation.
    Fail(String),
    /// A requested feature marked as required is not supported by the physical
    /// device.
    UnsupportedRequiredFeature(crate::graphics_device::Feature),
}

// Represents the result of a renderer operation.
type RendererResult<T> = Result<T, RendererError>;

/// Stores data in a GPU.
pub trait GpuBuffer: std::fmt::Debug + Copy + Clone + Send + Sync {
    fn size(&self) -> usize;
}

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
    fn set_settings(&mut self, settings: Settings) -> RendererResult<()>;

    /// Initializes the renderer using the provided options.
    fn initialize(&mut self, options: &InitializeOptions) -> RendererResult<()>;
    /// Frees all internal resources and shutdowns the internal API.
    fn uninitialize(&mut self) -> RendererResult<()>;

    /// Enumerates all available graphics devices on the system.
    fn enumerate_devices(&self) -> RendererResult<Vec<GraphicsDevice>>;
    /// Returns the currently active device, or `None` if there is no active
    /// device.
    fn get_device(&self) -> Option<&GraphicsDevice>;
    /// Sets the active graphics device and initializes it with the requested
    /// features.
    fn set_device(
        &mut self,
        device: Option<&GraphicsDevice>,
        requested_features: &[FeatureRequest],
    ) -> RendererResult<()>;

    /// Initializes the per-thread execution resources for the renderer.
    ///
    /// ### Important
    /// This function must be run **once per worker thread** that will be
    /// recording commands.
    fn initialize_thread(&mut self) -> RendererResult<()>;
    /// Frees and destroys all per-frame execution resources allocated during
    /// `initialize_frames`.
    fn uninitialize_thread(&mut self) -> RendererResult<()>;

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
    fn submit_commands(
        &mut self,
        recorded_commands: &[Self::RecordedCommand],
    ) -> RendererResult<()>;

    /// Begins a new frame. `end_frame` must be called when the frame is done.
    fn begin_frame(&mut self) -> RendererResult<()>;
    /// Ends the frame. `begin_frame` must be called before calling this method.
    fn end_frame(&mut self) -> RendererResult<()>;

    /// Blocks the current CPU thread until the GPU has finished executing all
    /// pending commands.
    fn wait_idle(&self) -> RendererResult<()>;

    /// Clears the current render target with the specified color.
    fn clear(&mut self, color: RGB<f32>) -> RendererResult<()>;
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            frames_in_flight: 1,
        }
    }
}

impl PartialEq for RendererError {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}
impl std::fmt::Display for RendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAppName => {
                write!(f, "Invalid application name provided.")
            }
            Self::InvalidArgument(err) => {
                write!(f, "Invalid argument: {}", err)
            }
            Self::InvalidOperation(err) => {
                write!(f, "Invalid operation: {}", err)
            }
            Self::Fail(err) => write!(f, "{}", err),
            Self::UnsupportedRequiredFeature(feature) => {
                write!(
                    f,
                    "Required hardware feature is not supported: {:?}",
                    feature
                )
            }
        }
    }
}
impl std::error::Error for RendererError {}

pub mod vulkan_renderer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics_device::*;

    #[test]
    fn test_renderer_error_display() {
        let err = RendererError::InvalidAppName;
        assert_eq!(err.to_string(), "Invalid application name provided.");

        let err = RendererError::InvalidArgument("bad_ptr".into());
        assert_eq!(err.to_string(), "Invalid argument: bad_ptr");

        let err = RendererError::InvalidOperation("wrong state".into());
        assert_eq!(err.to_string(), "Invalid operation: wrong state");

        let err = RendererError::Fail("generic crash".into());
        assert_eq!(err.to_string(), "generic crash");

        let err = RendererError::UnsupportedRequiredFeature(Feature::ComputeShaders);
        assert_eq!(
            err.to_string(),
            "Required hardware feature is not supported: ComputeShaders"
        );
    }
}
