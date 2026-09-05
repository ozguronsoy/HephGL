use std::collections::HashSet;

use crate::Version;

/// Defines the supported GPU types.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Type {
    /// A dedicated graphics card.
    DiscreteGpu,
    /// A graphics processor integrated to the CPU.
    IntegratedGpu,
    /// A virtualized graphics device.
    VirtualGpu,
    /// A software-based rendering implementation running on the CPU.
    Cpu,
    /// A hardware device that does not cleanly fit into any of the standard
    /// categories.
    Other,
    /// An unrecognized GPU type.
    Invalid,
}

/// Defines the GPU vendors.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
    Qualcomm,
    Arm,
    Apple,
    Microsoft,
    Other,
}

/// Defines the features a graphics device may support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    /// The device has dedicated geometry shader(s).
    GeometryShaders,
    /// The device has shader(s) dedicated to general computing.
    ComputeShaders,
    /// The device supports rendering polygons as wireframes.
    WireframeMode,
    /// The device supports drawing lines wider than a single pixel.
    WideLines,
    /// The device supports anisotropic texture filtering.
    AnisotropicFiltering,
    /// The device features hardware-accelerated ray tracing
    RayTracing,
    /// The device supports hardware-accelerated video decoding.
    VideoDecoding,
    /// The device supports hardware-accelerated video encoding.
    VideoEncoding,
    /// The device features dedicated hardware for optical flow calculations.
    OpticalFlow,
    /// The device supports asynchronous data transfers.
    AsyncTransfer,
}

/// Represents a graphics device.
#[derive(Debug, PartialEq, Clone)]
pub struct GraphicsDevice {
    /// The name of the graphics device.
    pub name: String,
    /// The type of the graphics device.
    pub device_type: Type,
    /// The unique identifier for the hardware vendor.
    pub vendor_id: u32,
    /// The unique identifier for the specific device model.
    pub device_id: u32,
    /// The version of the supported graphics API.
    pub api_version: Version,
    /// The version of the installed graphics driver.
    pub driver_version: Version,
    /// The total amount of dedicated VRAM in bytes.
    pub vram: u64,
    /// The set of features supported by this device.
    pub supported_features: HashSet<Feature>,
}

impl GraphicsDevice {
    /// Gets the vendor of the device.
    pub fn vendor(&self) -> Vendor {
        GraphicsDevice::vendor_from_id(self.vendor_id)
    }

    /// Sets the `vendor_id` of the device.
    pub fn set_vendor(&mut self, vendor: Vendor) {
        self.vendor_id = match vendor {
            Vendor::Nvidia => 0x10DE,
            Vendor::Amd => 0x1002,
            Vendor::Intel => 0x8086,
            Vendor::Qualcomm => 0x5143,
            Vendor::Arm => 0x13B5,
            Vendor::Apple => 0x106B,
            Vendor::Microsoft => 0x1414,
            Vendor::Other => 0,
        }
    }

    /// Gets the vendor from vendor id.
    pub fn vendor_from_id(vendor_id: u32) -> Vendor {
        match vendor_id {
            0x10DE => Vendor::Nvidia,
            0x1002 => Vendor::Amd,
            0x8086 => Vendor::Intel,
            0x5143 => Vendor::Qualcomm,
            0x13B5 => Vendor::Arm,
            0x106B => Vendor::Apple,
            0x1414 => Vendor::Microsoft,
            _ => Vendor::Other,
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Type::DiscreteGpu => write!(f, "Discrete GPU"),
            Type::IntegratedGpu => write!(f, "Integrated GPU"),
            Type::VirtualGpu => write!(f, "Virtual GPU"),
            Type::Cpu => write!(f, "CPU"),
            Type::Other => write!(f, "Other"),
            Type::Invalid => write!(f, "Invalid graphics device type."),
        }
    }
}

impl std::fmt::Display for Vendor {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Vendor::Nvidia => write!(f, "NVIDIA"),
            Vendor::Amd => write!(f, "AMD"),
            Vendor::Intel => write!(f, "Intel"),
            Vendor::Qualcomm => write!(f, "Qualcomm"),
            Vendor::Arm => write!(f, "ARM"),
            Vendor::Apple => write!(f, "Apple"),
            Vendor::Microsoft => write!(f, "Microsoft"),
            Vendor::Other => write!(f, "Other"),
        }
    }
}

impl std::fmt::Display for Feature {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Feature::GeometryShaders => write!(f, "Geometry Shaders"),
            Feature::ComputeShaders => write!(f, "Compute Shaders"),
            Feature::WireframeMode => write!(f, "Wireframe Mode"),
            Feature::WideLines => write!(f, "Wide Lines"),
            Feature::AnisotropicFiltering => write!(f, "Anisotropic Filtering"),
            Feature::RayTracing => write!(f, "Ray Tracing"),
            Feature::VideoDecoding => write!(f, "Video Decoding"),
            Feature::VideoEncoding => write!(f, "Video Encoding"),
            Feature::OpticalFlow => write!(f, "Optical Flow"),
            Feature::AsyncTransfer => write!(f, "Async Transfer"),
        }
    }
}

