/// Represents an error that can occur while loading or processing text resources.
#[derive(Debug)]
pub enum TextError {
    /// An error occurred while reading font data from disk.
    Io(String),
    /// An error occurred while parsing or rasterizing a font.
    Font(String),
    /// An invalid parameter was provided when generating text.
    InvalidArgument(String),
    /// An error occurred while generating the glyph geometry.
    Geometry(String),
}

impl TextError {
    /// Creates a new `Io` error from the provided message.
    pub fn io(msg: impl Into<String>) -> Self {
        Self::Io(msg.into())
    }
    /// Creates a new `Font` error from the provided message.
    pub fn font(msg: impl Into<String>) -> Self {
        Self::Font(msg.into())
    }
    /// Creates a new `InvalidArgument` error from the provided message.
    pub fn invalid_argument(msg: impl Into<String>) -> Self {
        Self::InvalidArgument(msg.into())
    }
    /// Creates a new `Geometry` error from the provided message.
    pub fn geometry(msg: impl Into<String>) -> Self {
        Self::Geometry(msg.into())
    }
}

impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => {
                write!(f, "IO Error: {}", err)
            }
            Self::Font(err) => {
                write!(f, "Failed to load the font: {}", err)
            }
            Self::InvalidArgument(err) => {
                write!(f, "Invalid text argument: {}", err)
            }
            Self::Geometry(err) => {
                write!(f, "Failed to generate text geometry: {}", err)
            }
        }
    }
}

impl std::error::Error for TextError {}

impl From<std::io::Error> for TextError {
    fn from(value: std::io::Error) -> Self {
        Self::io(value.to_string())
    }
}

impl From<&str> for TextError {
    fn from(value: &str) -> Self {
        Self::font(value)
    }
}

impl From<ttf_parser::FaceParsingError> for TextError {
    fn from(value: ttf_parser::FaceParsingError) -> Self {
        Self::font(value.to_string())
    }
}

impl From<lyon_tessellation::TessellationError> for TextError {
    fn from(value: lyon_tessellation::TessellationError) -> Self {
        Self::geometry(value.to_string())
    }
}
