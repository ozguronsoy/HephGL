use crate::graphics_device::GraphicsDeviceFeature;

/// Defines the error types.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// An invalid argument was provided.
    InvalidArgument(String),
    /// An invalid or unsupported operation was attempted.
    InvalidOperation(String),
    /// A general error or failure occurred.
    Fail(String),
    /// A requested graphics device feature marked as required is not supported.
    UnsupportedRequiredFeature(GraphicsDeviceFeature),
    /// The shader uses an unsupported language or bytecode format.
    UnsupportedShaderLanguage,
    /// An error occurred while reading or writing data.
    Io(String),
    /// An error occurred while parsing or rasterizing a font.
    Font(String),
    /// An error occurred while generating the glyph geometry.
    Geometry(String),
}

/// Represents the result of a library operation.
pub type Result<T> = std::result::Result<T, Error>;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for Error {}

impl Error {
    /// Creates a new `InvalidArgument` error from the provided message.
    pub fn invalid_argument(msg: impl Into<String>) -> Self {
        Self::InvalidArgument(msg.into())
    }
    /// Creates a new `InvalidOperation` error from the provided message.
    pub fn invalid_operation(msg: impl Into<String>) -> Self {
        Self::InvalidOperation(msg.into())
    }
    /// Creates a new `Fail` error from the provided message.
    pub fn fail(msg: impl Into<String>) -> Self {
        Self::Fail(msg.into())
    }
    /// Creates a new `Io` error from the provided message.
    pub fn io(msg: impl Into<String>) -> Self {
        Self::Io(msg.into())
    }
    /// Creates a new `Font` error from the provided message.
    pub fn font(msg: impl Into<String>) -> Self {
        Self::Font(msg.into())
    }
    /// Creates a new `Geometry` error from the provided message.
    pub fn geometry(msg: impl Into<String>) -> Self {
        Self::Geometry(msg.into())
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::io(value.to_string())
    }
}
impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(value: std::sync::PoisonError<T>) -> Self {
        Self::fail(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        thread,
    };

    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::invalid_argument("hello");
        assert_eq!(err.to_string(), format!("{:?}", err));
    }

    #[test]
    fn test_from_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = Error::from(io_error);
        assert!(matches!(err, Error::Io(ref msg) if msg == "file not found"));
    }

    #[test]
    fn test_from_poison_error() {
        let mutex = Arc::new(Mutex::new(0));
        let mutex_clone = Arc::clone(&mutex);

        let _ = thread::spawn(move || {
            let _lock = mutex_clone.lock().unwrap();
            panic!("Intentional panic to poison the mutex");
        })
        .join();

        let poison_error = mutex.lock().unwrap_err();
        let err = Error::from(poison_error);
        assert!(matches!(err, Error::Fail(_)));
        assert!(err.to_string().contains("poisoned lock"));
    }
}
