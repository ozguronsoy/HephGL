pub mod graphics_device;
pub mod math;
pub mod renderers;
pub mod shader;

/// Represents a version number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    /// Major part of the version.
    pub major: u32,
    /// Minor part of the version.
    pub minor: u32,
    /// Patch part of the version.
    pub patch: u32,
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Name of the HephGL engine.
pub const HEPHGL_ENGINE_NAME: &std::ffi::CStr = c"HephGL";
/// Current version of the HephGL engine.
pub const HEPHGL_ENGINE_VERSION: Version = Version {
    major: 0,
    minor: 1,
    patch: 0,
};
