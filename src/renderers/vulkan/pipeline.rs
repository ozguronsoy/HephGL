use ash::vk::{
    ColorComponentFlags, DescriptorSetLayout, Handle, Pipeline, PipelineColorBlendAttachmentState,
    PipelineLayout,
};

use crate::{
    renderers::{
        RendererComputePipeline, RendererGraphicsPipeline, RendererPipeline, TextureFormat,
        debug::RendererDebugObject,
        id::RendererId,
        pipeline::{ColorBlending, CullingMode, FrontFace, PolygonMode, PrimitiveTopology},
        vulkan::resources::DescriptorSetCache,
    },
    shader::ShaderStage,
};

/// Represents a Vulkan compute pipeline.
#[derive(Clone)]
pub(super) struct VulkanComputePipeline {
    pub(super) renderer_id: RendererId,
    pub(super) pipeline: Pipeline,
    pub(super) layout: PipelineLayout,
    pub(super) descriptor_layouts: Vec<DescriptorSetLayout>,
    pub(super) descriptor_binding_counts: Vec<usize>,
    pub(super) descriptor_set_cache: DescriptorSetCache,
}

/// Represents a Vulkan graphics pipeline.
#[derive(Clone)]
pub(super) struct VulkanGraphicsPipeline {
    pub(super) renderer_id: RendererId,
    pub(super) pipeline: Pipeline,
    pub(super) layout: PipelineLayout,
    pub(super) descriptor_layouts: Vec<DescriptorSetLayout>,
    pub(super) descriptor_binding_counts: Vec<usize>,
    pub(super) descriptor_set_cache: DescriptorSetCache,
    pub(super) depth_only: bool,
    pub(super) color_formats: Vec<TextureFormat>,
    pub(super) mesh_stage: Option<ShaderStage>,
}

impl RendererDebugObject for VulkanGraphicsPipeline {}
impl RendererPipeline for VulkanGraphicsPipeline {
    fn is_allocated(&self) -> bool {
        !(self.pipeline.is_null() || self.layout.is_null())
    }
}
impl RendererGraphicsPipeline for VulkanGraphicsPipeline {
    fn clone_pipeline(&self) -> Box<dyn RendererGraphicsPipeline> {
        Box::new(self.clone())
    }
}

impl RendererDebugObject for VulkanComputePipeline {}
impl RendererPipeline for VulkanComputePipeline {
    fn is_allocated(&self) -> bool {
        !(self.pipeline.is_null() || self.layout.is_null())
    }
}
impl RendererComputePipeline for VulkanComputePipeline {
    fn clone_pipeline(&self) -> Box<dyn RendererComputePipeline> {
        Box::new(self.clone())
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
            PrimitiveTopology::PatchList { .. } => Self::PATCH_LIST,
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

impl From<PolygonMode> for ash::vk::PolygonMode {
    fn from(value: PolygonMode) -> Self {
        match value {
            PolygonMode::Fill => Self::FILL,
            PolygonMode::Line => Self::LINE,
            PolygonMode::Point => Self::POINT,
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_polygon_mode() {
        assert_eq!(
            ash::vk::PolygonMode::from(PolygonMode::Fill),
            ash::vk::PolygonMode::FILL
        );
        assert_eq!(
            ash::vk::PolygonMode::from(PolygonMode::Line),
            ash::vk::PolygonMode::LINE
        );
        assert_eq!(
            ash::vk::PolygonMode::from(PolygonMode::Point),
            ash::vk::PolygonMode::POINT
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
}
