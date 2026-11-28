use ash::vk::SampleCountFlags;

use crate::renderers::Msaa;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msaa() {
        assert_eq!(SampleCountFlags::from(Msaa::X1), SampleCountFlags::TYPE_1);
        assert_eq!(SampleCountFlags::from(Msaa::X2), SampleCountFlags::TYPE_2);
        assert_eq!(SampleCountFlags::from(Msaa::X4), SampleCountFlags::TYPE_4);
        assert_eq!(SampleCountFlags::from(Msaa::X8), SampleCountFlags::TYPE_8);
        assert_eq!(SampleCountFlags::from(Msaa::X16), SampleCountFlags::TYPE_16);
        assert_eq!(SampleCountFlags::from(Msaa::X32), SampleCountFlags::TYPE_32);
        assert_eq!(SampleCountFlags::from(Msaa::X64), SampleCountFlags::TYPE_64);
    }
}
