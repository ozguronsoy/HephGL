use crate::Error;

impl From<std::ffi::NulError> for Error {
    fn from(_: std::ffi::NulError) -> Self {
        Self::InvalidAppName
    }
}
impl From<ash::LoadingError> for Error {
    fn from(e: ash::LoadingError) -> Self {
        Error::Fail(e.to_string())
    }
}
impl From<ash::vk::Result> for Error {
    fn from(e: ash::vk::Result) -> Self {
        Error::Fail(e.to_string())
    }
}
impl From<(Vec<ash::vk::Pipeline>, ash::vk::Result)> for Error {
    fn from(e: (Vec<ash::vk::Pipeline>, ash::vk::Result)) -> Self {
        Error::Fail(e.1.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::*;

    #[test]
    fn test_from_nul_error() {
        let nul_error = CString::new(b"invalid\0name".to_vec()).unwrap_err();
        let err = Error::from(nul_error);
        assert_eq!(err, Error::InvalidAppName);
    }

    #[test]
    fn test_from_ash_vk_result() {
        let vk_result = ash::vk::Result::ERROR_DEVICE_LOST;
        let err = Error::from(vk_result);
        assert_eq!(err, Error::fail(""));
        assert_eq!(err.to_string(), vk_result.to_string());
    }

    #[test]
    fn test_from_pipeline_error_tuple() {
        let vk_result = ash::vk::Result::ERROR_OUT_OF_DEVICE_MEMORY;
        let pipelines: Vec<ash::vk::Pipeline> = vec![];
        let err = Error::from((pipelines, vk_result));
        assert_eq!(err, Error::fail(""));
        assert_eq!(err.to_string(), vk_result.to_string());
    }

    #[test]
    fn test_from_ash_loading_error() {
        let loading_error = unsafe { ash::Entry::load_from("").err().expect("Expected to fail") };
        let expected_msg = loading_error.to_string();
        let err = Error::from(loading_error);
        assert_eq!(err, Error::fail(""));
        assert_eq!(err.to_string(), expected_msg);
    }
}
