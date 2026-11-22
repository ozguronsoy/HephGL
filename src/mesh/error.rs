use crate::Error;

#[cfg(feature = "gltf")]
impl From<gltf::Error> for Error {
    fn from(value: gltf::Error) -> Self {
        Self::fail(value.to_string())
    }
}
