use std::collections::HashSet;

use crate::Version;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Type {
    DiscreteGpu,
    IntegratedGpu,
    VirtualGpu,
    Cpu,
    Other,
    Invalid,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    GeometryShaders,
    ComputeShaders,
    WireframeMode,
    WideLines,
    AnisotropicFiltering,
    RayTracing,
    VideoDecoding,
    VideoEncoding,
    OpticalFlow,
    AsyncTransfer,
}

#[derive(Debug, PartialEq, Clone)]
pub struct GraphicsDevice {
    pub name: String,
    pub device_type: Type,
    pub vendor_id: u32,
    pub device_id: u32,
    pub api_version: Version,
    pub driver_version: Version,
    pub vram: u64,
    pub supported_features: HashSet<Feature>,
}

impl GraphicsDevice {
    pub fn vendor(&self) -> Vendor {
        GraphicsDevice::vendor_from_id(self.vendor_id)
    }

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
