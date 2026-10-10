use ash::vk::{ColorComponentFlags, PipelineColorBlendAttachmentState, SampleCountFlags};

use crate::renderers::{
    ColorBlending, CullingMode, Msaa, SamplerAddressMode, SamplerFilter,
    settings::{FrontFace, PrimitiveTopology},
};

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

impl From<ColorBlending> for PipelineColorBlendAttachmentState {
    fn from(value: ColorBlending) -> Self {
        match value {
            ColorBlending::Disabled => PipelineColorBlendAttachmentState::default()
                .blend_enable(false)
                .color_write_mask(
                    ColorComponentFlags::R
                        | ColorComponentFlags::G
                        | ColorComponentFlags::B
                        | ColorComponentFlags::A,
                ),

            ColorBlending::Alpha => PipelineColorBlendAttachmentState::default()
                .blend_enable(true)
                .src_color_blend_factor(ash::vk::BlendFactor::SRC_ALPHA)
                .dst_color_blend_factor(ash::vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_blend_op(ash::vk::BlendOp::ADD)
                .src_alpha_blend_factor(ash::vk::BlendFactor::ONE)
                .dst_alpha_blend_factor(ash::vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .alpha_blend_op(ash::vk::BlendOp::ADD)
                .color_write_mask(
                    ColorComponentFlags::R
                        | ColorComponentFlags::G
                        | ColorComponentFlags::B
                        | ColorComponentFlags::A,
                ),
        }
    }
}

impl From<PrimitiveTopology> for ash::vk::PrimitiveTopology {
    fn from(value: PrimitiveTopology) -> Self {
        match value {
            PrimitiveTopology::PointList => Self::POINT_LIST,
            PrimitiveTopology::LineList => Self::LINE_LIST,
            PrimitiveTopology::LineStrip => Self::LINE_STRIP,
            PrimitiveTopology::TriangleList => Self::TRIANGLE_LIST,
            PrimitiveTopology::TriangleStrip => Self::TRIANGLE_STRIP,
        }
    }
}

impl From<CullingMode> for ash::vk::CullModeFlags {
    fn from(value: CullingMode) -> Self {
        match value {
            CullingMode::Disabled => Self::NONE,
            CullingMode::Front => Self::FRONT,
            CullingMode::Back => Self::BACK,
        }
    }
}

impl From<FrontFace> for ash::vk::FrontFace {
    fn from(value: FrontFace) -> Self {
        match value {
            FrontFace::Clockwise => Self::CLOCKWISE,
            FrontFace::CounterClockwise => Self::COUNTER_CLOCKWISE,
        }
    }
}

impl From<SamplerFilter> for ash::vk::Filter {
    fn from(value: SamplerFilter) -> Self {
        match value {
            SamplerFilter::Nearest => Self::NEAREST,
            SamplerFilter::Linear => Self::LINEAR,
        }
    }
}

impl From<SamplerFilter> for ash::vk::SamplerMipmapMode {
    fn from(value: SamplerFilter) -> Self {
        match value {
            SamplerFilter::Nearest => Self::NEAREST,
            SamplerFilter::Linear => Self::LINEAR,
        }
    }
}

impl From<SamplerAddressMode> for ash::vk::SamplerAddressMode {
    fn from(value: SamplerAddressMode) -> Self {
        match value {
            SamplerAddressMode::Repeat => Self::REPEAT,
            SamplerAddressMode::MirroredRepeat => Self::MIRRORED_REPEAT,
            SamplerAddressMode::ClampToEdge => Self::CLAMP_TO_EDGE,
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

    #[test]
    fn test_topology() {
        assert_eq!(
            ash::vk::PrimitiveTopology::from(PrimitiveTopology::PointList),
            ash::vk::PrimitiveTopology::POINT_LIST
        );
        assert_eq!(
            ash::vk::PrimitiveTopology::from(PrimitiveTopology::LineList),
            ash::vk::PrimitiveTopology::LINE_LIST
        );
        assert_eq!(
            ash::vk::PrimitiveTopology::from(PrimitiveTopology::LineStrip),
            ash::vk::PrimitiveTopology::LINE_STRIP
        );
        assert_eq!(
            ash::vk::PrimitiveTopology::from(PrimitiveTopology::TriangleList),
            ash::vk::PrimitiveTopology::TRIANGLE_LIST
        );
        assert_eq!(
            ash::vk::PrimitiveTopology::from(PrimitiveTopology::TriangleStrip),
            ash::vk::PrimitiveTopology::TRIANGLE_STRIP
        );
    }

    #[test]
    fn test_culling() {
        assert_eq!(
            ash::vk::CullModeFlags::from(CullingMode::Disabled),
            ash::vk::CullModeFlags::NONE
        );
        assert_eq!(
            ash::vk::CullModeFlags::from(CullingMode::Front),
            ash::vk::CullModeFlags::FRONT
        );
        assert_eq!(
            ash::vk::CullModeFlags::from(CullingMode::Back),
            ash::vk::CullModeFlags::BACK
        );
    }

    #[test]
    fn test_front_face() {
        assert_eq!(
            ash::vk::FrontFace::from(FrontFace::Clockwise),
            ash::vk::FrontFace::CLOCKWISE
        );
        assert_eq!(
            ash::vk::FrontFace::from(FrontFace::CounterClockwise),
            ash::vk::FrontFace::COUNTER_CLOCKWISE
        );
    }

    #[test]
    fn test_sampler_filter() {
        assert_eq!(
            ash::vk::Filter::from(SamplerFilter::Nearest),
            ash::vk::Filter::NEAREST
        );
        assert_eq!(
            ash::vk::Filter::from(SamplerFilter::Linear),
            ash::vk::Filter::LINEAR
        );
        assert_eq!(
            ash::vk::SamplerMipmapMode::from(SamplerFilter::Nearest),
            ash::vk::SamplerMipmapMode::NEAREST
        );
        assert_eq!(
            ash::vk::SamplerMipmapMode::from(SamplerFilter::Linear),
            ash::vk::SamplerMipmapMode::LINEAR
        );
    }

    #[test]
    fn test_sampler_address_mode() {
        assert_eq!(
            ash::vk::SamplerAddressMode::from(SamplerAddressMode::Repeat),
            ash::vk::SamplerAddressMode::REPEAT
        );
        assert_eq!(
            ash::vk::SamplerAddressMode::from(SamplerAddressMode::MirroredRepeat),
            ash::vk::SamplerAddressMode::MIRRORED_REPEAT
        );
        assert_eq!(
            ash::vk::SamplerAddressMode::from(SamplerAddressMode::ClampToEdge),
            ash::vk::SamplerAddressMode::CLAMP_TO_EDGE
        );
    }
}
