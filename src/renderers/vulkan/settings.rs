use ash::vk::SampleCountFlags;

use crate::renderers::settings::Msaa;

impl From<Msaa> for SampleCountFlags {
    fn from(value: Msaa) -> Self {
        match value {
            Msaa::X1 => SampleCountFlags::TYPE_1,
            Msaa::X2 => SampleCountFlags::TYPE_2,
            Msaa::X4 => SampleCountFlags::TYPE_4,
            Msaa::X8 => SampleCountFlags::TYPE_8,
            Msaa::X16 => SampleCountFlags::TYPE_16,
            Msaa::X32 => SampleCountFlags::TYPE_32,
            Msaa::X64 => SampleCountFlags::TYPE_64,
        }
    }
}