impl std::fmt::Display for GraphicsDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "Graphics Device:")?;
        writeln!(f, "    Name:               {}", self.name)?;
        writeln!(f, "    Type:               {}", self.device_type)?;
        writeln!(f, "    Vendor:             {}", self.vendor())?;
        writeln!(f, "    Vendor ID:          {:#06X}", self.vendor_id)?;
        writeln!(f, "    Device ID:          {:#06X}", self.device_id)?;
        writeln!(f, "    API Version:        {}", self.api_version)?;
        writeln!(f, "    Driver Version:     {}", self.driver_version)?;
        writeln!(
            f,
            "    VRAM:           {} GB",
            ((self.vram as f64) / (1024.0 * 1024.0 * 1024.0)).round() as u64
        )?;
        if !self.supported_features.is_empty() {
            writeln!(f, "    Supported Features:")?;
            for feature in &self.supported_features {
                writeln!(f, "     - {}", feature)?;
            }
        }
        std::fmt::Result::Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vendor() {
        let mut dev = GraphicsDevice {
            name: "".to_string(),
            device_type: Type::Other,
            vendor_id: 0,
            device_id: 0,
            api_version: Version {
                major: 0,
                minor: 0,
                patch: 0,
            },
            driver_version: Version {
                major: 0,
                minor: 0,
                patch: 0,
            },
            vram: 0,
            supported_features: HashSet::default(),
        };
        assert_eq!(dev.vendor(), Vendor::Other);

        dev.set_vendor(Vendor::Nvidia);
        assert_eq!(dev.vendor(), Vendor::Nvidia);

        dev.set_vendor(Vendor::Amd);
        assert_eq!(dev.vendor(), Vendor::Amd);

        dev.set_vendor(Vendor::Intel);
        assert_eq!(dev.vendor(), Vendor::Intel);

        dev.set_vendor(Vendor::Qualcomm);
        assert_eq!(dev.vendor(), Vendor::Qualcomm);

        dev.set_vendor(Vendor::Arm);
        assert_eq!(dev.vendor(), Vendor::Arm);

        dev.set_vendor(Vendor::Apple);
        assert_eq!(dev.vendor(), Vendor::Apple);

        dev.set_vendor(Vendor::Microsoft);
        assert_eq!(dev.vendor(), Vendor::Microsoft);

        dev.set_vendor(Vendor::Other);
        assert_eq!(dev.vendor(), Vendor::Other);
    }

    #[test]
    fn test_type_display() {
        assert_eq!(Type::DiscreteGpu.to_string(), "Discrete GPU");
        assert_eq!(Type::IntegratedGpu.to_string(), "Integrated GPU");
        assert_eq!(Type::VirtualGpu.to_string(), "Virtual GPU");
        assert_eq!(Type::Cpu.to_string(), "CPU");
        assert_eq!(Type::Other.to_string(), "Other");
        assert_eq!(Type::Invalid.to_string(), "Invalid graphics device type.");
    }

    #[test]
    fn test_vendor_display() {
        assert_eq!(Vendor::Nvidia.to_string(), "NVIDIA");
        assert_eq!(Vendor::Amd.to_string(), "AMD");
        assert_eq!(Vendor::Intel.to_string(), "Intel");
        assert_eq!(Vendor::Qualcomm.to_string(), "Qualcomm");
        assert_eq!(Vendor::Arm.to_string(), "ARM");
        assert_eq!(Vendor::Apple.to_string(), "Apple");
        assert_eq!(Vendor::Microsoft.to_string(), "Microsoft");
        assert_eq!(Vendor::Other.to_string(), "Other");
    }

    #[test]
    fn test_feature_display() {
        assert_eq!(Feature::GeometryShaders.to_string(), "Geometry Shaders");
        assert_eq!(Feature::ComputeShaders.to_string(), "Compute Shaders");
        assert_eq!(Feature::WireframeMode.to_string(), "Wireframe Mode");
        assert_eq!(Feature::WideLines.to_string(), "Wide Lines");
        assert_eq!(
            Feature::AnisotropicFiltering.to_string(),
            "Anisotropic Filtering"
        );
        assert_eq!(Feature::RayTracing.to_string(), "Ray Tracing");
        assert_eq!(Feature::VideoDecoding.to_string(), "Video Decoding");
        assert_eq!(Feature::VideoEncoding.to_string(), "Video Encoding");
        assert_eq!(Feature::OpticalFlow.to_string(), "Optical Flow");
        assert_eq!(Feature::AsyncTransfer.to_string(), "Async Transfer");
    }

    #[test]
    fn test_graphics_device_display() {
        let mut device = GraphicsDevice {
            name: "".to_string(),
            device_type: Type::Other,
            vendor_id: 0,
            device_id: 0,
            api_version: Version {
                major: 1,
                minor: 0,
                patch: 0,
            },
            driver_version: Version {
                major: 1,
                minor: 0,
                patch: 0,
            },
            vram: 1024 * 1024 * 1024,
            supported_features: HashSet::default(),
        };

        assert!(!device.to_string().contains("Supported Features:"));

        device.supported_features.insert(Feature::RayTracing);
        assert!(device.to_string().contains("Supported Features:"));
    }
}
