use crate::Error;

impl From<spirq::error::Error> for Error {
    fn from(value: spirq::error::Error) -> Self {
        Self::fail(value.to_string())
    }
}
