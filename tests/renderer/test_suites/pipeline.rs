use heph_gl::{
    Error,
    GraphicsDeviceFeature::WireframeMode,
    Shader,
    renderers::{
        FeatureRequest, GraphicsPipelineOptions, PolygonMode, Renderer, RendererComputePipeline,
        RendererGraphicsPipeline, RendererHandle, RendererPipeline, RendererWorkerFactory,
    },
};

use crate::{heph_expect_err, heph_expect_success, renderer::RendererTests, utils::SHADERS_DIR};

struct ForeignComputePipeline;
impl RendererPipeline for ForeignComputePipeline {
    fn is_allocated(&self) -> bool {
        false
    }
}
impl RendererComputePipeline for ForeignComputePipeline {}

struct ForeignGraphicsPipeline;
impl RendererPipeline for ForeignGraphicsPipeline {
    fn is_allocated(&self) -> bool {
        false
    }
}
impl RendererGraphicsPipeline for ForeignGraphicsPipeline {}

impl<TestRenderer: Renderer> RendererTests<TestRenderer>
where
    RendererHandle<TestRenderer>: RendererWorkerFactory<TestRenderer>,
{
    pub(crate) fn test_compute_pipeline(&self) {
        let renderer = self.create_renderer_with_any_device(&[]);
        let shader =
            heph_expect_success!(Shader::from_file(format!("{}/addition.spv", SHADERS_DIR)));
        // TODO: `create_compute_pipeline` should take a mutable self.
        let mut pipeline = heph_expect_success!(renderer.create_compute_pipeline(&shader));
        assert!(pipeline.as_ref().is_allocated());
        heph_expect_success!(renderer.destroy_compute_pipeline(pipeline.as_mut()));
        assert!(!pipeline.as_ref().is_allocated());
    }

    pub(crate) fn test_graphics_pipeline(&self) {
        {
            let renderer = self.create_renderer_with_any_device(&[]);
            let frag_shader =
                heph_expect_success!(Shader::from_file(format!("{}/mesh_frag.spv", SHADERS_DIR)));
            let vert_shader = heph_expect_success!(Shader::from_file(format!(
                "{}/camera_vert.spv",
                SHADERS_DIR
            )));
            let mut pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
                &[&frag_shader, &vert_shader],
                &GraphicsPipelineOptions::default()
            ));
            assert!(pipeline.as_ref().is_allocated());
            heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
            assert!(!pipeline.as_ref().is_allocated());
        }

        {
            let renderer =
                self.create_renderer_with_any_device(&[FeatureRequest::new(WireframeMode, false)]);
            if renderer
                .get_device()
                .unwrap()
                .supported_features
                .contains(&WireframeMode)
            {
                let frag_shader = heph_expect_success!(Shader::from_file(format!(
                    "{}/mesh_frag.spv",
                    SHADERS_DIR
                )));
                let vert_shader = heph_expect_success!(Shader::from_file(format!(
                    "{}/camera_vert.spv",
                    SHADERS_DIR
                )));
                for polygon_mode in [PolygonMode::Fill, PolygonMode::Line, PolygonMode::Point] {
                    let mut pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
                        &[&frag_shader, &vert_shader],
                        &GraphicsPipelineOptions {
                            polygon_mode,
                            ..Default::default()
                        }
                    ));
                    heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
                }
            }
        }

        {
            let renderer = self.create_renderer_with_any_device(&[]);
            let frag_shader = heph_expect_success!(Shader::from_file(format!(
                "{}/shapes/basic_triangle_frag.spv",
                SHADERS_DIR
            )));
            let vert_shader = heph_expect_success!(Shader::from_file(format!(
                "{}/shapes/basic_triangle_vert.spv",
                SHADERS_DIR
            )));
            for polygon_mode in [PolygonMode::Line, PolygonMode::Point] {
                heph_expect_err!(
                    renderer.create_graphics_pipeline(
                        &[&frag_shader, &vert_shader],
                        &GraphicsPipelineOptions {
                            polygon_mode,
                            ..Default::default()
                        }
                    ),
                    Error::invalid_operation("")
                );
            }
        }
    }

    pub(crate) fn test_reject_foreign_pipelines(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);
        let mut compute_pipeline: Box<dyn RendererComputePipeline> =
            Box::new(ForeignComputePipeline);
        let mut graphics_pipeline: Box<dyn RendererGraphicsPipeline> =
            Box::new(ForeignGraphicsPipeline);

        heph_expect_err!(
            renderer.record_compute_command(compute_pipeline.as_mut(), &[], (1, 1, 1)),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            renderer.destroy_compute_pipeline(compute_pipeline.as_mut()),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            renderer.record_graphics_command(graphics_pipeline.as_mut(), None, &[], &[], 3, 1),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            renderer.destroy_graphics_pipeline(graphics_pipeline.as_mut()),
            Error::invalid_argument("")
        );
    }
}
