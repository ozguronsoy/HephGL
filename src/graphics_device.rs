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
    /// A hardware device that does not cleanly fit into any of the standard categories.
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
