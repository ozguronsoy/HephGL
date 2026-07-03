#[derive(Debug, PartialEq, Eq)]
pub enum GraphicsDeviceType {
    DiscreteGpu,
    IntegratedGpu,
    VirtualGpu,
    Cpu,
    Other,
}

#[derive(Debug, PartialEq)]
pub struct GraphicsDevice {
    pub name: String,
    pub device_type: GraphicsDeviceType,
    pub vendor_id: u32,
    pub device_id: u32,
    pub api_version: String,
    pub driver_version: String,
}

impl std::fmt::Display for GraphicsDeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            GraphicsDeviceType::DiscreteGpu => write!(f, "Discrete GPU"),
            GraphicsDeviceType::IntegratedGpu => write!(f, "Integrated GPU"),
            GraphicsDeviceType::VirtualGpu => write!(f, "Virtual GPU"),
            GraphicsDeviceType::Cpu => write!(f, "CPU"),
            GraphicsDeviceType::Other => write!(f, "Other"),
        }
    }
}

impl std::fmt::Display for GraphicsDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "Graphics Device:")?;
        writeln!(f, "    Name:           {}", self.name)?;
        writeln!(f, "    Type:           {}", self.device_type)?;
        writeln!(f, "    Vendor ID:      {:#06X}", self.vendor_id)?;
        writeln!(f, "    Device ID:      {:#06X}", self.device_id)?;
        writeln!(f, "    API Version:    {}", self.api_version)?;
        write!(f, "    Driver Version: {}", self.driver_version)
    }
}
