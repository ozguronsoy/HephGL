use std::collections::HashSet;

use crate::Version;

/// Defines the supported GPU types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphicsDeviceType {
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
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphicsDeviceVendor {
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
pub enum GraphicsDeviceFeature {
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
    /// The device features hardware-accelerated ray tracing.
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
    pub device_type: GraphicsDeviceType,
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
    /// The maximum supported sampler anisotropy level.
    pub max_sampler_anisotropy: f32,
    /// The set of features supported by this device.
    pub supported_features: HashSet<GraphicsDeviceFeature>,
}

impl GraphicsDevice {
    /// Gets the vendor of the device.
    pub fn vendor(&self) -> GraphicsDeviceVendor {
        GraphicsDevice::vendor_from_id(self.vendor_id)
    }

    /// Sets the `vendor_id` of the device.
    pub fn set_vendor(&mut self, vendor: GraphicsDeviceVendor) {
        self.vendor_id = match vendor {
            GraphicsDeviceVendor::Nvidia => 0x10DE,
            GraphicsDeviceVendor::Amd => 0x1002,
            GraphicsDeviceVendor::Intel => 0x8086,
            GraphicsDeviceVendor::Qualcomm => 0x5143,
            GraphicsDeviceVendor::Arm => 0x13B5,
            GraphicsDeviceVendor::Apple => 0x106B,
            GraphicsDeviceVendor::Microsoft => 0x1414,
            GraphicsDeviceVendor::Other => 0,
        }
    }

    /// Gets the vendor from vendor id.
    pub fn vendor_from_id(vendor_id: u32) -> GraphicsDeviceVendor {
        match vendor_id {
            0x10DE => GraphicsDeviceVendor::Nvidia,
            0x1002 => GraphicsDeviceVendor::Amd,
            0x8086 => GraphicsDeviceVendor::Intel,
            0x5143 => GraphicsDeviceVendor::Qualcomm,
            0x13B5 => GraphicsDeviceVendor::Arm,
            0x106B => GraphicsDeviceVendor::Apple,
            0x1414 => GraphicsDeviceVendor::Microsoft,
            _ => GraphicsDeviceVendor::Other,
        }
    }
}

impl std::fmt::Display for GraphicsDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "Graphics Device:")?;
        writeln!(f, "    Name:               {}", self.name)?;
        writeln!(f, "    Type:               {:?}", self.device_type)?;
        writeln!(f, "    Vendor:             {:?}", self.vendor())?;
        writeln!(f, "    Vendor ID:          {:#06X}", self.vendor_id)?;
        writeln!(f, "    Device ID:          {:#06X}", self.device_id)?;
        writeln!(f, "    API Version:        {}", self.api_version)?;
        writeln!(f, "    Driver Version:     {}", self.driver_version)?;
        writeln!(
            f,
            "    VRAM:               {} GB",
            ((self.vram as f64) / (1024.0 * 1024.0 * 1024.0)).round() as u64
        )?;
        writeln!(f, "    Max Anisotropy:     {}", self.max_sampler_anisotropy)?;
        if !self.supported_features.is_empty() {
            writeln!(f, "    Supported Features:")?;
            for feature in &self.supported_features {
                writeln!(f, "     - {:?}", feature)?;
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
            device_type: GraphicsDeviceType::Other,
            vendor_id: 0,
            device_id: 0,
            api_version: Version::new(0, 0, 0),
            driver_version: Version::new(0, 0, 0),
            vram: 0,
            max_sampler_anisotropy: 1.0,
            supported_features: HashSet::default(),
        };
        assert_eq!(dev.vendor(), GraphicsDeviceVendor::Other);

        dev.set_vendor(GraphicsDeviceVendor::Nvidia);
        assert_eq!(dev.vendor(), GraphicsDeviceVendor::Nvidia);

        dev.set_vendor(GraphicsDeviceVendor::Amd);
        assert_eq!(dev.vendor(), GraphicsDeviceVendor::Amd);

        dev.set_vendor(GraphicsDeviceVendor::Intel);
        assert_eq!(dev.vendor(), GraphicsDeviceVendor::Intel);

        dev.set_vendor(GraphicsDeviceVendor::Qualcomm);
        assert_eq!(dev.vendor(), GraphicsDeviceVendor::Qualcomm);

        dev.set_vendor(GraphicsDeviceVendor::Arm);
        assert_eq!(dev.vendor(), GraphicsDeviceVendor::Arm);

        dev.set_vendor(GraphicsDeviceVendor::Apple);
        assert_eq!(dev.vendor(), GraphicsDeviceVendor::Apple);

        dev.set_vendor(GraphicsDeviceVendor::Microsoft);
        assert_eq!(dev.vendor(), GraphicsDeviceVendor::Microsoft);

        dev.set_vendor(GraphicsDeviceVendor::Other);
        assert_eq!(dev.vendor(), GraphicsDeviceVendor::Other);
    }

    #[test]
    fn test_graphics_device_display() {
        let mut device = GraphicsDevice {
            name: "".to_string(),
            device_type: GraphicsDeviceType::Other,
            vendor_id: 0,
            device_id: 0,
            api_version: Version::new(1, 0, 0),
            driver_version: Version::new(1, 0, 0),
            vram: 1024 * 1024 * 1024,
            max_sampler_anisotropy: 16.0,
            supported_features: HashSet::default(),
        };

        assert!(!device.to_string().contains("Supported Features:"));

        let _ = device
            .supported_features
            .insert(GraphicsDeviceFeature::RayTracing);
        assert!(device.to_string().contains("Supported Features:"));
    }
}
