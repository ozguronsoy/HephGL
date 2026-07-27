use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

use crate::{graphics_device::GraphicsDevice, shader::ShaderSource};

/// Represents the settings used throughout the lifetime of the renderer.
#[derive(Debug, Clone, Copy)]
pub struct Settings {
    /// The maximum number of frames that can be processed concurrently by the CPU and GPU.
    pub frames_in_flight: u32,
}

/// Represents a request for a specific graphics feature, indicating whether it is strictly required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FeatureRequest {
    /// The feature that is being requested.
    pub feature: crate::graphics_device::Feature,
    /// Indicates whether the graphics device must support the requested feature.
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

/// Defines the possible usages of a buffer.
pub enum BufferUsage {
    /// The buffer is used to store general purpose data.
    Storage,
    /// The buffer is used to pass read-only data, such as transformation matrices or material properties, to shaders.
    Uniform,
    /// The buffer is used to store vertex data for 3D geometry.
    Vertex,
    /// The buffer is used to store index data for drawing geometry.
    Index,
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
    /// The renderer or one of its core subsystems failed to initialize.
    FailedToInitialize(String),
    /// Failed to create the window or presentation surface.
    FailedToCreateSurface(String),
    /// Failed to enumerate the available graphics devices.
    FailedToEnumerateDevices(String),
    /// Failed to enumerate the features supported by the graphics device.
    FailedToEnumerateSupportedFeatures(String),
    /// A requested feature marked as required is not supported by the physical device.
    UnsupportedRequiredFeature(crate::graphics_device::Feature),
}

/// The core interface for a graphics renderer.
pub trait Renderer {
    /// Represents a compiled shader module on the GPU.
    type ShaderHandle;
    /// Represents a block of memory on the GPU.
    type BufferHandle;
    /// Represents a compiled compute pipeline.
    type ComputePipelineHandle;

    /// Creates an uninitialized instance of the renderer.
    fn new() -> Self;

    /// Returns the current settings used by the renderer.
    fn get_settings(&self) -> &Settings;
    /// Updates the settings.
    fn set_settings(&mut self, settings: Settings) -> Result<(), RendererError>;

    /// Initializes the renderer using the provided options.
    fn initialize(&mut self, options: &InitializeOptions) -> Result<(), RendererError>;
    /// Frees all internal resources and shutdowns the internal API.
    fn uninitialize(&mut self);

    /// Enumerates all available graphics devices on the system.
    fn enumerate_devices(&mut self) -> Result<Vec<GraphicsDevice>, RendererError>;
    /// Returns the currently active device, or `None` if there is no active device.
    fn get_device(&self) -> Option<GraphicsDevice>;
    /// Sets the active graphics device and initializes it with the requested features.
    fn set_device(
        &mut self,
        device: &GraphicsDevice,
        requested_features: &Vec<FeatureRequest>,
    ) -> Result<(), RendererError>;

    /// Compiles the shader from the provided source.
    fn create_shader(&self, source: &ShaderSource) -> Result<Self::ShaderHandle, RendererError>;
    /// Destroys the shader and frees the resources.
    fn destroy_shader(&self, shader: &Self::ShaderHandle);

    /// Allocates a new buffer on the GPU with the specified size and usage.
    fn create_buffer(
        &self,
        size: u64,
        usage: BufferUsage,
    ) -> Result<Self::BufferHandle, RendererError>;
    /// Writes data to the buffer on the GPU.
    fn write_buffer(&self, buffer: &Self::BufferHandle, data: &[u8]) -> Result<(), RendererError>;
    /// Reads data from the buffer on the GPU.
    fn read_buffer(
        &self,
        buffer: &Self::BufferHandle,
        dest: &mut [u8],
    ) -> Result<(), RendererError>;
    /// Frees the memory allocated for the provided buffer.
    fn destroy_buffer(&self, buffer: &mut Self::BufferHandle);

    /// Creates a compute pipeline using the provided shader.
    fn create_compute_pipeline(
        &self,
        shader: &Self::ShaderHandle,
    ) -> Result<Self::ComputePipelineHandle, RendererError>;
    /// Destroys the compute pipeline.
    fn destroy_compute_pipeline(&self, pipeline: &Self::ComputePipelineHandle);
    /// Dispatches a compute workload to the GPU.
    fn dispatch_compute(
        &mut self,
        pipeline: &Self::ComputePipelineHandle,
        buffers: &[&Self::BufferHandle],
        group_count: (u32, u32, u32),
    ) -> Result<(), RendererError>;

    /// Blocks the current CPU thread until the GPU has finished executing all pending commands.
    fn wait_idle(&self) -> Result<(), RendererError>;

    /// Clears the current render target with the specified color.
    fn clear(&mut self, color: RGB<f32>) -> Result<(), RendererError>;
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            frames_in_flight: 1,
        }
    }
}

impl std::fmt::Display for RendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAppName => write!(f, "Invalid application name provided."),
            Self::InvalidArgument(err) => write!(f, "Invalid argument: {}", err),
            Self::InvalidOperation(err) => write!(f, "Invalid operation: {}", err),
            Self::Fail(err) => write!(f, "{}", err),
            Self::FailedToInitialize(err) => write!(f, "Failed to load graphics backend: {}", err),
            Self::FailedToCreateSurface(err) => {
                write!(f, "Failed to create window surface: {}", err)
            }
            Self::FailedToEnumerateDevices(err) => {
                write!(f, "Failed to enumerate physical devices: {}", err)
            }
            Self::FailedToEnumerateSupportedFeatures(err) => {
                write!(f, "Failed to enumerate device features: {}", err)
            }
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
