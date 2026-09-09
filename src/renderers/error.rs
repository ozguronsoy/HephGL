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

impl RendererError {
    /// Creates a new `InvalidArgument` error from the provided message.
    pub fn invalid_argument(msg: impl Into<String>) -> Self {
        RendererError::InvalidArgument(msg.into())
    }
    /// Creates a new `InvalidOperation` error from the provided message.
    pub fn invalid_operation(msg: impl Into<String>) -> Self {
        RendererError::InvalidOperation(msg.into())
    }
    /// Creates a new `Fail` error from the provided message.
    pub fn fail(msg: impl Into<String>) -> Self {
        RendererError::Fail(msg.into())
    }
}

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
