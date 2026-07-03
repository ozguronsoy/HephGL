use crate::Version;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum GraphicsDeviceType {
    DiscreteGpu,
    IntegratedGpu,
    VirtualGpu,
    Cpu,
    Other,
    Invalid,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
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

#[derive(Debug, PartialEq)]
pub struct GraphicsDevice {
    pub name: String,
    pub device_type: GraphicsDeviceType,
    pub vendor_id: u32,
    pub device_id: u32,
    pub api_version: Version,
    pub driver_version: Version,
    pub vram: u64,
}

impl GraphicsDevice {
    pub fn vendor(&self) -> GraphicsDeviceVendor {
        GraphicsDevice::vendor_from_id(self.vendor_id)
    }

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

impl std::fmt::Display for GraphicsDeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            GraphicsDeviceType::DiscreteGpu => write!(f, "Discrete GPU"),
            GraphicsDeviceType::IntegratedGpu => write!(f, "Integrated GPU"),
            GraphicsDeviceType::VirtualGpu => write!(f, "Virtual GPU"),
            GraphicsDeviceType::Cpu => write!(f, "CPU"),
            GraphicsDeviceType::Other => write!(f, "Other"),
            GraphicsDeviceType::Invalid => write!(f, "Invalid graphics device type."),
        }
    }
}

impl std::fmt::Display for GraphicsDeviceVendor {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            GraphicsDeviceVendor::Nvidia => write!(f, "NVIDIA"),
            GraphicsDeviceVendor::Amd => write!(f, "AMD"),
            GraphicsDeviceVendor::Intel => write!(f, "Intel"),
            GraphicsDeviceVendor::Qualcomm => write!(f, "Qualcomm"),
            GraphicsDeviceVendor::Arm => write!(f, "ARM"),
            GraphicsDeviceVendor::Apple => write!(f, "Apple"),
            GraphicsDeviceVendor::Microsoft => write!(f, "Microsoft"),
            GraphicsDeviceVendor::Other => write!(f, "Other"),
        }
    }
}

impl std::fmt::Display for GraphicsDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "Graphics Device:")?;
        writeln!(f, "    Name:           {}", self.name)?;
        writeln!(f, "    Type:           {}", self.device_type)?;
        writeln!(f, "    Vendor:         {}", self.vendor())?;
        writeln!(f, "    Vendor ID:      {:#06X}", self.vendor_id)?;
        writeln!(f, "    Device ID:      {:#06X}", self.device_id)?;
        writeln!(f, "    API Version:    {}", self.api_version)?;
        writeln!(f, "    Driver Version: {}", self.driver_version)?;
        write!(
            f,
            "    VRAM:           {} GB",
            ((self.vram as f64) / (1024.0 * 1024.0 * 1024.0)).round() as u64
        )
    }
}
