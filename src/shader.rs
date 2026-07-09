use std::path::Path;

#[derive(Debug)]
pub struct ShaderSource {
    pub file_path: String,
    pub(crate) data: Vec<u8>,
}

impl ShaderSource {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        let data = std::fs::read(path.as_ref())?;
        Ok(Self {
            file_path: path.as_ref().to_string_lossy().into_owned(),
            data,
        })
    }
}
