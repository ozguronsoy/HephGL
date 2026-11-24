use heph_gl::{
    Error,
    GraphicsDeviceFeature::{
        DepthBiasClamp, DepthClamp, GeometryShaders, SampleRateShading, TessellationShaders,
        WireframeMode,
    },
    Shader,
    renderers::{
        DISPLAY_CANVAS, DepthBias, FeatureRequest, GraphicsPipelineOptions, PolygonMode,
        PrimitiveTopology, Renderer, RendererComputePipeline, RendererGraphicsPipeline,
        RendererPipeline, debug::RendererDebugObject,
    },
};

use crate::{heph_expect_err, heph_expect_success, renderer::RendererTests, utils::SHADERS_DIR};

struct ForeignComputePipeline;
impl RendererDebugObject for ForeignComputePipeline {}
impl RendererPipeline for ForeignComputePipeline {
    fn is_allocated(&self) -> bool {
        false
    }
}
impl RendererComputePipeline for ForeignComputePipeline {
    fn clone_pipeline(&self) -> Box<dyn RendererComputePipeline> {
        Box::new(ForeignComputePipeline)
    }
}

struct ForeignGraphicsPipeline;
impl RendererDebugObject for ForeignGraphicsPipeline {}
impl RendererPipeline for ForeignGraphicsPipeline {
    fn is_allocated(&self) -> bool {
        false
    }
}
impl RendererGraphicsPipeline for ForeignGraphicsPipeline {
    fn clone_pipeline(&self) -> Box<dyn RendererGraphicsPipeline> {
        Box::new(ForeignGraphicsPipeline)
    }
}

