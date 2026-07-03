pub mod graphics_device;
pub mod math;
pub mod renderers;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

pub const HEPHGL_ENGINE_NAME: &std::ffi::CStr = c"HephGL";
pub const HEPHGL_ENGINE_VERSION: Version = Version {
    major: 0,
    minor: 1,
    patch: 0,
};
