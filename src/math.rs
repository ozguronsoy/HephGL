// This makes it easy to change the math library if we ever want to.

pub(crate) use glam::camera::lh::proj::directx::{orthographic, perspective};

pub type Vec2 = glam::Vec2;
pub type Vec3 = glam::Vec3;
pub type Vec4 = glam::Vec4;
pub type Mat3 = glam::Mat3;
pub type Mat4 = glam::Mat4;
pub type Quat = glam::Quat;

/// Represents an error that can occur while calculating transforms or camera matrices.
#[derive(Debug, Clone, PartialEq)]
pub enum MathError {
    /// An invalid argument was provided to a math operation.
    InvalidArgument(String),
}

impl MathError {
    /// Creates a new `InvalidArgument` error from the provided message.
    pub(crate) fn invalid_argument(msg: impl Into<String>) -> Self {
        Self::InvalidArgument(msg.into())
    }
}
impl std::fmt::Display for MathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArgument(err) => write!(f, "Invalid argument: {}", err),
        }
    }
}
impl std::error::Error for MathError {}
