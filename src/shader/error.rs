use crate::Error;

impl From<naga::front::spv::Error> for Error {
    fn from(value: naga::front::spv::Error) -> Self {
        Self::fail(value.to_string())
    }
}
