use ash::vk::{ColorComponentFlags, PipelineColorBlendAttachmentState, SampleCountFlags};

use crate::renderers::settings::{ColorBlending, CullingMode, Msaa};

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

impl From<crate::renderers::settings::PrimitiveTopology> for ash::vk::PrimitiveTopology {
    fn from(value: crate::renderers::settings::PrimitiveTopology) -> Self {
        match value {
            crate::renderers::settings::PrimitiveTopology::PointList => Self::POINT_LIST,
            crate::renderers::settings::PrimitiveTopology::LineList => Self::LINE_LIST,
            crate::renderers::settings::PrimitiveTopology::LineStrip => Self::LINE_STRIP,
            crate::renderers::settings::PrimitiveTopology::TriangleList => Self::TRIANGLE_LIST,
            crate::renderers::settings::PrimitiveTopology::TriangleStrip => Self::TRIANGLE_STRIP,
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

impl From<crate::renderers::settings::FrontFace> for ash::vk::FrontFace {
    fn from(value: crate::renderers::settings::FrontFace) -> Self {
        match value {
            crate::renderers::settings::FrontFace::Clockwise => Self::CLOCKWISE,
            crate::renderers::settings::FrontFace::CounterClockwise => Self::COUNTER_CLOCKWISE,
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
            ash::vk::PrimitiveTopology::from(
                crate::renderers::settings::PrimitiveTopology::PointList
            ),
            ash::vk::PrimitiveTopology::POINT_LIST
        );
        assert_eq!(
            ash::vk::PrimitiveTopology::from(
                crate::renderers::settings::PrimitiveTopology::LineList
            ),
            ash::vk::PrimitiveTopology::LINE_LIST
        );
        assert_eq!(
            ash::vk::PrimitiveTopology::from(
                crate::renderers::settings::PrimitiveTopology::LineStrip
            ),
            ash::vk::PrimitiveTopology::LINE_STRIP
        );
        assert_eq!(
            ash::vk::PrimitiveTopology::from(
                crate::renderers::settings::PrimitiveTopology::TriangleList
            ),
            ash::vk::PrimitiveTopology::TRIANGLE_LIST
        );
        assert_eq!(
            ash::vk::PrimitiveTopology::from(
                crate::renderers::settings::PrimitiveTopology::TriangleStrip
            ),
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
            ash::vk::FrontFace::from(crate::renderers::settings::FrontFace::Clockwise),
            ash::vk::FrontFace::CLOCKWISE
        );
        assert_eq!(
            ash::vk::FrontFace::from(crate::renderers::settings::FrontFace::CounterClockwise),
            ash::vk::FrontFace::COUNTER_CLOCKWISE
        );
    }
}
