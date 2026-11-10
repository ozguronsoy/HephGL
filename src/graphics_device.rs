use std::collections::BTreeSet;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GraphicsDeviceFeature {
    /// The device supports dedicated geometry shaders.
    GeometryShaders,
    /// The device supports tessellation shaders.
    TessellationShaders,
    /// The device supports shaders dedicated to general computing.
    ComputeShaders,
    /// The device supports rendering polygons as wireframes.
    WireframeMode,
    /// The device supports anisotropic texture filtering.
    AnisotropicFiltering,
    /// The device supports shading individual samples of multisampled pixels.
    SampleRateShading,
    /// The device supports clamping fragment depth values instead of clipping primitives against
    /// the near and far planes.
    DepthClamp,
    /// The device features hardware-accelerated ray tracing.
    RayTracing,
    /// The device supports hardware-accelerated video decoding.
    VideoDecoding,
    /// The device supports hardware-accelerated video encoding.
    VideoEncoding,
    /// The device supports asynchronous data transfers.
    AsyncTransfer,
}

/// Represents a graphics device.
#[derive(Debug, PartialEq, Clone)]
pub struct GraphicsDevice {
    pub(crate) name: String,
    pub(crate) device_type: GraphicsDeviceType,
    pub(crate) vendor_id: u32,
    pub(crate) device_id: u32,
    pub(crate) api_version: Version,
    pub(crate) driver_version: Version,
    pub(crate) vram: u64,
    pub(crate) max_sampler_anisotropy: f32,
    pub(crate) supported_features: BTreeSet<GraphicsDeviceFeature>,
}

impl GraphicsDevice {
    /// Gets the name of the device.
    pub fn name(&self) -> &'_ str {
        &self.name
    }

    /// Gets the type of the device.
    pub fn device_type(&self) -> GraphicsDeviceType {
        self.device_type
    }

    /// Gets the vendor of the device.
    pub fn vendor(&self) -> GraphicsDeviceVendor {
        GraphicsDeviceVendor::from_id(self.vendor_id)
    }

    /// Gets the unique identifier of the device.
    pub fn device_id(&self) -> u32 {
        self.device_id
    }

    /// Gets the maximum supported backend API version by the device.
    pub fn api_version(&self) -> Version {
        self.api_version
    }

    /// Gets the version of the installed graphics driver.
    pub fn driver_version(&self) -> Version {
        self.driver_version
    }

    /// Gets the total amount of dedicated VRAM the device has in bytes.
    pub fn vram(&self) -> u64 {
        self.vram
    }

    /// Gets the total amount of dedicated VRAM the device has in kilobytes.
    pub fn vram_kb(&self) -> u64 {
        self.vram / 1024
    }

    /// Gets the total amount of dedicated VRAM the device has in megabytes.
    pub fn vram_mb(&self) -> u64 {
        self.vram_kb() / 1024
    }

    /// Gets the total amount of dedicated VRAM the device has in gigabytes.
    pub fn vram_gb(&self) -> u64 {
        self.vram_mb() / 1024
    }

    /// Gets the maximum sampler anisotropy level supported by the device.
    pub fn max_sampler_anisotropy(&self) -> f32 {
        self.max_sampler_anisotropy
    }

    /// Gets the set of features supported by the device.
    pub fn supported_features(&self) -> &BTreeSet<GraphicsDeviceFeature> {
        &self.supported_features
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
        writeln!(f, "    VRAM:               {} GB", self.vram_gb())?;
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

impl GraphicsDeviceVendor {
    pub fn from_id(id: u32) -> Self {
        match id {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vendor() {
        let mut device = GraphicsDevice {
            name: "".to_string(),
            device_type: GraphicsDeviceType::Other,
            vendor_id: 0,
            device_id: 0,
            api_version: Version::new(0, 0, 0),
            driver_version: Version::new(0, 0, 0),
            vram: 0,
            max_sampler_anisotropy: 1.0,
            supported_features: BTreeSet::default(),
        };
        assert_eq!(device.vendor(), GraphicsDeviceVendor::Other);

        device.vendor_id = 0x10DE;
        assert_eq!(device.vendor(), GraphicsDeviceVendor::Nvidia);

        device.vendor_id = 0x1002;
        assert_eq!(device.vendor(), GraphicsDeviceVendor::Amd);

        device.vendor_id = 0x8086;
        assert_eq!(device.vendor(), GraphicsDeviceVendor::Intel);

        device.vendor_id = 0x5143;
        assert_eq!(device.vendor(), GraphicsDeviceVendor::Qualcomm);

        device.vendor_id = 0x13B5;
        assert_eq!(device.vendor(), GraphicsDeviceVendor::Arm);

        device.vendor_id = 0x106B;
        assert_eq!(device.vendor(), GraphicsDeviceVendor::Apple);

        device.vendor_id = 0x1414;
        assert_eq!(device.vendor(), GraphicsDeviceVendor::Microsoft);
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
            supported_features: BTreeSet::default(),
        };

        assert!(!device.to_string().contains("Supported Features:"));

        let _ = device
            .supported_features
            .insert(GraphicsDeviceFeature::RayTracing);
        assert!(device.to_string().contains("Supported Features:"));
    }
}
