use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

use crate::graphics_device::GraphicsDevice;

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
    fn new() -> Self;

    fn initialize(&mut self, options: &InitializeOptions) -> Result<(), RendererError>;
    fn uninitialize(&mut self);

    fn enumerate_devices(&mut self) -> Result<Vec<GraphicsDevice>, RendererError>;
    fn get_device(&self) -> Option<GraphicsDevice>;
    fn set_device(
        &mut self,
        device: &GraphicsDevice,
        requested_features: &Vec<FeatureRequest>,
    ) -> Result<(), RendererError>;

    fn clear(&mut self, color: RGB<f32>) -> Result<(), RendererError>;
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
