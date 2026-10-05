use ash::vk::{ColorComponentFlags, PipelineColorBlendAttachmentState, SampleCountFlags};

use crate::renderers::settings::{ColorBlending, Msaa};

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
