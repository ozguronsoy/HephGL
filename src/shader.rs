use std::path::Path;

/// Represents a loaded shader resource containing compiled bytecode.
#[derive(Debug)]
pub struct ShaderSource {
    /// The file path of the shader.
    pub file_path: String,
    /// The compiled bytecode.
    pub(crate) data: Vec<u8>,
}

impl ShaderSource {
    /// Loads the shader from a file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        let data = std::fs::read(path.as_ref())?;
        Ok(Self {
            file_path: path.as_ref().to_string_lossy().into_owned(),
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_file() {
        {
            const SHADER_PATH: &str = "shaders/addition.spv";
            let result = ShaderSource::from_file(SHADER_PATH);
            assert!(result.is_ok());
            let shader = result.unwrap();
            assert_eq!(shader.file_path, SHADER_PATH);
            assert!(!shader.data.is_empty());
        }
        {
            assert!(ShaderSource::from_file("").is_err());
        }
    }
}
