use crate::renderers::RendererError;

impl From<std::ffi::NulError> for RendererError {
    fn from(_: std::ffi::NulError) -> Self {
        RendererError::InvalidAppName
    }
}

impl<T> From<std::sync::PoisonError<T>> for RendererError {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        RendererError::Fail(e.to_string())
    }
}

impl From<ash::LoadingError> for RendererError {
    fn from(e: ash::LoadingError) -> Self {
        RendererError::Fail(e.to_string())
    }
}

impl From<ash::vk::Result> for RendererError {
    fn from(e: ash::vk::Result) -> Self {
        RendererError::Fail(e.to_string())
    }
}

impl From<(Vec<ash::vk::Pipeline>, ash::vk::Result)> for RendererError {
    fn from(e: (Vec<ash::vk::Pipeline>, ash::vk::Result)) -> Self {
        RendererError::Fail(e.1.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::sync::{Arc, Mutex};
    use std::thread;

    use super::*;

    #[test]
    fn test_from_nul_error() {
        let nul_error = CString::new(b"invalid\0name".to_vec()).unwrap_err();
        let err = RendererError::from(nul_error);
        assert_eq!(err, RendererError::InvalidAppName);
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
        let err = RendererError::from(poison_error);
        assert_eq!(err, RendererError::fail(""));
        assert!(err.to_string().contains("poisoned lock"));
    }

    #[test]
    fn test_from_ash_vk_result() {
        let vk_result = ash::vk::Result::ERROR_DEVICE_LOST;
        let err = RendererError::from(vk_result);
        assert_eq!(err, RendererError::fail(""));
        assert_eq!(err.to_string(), vk_result.to_string());
    }

    #[test]
    fn test_from_pipeline_error_tuple() {
        let vk_result = ash::vk::Result::ERROR_OUT_OF_DEVICE_MEMORY;
        let pipelines: Vec<ash::vk::Pipeline> = vec![];
        let err = RendererError::from((pipelines, vk_result));
        assert_eq!(err, RendererError::fail(""));
        assert_eq!(err.to_string(), vk_result.to_string());
    }

    #[test]
    fn test_from_ash_loading_error() {
        let loading_error = unsafe { ash::Entry::load_from("").err().expect("Expected to fail") };
        let expected_msg = loading_error.to_string();
        let err = RendererError::from(loading_error);
        assert_eq!(err, RendererError::fail(""));
        assert_eq!(err.to_string(), expected_msg);
    }
}
