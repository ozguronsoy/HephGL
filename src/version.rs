/// Current version of the HephGL.
pub(crate) const HEPHGL_VERSION: Version = Version::new(
    match u32::from_str_radix(env!("CARGO_PKG_VERSION_MAJOR"), 10) {
        Ok(v) => v,
        Err(_) => unreachable!(),
    },
    match u32::from_str_radix(env!("CARGO_PKG_VERSION_MINOR"), 10) {
        Ok(v) => v,
        Err(_) => unreachable!(),
    },
    match u32::from_str_radix(env!("CARGO_PKG_VERSION_PATCH"), 10) {
        Ok(v) => v,
        Err(_) => unreachable!(),
    },
);

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

impl Version {
    /// Creates a new instance.
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let expected = Version::new(1, 2, 3);
        assert_eq!(Version::new(1, 2, 3), expected);
        assert_eq!(
            expected.to_string(),
            format!("v{}.{}.{}", expected.major, expected.minor, expected.patch)
        );
    }
}
