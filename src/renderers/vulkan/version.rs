use crate::Version;

/// Dummy struct for implementing conversions between HephGL and Vulkan API versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VulkanApiVersion(pub u32);

impl From<VulkanApiVersion> for Version {
    fn from(value: VulkanApiVersion) -> Self {
        Self::new(
            ash::vk::api_version_major(value.0),
            ash::vk::api_version_minor(value.0),
            ash::vk::api_version_patch(value.0),
        )
    }
}

impl From<Version> for VulkanApiVersion {
    fn from(value: Version) -> Self {
        VulkanApiVersion(ash::vk::make_api_version(
            0,
            value.major,
            value.minor,
            value.patch,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vulkan_api_version_to_version() {
        let expected = Version::new(1, 2, 3);
        let vulkan_api_version = VulkanApiVersion(ash::vk::make_api_version(
            0,
            expected.major,
            expected.minor,
            expected.patch,
        ));
        let version: Version = vulkan_api_version.into();
        assert_eq!(version, expected);
    }

    #[test]
    fn test_version_to_vulkan_api_version() {
        let expected = VulkanApiVersion(ash::vk::make_api_version(0, 4, 5, 6));
        let version: Version = Version::new(
            ash::vk::api_version_major(expected.0),
            ash::vk::api_version_minor(expected.0),
            ash::vk::api_version_patch(expected.0),
        );
        let vulkan_api_version: VulkanApiVersion = version.into();
        assert_eq!(vulkan_api_version, expected);
    }
}
