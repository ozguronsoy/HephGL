use crate::Error;

impl From<ttf_parser::FaceParsingError> for Error {
    fn from(value: ttf_parser::FaceParsingError) -> Self {
        Self::font(value.to_string())
    }
}
impl From<lyon_tessellation::TessellationError> for Error {
    fn from(value: lyon_tessellation::TessellationError) -> Self {
        Self::geometry(value.to_string())
    }
}
impl From<&str> for Error {
    fn from(value: &str) -> Self {
        Self::font(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_font_error_message() {
        let err = Error::from("invalid font data");
        assert!(matches!(err, Error::Font(ref msg) if msg == "invalid font data"));
    }
}