impl<TestRenderer: Renderer> RendererTests<TestRenderer> {
    pub(crate) fn test_compute_pipeline(&self) {
        let renderer = self.create_renderer_with_any_device(&[]);
        let shader = heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/addition.spv")));
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
                heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/mesh_frag.spv")));
            let vert_shader =
                heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/camera_vert.spv")));
            let mut pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
                &[&frag_shader, &vert_shader],
                &GraphicsPipelineOptions::default()
            ));
            assert!(pipeline.as_ref().is_allocated());
            heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
            assert!(!pipeline.as_ref().is_allocated());
        }

        {
            let renderer = self.create_renderer_with_any_device(&[]);
            let frag_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/shapes/basic_triangle_frag.spv"
            )));
            let vert_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/shapes/basic_triangle_vert.spv"
            )));
            heph_expect_err!(
                renderer.create_graphics_pipeline(
                    &[&frag_shader, &vert_shader, &vert_shader],
                    &GraphicsPipelineOptions::default()
                ),
                Error::InvalidArgument(_)
            );
        }

        {
            let renderer = self.create_renderer_with_any_device(&[]);
            let frag_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/shapes/basic_triangle_frag.spv"
            )));
            let vert_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/shapes/basic_triangle_vert.spv"
            )));
            let comp_shader =
                heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/addition.spv")));
            heph_expect_err!(
                renderer.create_graphics_pipeline(
                    &[&frag_shader, &vert_shader, &comp_shader],
                    &GraphicsPipelineOptions::default()
                ),
                Error::InvalidArgument(_)
            );
        }

        {
            let renderer =
                self.create_renderer_with_any_device(&[FeatureRequest::new(WireframeMode, false)]);
            if renderer
                .get_device()
                .unwrap()
                .supported_features()
                .contains(&WireframeMode)
            {
                let frag_shader =
                    heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/mesh_frag.spv")));
                let vert_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/camera_vert.spv"
                )));
                for polygon_mode in [PolygonMode::Fill, PolygonMode::Line] {
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
                "{SHADERS_DIR}/shapes/basic_triangle_frag.spv"
            )));
            let vert_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/shapes/basic_triangle_vert.spv"
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
                    Error::InvalidOperation(_)
                );
            }
        }

        {
            let renderer = self.create_renderer_with_any_device(&[]);
            let frag_shader =
                heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/mesh_frag.spv")));
            let geom_shader =
                heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/explode_geom.spv")));
            let vert_shader =
                heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/camera_vert.spv")));
            heph_expect_err!(
                renderer.create_graphics_pipeline(
                    &[&frag_shader, &geom_shader, &vert_shader],
                    &GraphicsPipelineOptions::default()
                ),
                Error::InvalidOperation(_)
            );
        }

        {
            let renderer = self
                .create_renderer_with_any_device(&[FeatureRequest::new(GeometryShaders, false)]);
            if renderer
                .get_device()
                .unwrap()
                .supported_features()
                .contains(&GeometryShaders)
            {
                let frag_shader =
                    heph_expect_success!(Shader::from_file(format!("{SHADERS_DIR}/mesh_frag.spv")));
                let geom_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/explode_geom.spv"
                )));
                let vert_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/camera_vert.spv"
                )));
                for topology in [
                    PrimitiveTopology::TriangleList,
                    PrimitiveTopology::TriangleStrip,
                ] {
                    let mut pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
                        &[&frag_shader, &geom_shader, &vert_shader],
                        &GraphicsPipelineOptions {
                            topology,
                            ..Default::default()
                        }
                    ));
                    heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
                }
                for topology in [
                    PrimitiveTopology::PointList,
                    PrimitiveTopology::LineList,
                    PrimitiveTopology::LineStrip,
                ] {
                    heph_expect_err!(
                        renderer.create_graphics_pipeline(
                            &[&frag_shader, &geom_shader, &vert_shader],
                            &GraphicsPipelineOptions {
                                topology,
                                ..Default::default()
                            }
                        ),
                        Error::InvalidArgument(_)
                    );
                }
                heph_expect_err!(
                    renderer.create_graphics_pipeline(
                        &[&frag_shader, &geom_shader, &geom_shader, &vert_shader],
                        &GraphicsPipelineOptions::default()
                    ),
                    Error::InvalidArgument(_)
                );
            }
        }

        {
            let renderer = self.create_renderer_with_any_device(&[]);
            let frag_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/broken_brick_wall_frag.spv"
            )));
            let tese_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/broken_brick_wall_tese.spv"
            )));
            let tesc_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/broken_brick_wall_tesc.spv"
            )));
            let vert_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/broken_brick_wall_vert.spv"
            )));
            heph_expect_err!(
                renderer.create_graphics_pipeline(
                    &[&frag_shader, &tese_shader, &tesc_shader, &vert_shader],
                    &GraphicsPipelineOptions {
                        topology: PrimitiveTopology::PatchList { control_points: 3 },
                        ..Default::default()
                    }
                ),
                Error::InvalidOperation(_)
            );
        }

        {
            let renderer = self.create_renderer_with_any_device(&[FeatureRequest::new(
                TessellationShaders,
                false,
            )]);
            if renderer
                .get_device()
                .unwrap()
                .supported_features()
                .contains(&TessellationShaders)
            {
                let frag_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/broken_brick_wall_frag.spv"
                )));
                let tese_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/broken_brick_wall_tese.spv"
                )));
                let tesc_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/broken_brick_wall_tesc.spv"
                )));
                let vert_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/broken_brick_wall_vert.spv"
                )));
                let patch_list = GraphicsPipelineOptions {
                    topology: PrimitiveTopology::PatchList { control_points: 3 },
                    ..Default::default()
                };
                let mut pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
                    &[&frag_shader, &tese_shader, &tesc_shader, &vert_shader],
                    &patch_list
                ));
                heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
                heph_expect_err!(
                    renderer.create_graphics_pipeline(&[&frag_shader, &vert_shader], &patch_list),
                    Error::InvalidArgument(_)
                );
                heph_expect_err!(
                    renderer.create_graphics_pipeline(
                        &[&frag_shader, &tesc_shader, &vert_shader],
                        &patch_list
                    ),
                    Error::InvalidArgument(_)
                );
                heph_expect_err!(
                    renderer.create_graphics_pipeline(
                        &[&frag_shader, &tese_shader, &vert_shader],
                        &patch_list
                    ),
                    Error::InvalidArgument(_)
                );
                heph_expect_err!(
                    renderer.create_graphics_pipeline(
                        &[
                            &frag_shader,
                            &tese_shader,
                            &tesc_shader,
                            &tesc_shader,
                            &vert_shader
                        ],
                        &patch_list
                    ),
                    Error::InvalidArgument(_)
                );
                heph_expect_err!(
                    renderer.create_graphics_pipeline(
                        &[&frag_shader, &tese_shader, &tesc_shader, &vert_shader],
                        &GraphicsPipelineOptions::default()
                    ),
                    Error::InvalidArgument(_)
                );
                for control_points in [0, u32::MAX] {
                    heph_expect_err!(
                        renderer.create_graphics_pipeline(
                            &[&frag_shader, &tese_shader, &tesc_shader, &vert_shader],
                            &GraphicsPipelineOptions {
                                topology: PrimitiveTopology::PatchList { control_points },
                                ..Default::default()
                            }
                        ),
                        Error::InvalidArgument(_)
                    );
                }
            }
        }

        {
            let renderer = self.create_renderer_with_any_device(&[
                FeatureRequest::new(TessellationShaders, false),
                FeatureRequest::new(GeometryShaders, false),
            ]);
            let supported_features = &renderer.get_device().unwrap().supported_features();
            if supported_features.contains(&TessellationShaders)
                && supported_features.contains(&GeometryShaders)
            {
                let frag_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/broken_brick_wall_frag.spv"
                )));
                let triangles_geom_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/pass_through_triangles_geom.spv"
                )));
                let points_geom_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/pass_through_points_geom.spv"
                )));
                let tese_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/broken_brick_wall_tese.spv"
                )));
                let tesc_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/broken_brick_wall_tesc.spv"
                )));
                let vert_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/broken_brick_wall_vert.spv"
                )));
                let patch_list = GraphicsPipelineOptions {
                    topology: PrimitiveTopology::PatchList { control_points: 3 },
                    ..Default::default()
                };
                let mut pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
                    &[
                        &frag_shader,
                        &triangles_geom_shader,
                        &tese_shader,
                        &tesc_shader,
                        &vert_shader
                    ],
                    &patch_list
                ));
                heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
                heph_expect_err!(
                    renderer.create_graphics_pipeline(
                        &[
                            &frag_shader,
                            &points_geom_shader,
                            &tese_shader,
                            &tesc_shader,
                            &vert_shader
                        ],
                        &patch_list
                    ),
                    Error::InvalidArgument(_)
                );
            }
        }

        {
            let renderer = self.create_renderer_with_any_device(&[]);
            let frag_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/shapes/basic_triangle_frag.spv"
            )));
            let vert_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/shapes/basic_triangle_vert.spv"
            )));
            heph_expect_err!(
                renderer.create_graphics_pipeline(
                    &[&frag_shader, &vert_shader],
                    &GraphicsPipelineOptions {
                        min_sample_shading: Some(1.0),
                        ..Default::default()
                    }
                ),
                Error::InvalidOperation(_)
            );
        }

        {
            let renderer = self
                .create_renderer_with_any_device(&[FeatureRequest::new(SampleRateShading, false)]);
            if renderer
                .get_device()
                .unwrap()
                .supported_features()
                .contains(&SampleRateShading)
            {
                let frag_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/shapes/basic_triangle_frag.spv"
                )));
                let vert_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/shapes/basic_triangle_vert.spv"
                )));
                for min_sample_shading in [0.0, 0.5, 1.0] {
                    let mut pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
                        &[&frag_shader, &vert_shader],
                        &GraphicsPipelineOptions {
                            min_sample_shading: Some(min_sample_shading),
                            ..Default::default()
                        }
                    ));
                    heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
                }
                for min_sample_shading in [-0.1, 1.1, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                    heph_expect_err!(
                        renderer.create_graphics_pipeline(
                            &[&frag_shader, &vert_shader],
                            &GraphicsPipelineOptions {
                                min_sample_shading: Some(min_sample_shading),
                                ..Default::default()
                            }
                        ),
                        Error::InvalidArgument(_)
                    );
                }
            }
        }

        {
            let renderer =
                self.create_renderer_with_any_device(&[FeatureRequest::new(DepthClamp, false)]);
            if renderer
                .get_device()
                .unwrap()
                .supported_features()
                .contains(&DepthClamp)
            {
                let frag_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/shapes/basic_triangle_frag.spv"
                )));
                let vert_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/shapes/basic_triangle_vert.spv"
                )));
                let mut pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
                    &[&frag_shader, &vert_shader],
                    &GraphicsPipelineOptions {
                        depth_clamp: true,
                        ..Default::default()
                    }
                ));
                heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
            }
        }

        {
            let renderer = self.create_renderer_with_any_device(&[]);
            let frag_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/shapes/basic_triangle_frag.spv"
            )));
            let vert_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/shapes/basic_triangle_vert.spv"
            )));
            heph_expect_err!(
                renderer.create_graphics_pipeline(
                    &[&frag_shader, &vert_shader],
                    &GraphicsPipelineOptions {
                        depth_clamp: true,
                        ..Default::default()
                    }
                ),
                Error::InvalidOperation(_)
            );
        }

        {
            let renderer =
                self.create_renderer_with_any_device(&[FeatureRequest::new(DepthBiasClamp, false)]);
            if renderer
                .get_device()
                .unwrap()
                .supported_features()
                .contains(&DepthBiasClamp)
            {
                let frag_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/shapes/basic_triangle_frag.spv"
                )));
                let vert_shader = heph_expect_success!(Shader::from_file(format!(
                    "{SHADERS_DIR}/shapes/basic_triangle_vert.spv"
                )));
                let mut pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
                    &[&frag_shader, &vert_shader],
                    &GraphicsPipelineOptions {
                        depth_bias: Some(DepthBias {
                            constant: 1.0,
                            slope: 1.0,
                            clamp: Some(0.01),
                        }),
                        ..Default::default()
                    }
                ));
                heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
            }
        }

        {
            let renderer = self.create_renderer_with_any_device(&[]);
            let frag_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/shapes/basic_triangle_frag.spv"
            )));
            let vert_shader = heph_expect_success!(Shader::from_file(format!(
                "{SHADERS_DIR}/shapes/basic_triangle_vert.spv"
            )));
            heph_expect_err!(
                renderer.create_graphics_pipeline(
                    &[&frag_shader, &vert_shader],
                    &GraphicsPipelineOptions {
                        depth_bias: Some(DepthBias {
                            constant: 1.0,
                            slope: 1.0,
                            clamp: Some(0.01),
                        }),
                        ..Default::default()
                    }
                ),
                Error::InvalidOperation(_)
            );
        }
    }

    pub(crate) fn test_reject_foreign_pipelines(&self) {
        let renderer = self.create_renderer_with_any_device(&[]);
        let mut compute_pipeline: Box<dyn RendererComputePipeline> =
            Box::new(ForeignComputePipeline);
        let mut graphics_pipeline: Box<dyn RendererGraphicsPipeline> =
            Box::new(ForeignGraphicsPipeline);

        heph_expect_success!(renderer.begin_frame());
        heph_expect_err!(
            renderer.record_compute_command(compute_pipeline.as_mut(), &[], (1, 1, 1)),
            Error::InvalidArgument(_)
        );
        heph_expect_err!(
            renderer.destroy_compute_pipeline(compute_pipeline.as_mut()),
            Error::InvalidArgument(_)
        );
        heph_expect_err!(
            renderer.record_graphics_command(
                DISPLAY_CANVAS,
                graphics_pipeline.as_mut(),
                None,
                &[],
                &[],
                3,
                1
            ),
            Error::InvalidArgument(_)
        );
        heph_expect_err!(
            renderer.destroy_graphics_pipeline(graphics_pipeline.as_mut()),
            Error::InvalidArgument(_)
        );
        heph_expect_success!(renderer.end_frame());
    }
}
