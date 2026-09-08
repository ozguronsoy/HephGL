use crate::renderers::RendererError;

impl From<std::ffi::NulError> for RendererError {
    fn from(_: std::ffi::NulError) -> Self {
        RendererError::InvalidAppName
    }
}

impl<T> From<std::sync::PoisonError<T>> for RendererError {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        RendererError::Fail(e.to_string())
    }
}

impl From<ash::LoadingError> for RendererError {
    fn from(e: ash::LoadingError) -> Self {
        RendererError::Fail(e.to_string())
    }
}

impl From<ash::vk::Result> for RendererError {
    fn from(e: ash::vk::Result) -> Self {
        RendererError::Fail(e.to_string())
    }
}

impl From<(Vec<ash::vk::Pipeline>, ash::vk::Result)> for RendererError {
    fn from(e: (Vec<ash::vk::Pipeline>, ash::vk::Result)) -> Self {
        RendererError::Fail(e.1.to_string())
    }
}
