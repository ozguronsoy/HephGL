use crate::Version;

/// Represents a vendor-specific driver version number.
pub struct DriverVersion {
    /// The version number.
    pub value: u32,
    /// The device vendor.
    pub vendor: crate::graphics_device::Vendor,
}

impl DriverVersion {
    pub const fn new(value: u32, vendor: crate::graphics_device::Vendor) -> Self {
        Self { value, vendor }
    }
}

impl From<DriverVersion> for Version {
    fn from(driver_version: DriverVersion) -> Self {
        match driver_version.vendor {
            crate::graphics_device::Vendor::Nvidia => Version::new(
                (driver_version.value >> 22) & 0x3FF,
                (driver_version.value >> 14) & 0x0FF,
                (driver_version.value >> 6) & 0x0FF,
            ),
            crate::graphics_device::Vendor::Intel => {
                Version::new(driver_version.value >> 14, driver_version.value & 0x3FFF, 0)
            }
            _ => Version::new(
                (driver_version.value >> 22) & 0x7F,
                (driver_version.value >> 12) & 0x3FF,
                driver_version.value & 0xFFF,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics_device::Vendor;

    #[test]
    fn test_driver_version_to_version() {
        {
            let value = (535 << 22) | (104 << 14) | (5 << 6);
            let driver = DriverVersion::new(value, Vendor::Nvidia);
            let version: Version = driver.into();
            assert_eq!(version, Version::new(535, 104, 5));
        }

        {
            let value = (100 << 14) | 1234;
            let driver = DriverVersion::new(value, Vendor::Intel);
            let version: Version = driver.into();
            assert_eq!(version, Version::new(100, 1234, 0));
        }

        {
            let value = (1 << 22) | (2 << 12) | 3;
            let driver = DriverVersion::new(value, Vendor::Amd);
            let version: Version = driver.into();
            assert_eq!(version, Version::new(1, 2, 3));
        }

        {
            let value = (2 << 22) | (4 << 12) | 10;
            let driver = DriverVersion::new(value, Vendor::Qualcomm);
            let version: Version = driver.into();
            assert_eq!(version, Version::new(2, 4, 10));
        }

        {
            let value = (3 << 22) | (1 << 12) | 50;
            let driver = DriverVersion::new(value, Vendor::Arm);
            let version: Version = driver.into();
            assert_eq!(version, Version::new(3, 1, 50));
        }

        {
            let value = (1 << 22) | (15 << 12) | 2;
            let driver = DriverVersion::new(value, Vendor::Apple);
            let version: Version = driver.into();
            assert_eq!(version, Version::new(1, 15, 2));
        }

        {
            let value = (10 << 22) | (3 << 12) | 1;
            let driver = DriverVersion::new(value, Vendor::Microsoft);
            let version: Version = driver.into();
            assert_eq!(version, Version::new(10, 3, 1));
        }

        {
            let value = (9 << 22) | (99 << 12) | 999;
            let driver = DriverVersion::new(value, Vendor::Other);
            let version: Version = driver.into();
            assert_eq!(version, Version::new(9, 99, 999));
        }
    }
}
