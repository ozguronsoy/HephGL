use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

use crate::{graphics_device::GraphicsDevice, shader::ShaderSource};

#[derive(Debug, Clone, Copy)]
pub struct Settings {
    pub frames_in_flight: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FeatureRequest {
    pub feature: crate::graphics_device::Feature,
    pub required: bool,
}

pub struct InitializeOptions<'a> {
    pub app_name: &'a str,
    pub window_handle: RawWindowHandle,
    pub display_handle: RawDisplayHandle,
}

pub enum BufferUsage {
    Storage,
    Uniform,
    Vertex,
    Index,
}

#[derive(Debug, Clone)]
pub enum RendererError {
    InvalidAppName,
    InvalidArgument(String),
    InvalidOperation(String),
    Fail(String),
    FailedToInitialize(String),
    FailedToCreateSurface(String),
    FailedToEnumerateDevices(String),
    FailedToEnumerateSupportedFeatures(String),
    UnsupportedRequiredFeature(crate::graphics_device::Feature),
}

pub trait Renderer {
    type ShaderHandle;
    type BufferHandle;
    type ComputePipelineHandle;

    fn new() -> Self;

    fn get_settings(&self) -> &Settings;
    fn set_settings(&mut self, settings: Settings) -> Result<(), RendererError>;

    fn initialize(&mut self, options: &InitializeOptions) -> Result<(), RendererError>;
    fn uninitialize(&mut self);

    fn enumerate_devices(&mut self) -> Result<Vec<GraphicsDevice>, RendererError>;
    fn get_device(&self) -> Option<GraphicsDevice>;
    fn set_device(
        &mut self,
        device: &GraphicsDevice,
        requested_features: &Vec<FeatureRequest>,
    ) -> Result<(), RendererError>;

    fn create_shader(&self, source: &ShaderSource) -> Result<Self::ShaderHandle, RendererError>;
    fn destroy_shader(&self, shader: Self::ShaderHandle);

    fn create_buffer(
        &self,
        size: u64,
        usage: BufferUsage,
    ) -> Result<Self::BufferHandle, RendererError>;
    fn write_buffer(&self, buffer: &Self::BufferHandle, data: &[u8]) -> Result<(), RendererError>;
    fn read_buffer(
        &self,
        buffer: &Self::BufferHandle,
        dest: &mut [u8],
    ) -> Result<(), RendererError>;
    fn destroy_buffer(&self, buffer: &mut Self::BufferHandle);

    fn create_compute_pipeline(
        &self,
        shader: &Self::ShaderHandle,
    ) -> Result<Self::ComputePipelineHandle, RendererError>;
    fn destroy_compute_pipeline(&self, pipeline: Self::ComputePipelineHandle);

    fn wait_idle(&self) -> Result<(), RendererError>;
    fn dispatch_compute(
        &mut self,
        pipeline: &Self::ComputePipelineHandle,
        buffers: &[&Self::BufferHandle],
        group_count: (u32, u32, u32),
    ) -> Result<(), RendererError>;

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
