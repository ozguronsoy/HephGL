use std::{ffi::CString, mem::DropGuard};

use ash::vk::{
    AttachmentDescription, AttachmentLoadOp, AttachmentReference, AttachmentStoreOp, CommandBuffer,
    DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo, DescriptorType, DynamicState,
    FormatFeatureFlags, FramebufferCreateInfo, GraphicsPipelineCreateInfo, PipelineBindPoint,
    PipelineCache, PipelineColorBlendStateCreateInfo, PipelineDepthStencilStateCreateInfo,
    PipelineDynamicStateCreateInfo, PipelineInputAssemblyStateCreateInfo, PipelineLayoutCreateInfo,
    PipelineMultisampleStateCreateInfo, PipelineRasterizationStateCreateInfo,
    PipelineShaderStageCreateInfo, PipelineTessellationStateCreateInfo,
    PipelineVertexInputStateCreateInfo, PipelineViewportStateCreateInfo, PrimitiveTopology,
    RenderPass, RenderPassBeginInfo, RenderPassCreateInfo, SampleCountFlags,
    ShaderModuleCreateInfo, ShaderStageFlags, SubpassContents, SubpassDescription,
    VertexInputAttributeDescription, VertexInputBindingDescription, VertexInputRate,
};

use crate::{
    Error, Result,
    graphics_device::GraphicsDeviceFeature,
    renderers::{
        RendererCanvas, TextureFormat,
        settings::{ColorBlending, GraphicsPipelineOptions, Msaa, PolygonMode, Settings},
        vulkan::{
            VulkanRenderer,
            canvas::{VulkanColorCanvas, VulkanDepthCanvas},
            device::DeviceContext,
            pipeline::VulkanGraphicsPipeline,
            rendering::VulkanRendering,
            resources::DescriptorSetCache,
        },
    },
    shader::{ScalarKind, Shader, ShaderPrimitive, ShaderStage},
};

/// Implements rendering via render pass.
pub(super) struct RenderPassRendering {
    msaa: Msaa,
}

impl VulkanRendering for RenderPassRendering {
    fn new(settings: &Settings, _: &ash::Device) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            msaa: settings.msaa,
        })
    }
    fn create_graphics_pipeline(
        &self,
        instance: &ash::Instance,
        device_context: &DeviceContext,
        shaders: &[&Shader],
        options: &GraphicsPipelineOptions,
    ) -> Result<VulkanGraphicsPipeline> {
        #[derive(PartialEq)]
        enum PipelineType {
            None,
            Vertex,
            Mesh,
        }
        let device = &device_context.logical_device;
        let enabled_features = &device_context.enabled_features;
        let max_tessellation_patch_size = device_context.max_tessellation_patch_size;
        if options.polygon_mode != PolygonMode::Fill
            && !enabled_features.contains(&GraphicsDeviceFeature::WireframeMode)
        {
            return Err(Error::invalid_operation(
                "Non-fill polygon modes require the `WireframeMode` feature to be enabled.",
            ));
        }
        if let crate::renderers::PrimitiveTopology::PatchList { control_points } = options.topology
            && (control_points == 0 || control_points > max_tessellation_patch_size)
        {
            return Err(Error::invalid_argument(
                "The number of patch control points is not supported by the device.",
            ));
        }
        if let Some(min_sample_shading) = options.min_sample_shading {
            if !enabled_features.contains(&GraphicsDeviceFeature::SampleRateShading) {
                return Err(Error::invalid_operation(
                    "Sample rate shading requires the `SampleRateShading` feature to be enabled.",
                ));
            }
            if !(min_sample_shading >= 0.0 && min_sample_shading <= 1.0) {
                return Err(Error::invalid_argument(
                    "Minimum sample shading must be between 0.0 and 1.0.",
                ));
            }
        }
        if options.depth_clamp && !enabled_features.contains(&GraphicsDeviceFeature::DepthClamp) {
            return Err(Error::invalid_operation(
                "Depth clamping requires the `DepthClamp` feature to be enabled.",
            ));
        }
        if options
            .depth_bias
            .is_some_and(|depth_bias| depth_bias.clamp.is_some())
            && !enabled_features.contains(&GraphicsDeviceFeature::DepthBiasClamp)
        {
            return Err(Error::invalid_operation(
                "Depth bias clamping requires the `DepthBiasClamp` feature to be enabled.",
            ));
        }
        let color_formats = match options.depth_only {
            true => Vec::new(),
            false => VulkanRenderer::resolve_texture_formats(
                instance,
                device_context,
                &options.texture_formats,
                match options.blending {
                    ColorBlending::Disabled => FormatFeatureFlags::COLOR_ATTACHMENT,
                    _ => {
                        FormatFeatureFlags::COLOR_ATTACHMENT
                            | FormatFeatureFlags::COLOR_ATTACHMENT_BLEND
                    }
                },
            )?,
        };

        // Create shader stage infos.
        let mut shader_modules =
            DropGuard::new(Vec::with_capacity(shaders.len()), |shader_modules| unsafe {
                for shader_module in shader_modules {
                    device.destroy_shader_module(shader_module, None);
                }
            });
        let mut entry_names = Vec::with_capacity(shaders.len());
        let mut shader_stages = Vec::with_capacity(shaders.len());
        let mut pipeline_type = PipelineType::None;
        let mut vertex_bindings = &[][..];
        for shader in shaders {
            let (prefix, code, suffix) = unsafe { shader.data.align_to::<u32>() };
            if !prefix.is_empty() || !suffix.is_empty() {
                return Err(Error::fail(format!(
                    "Shader data from '{}' is not valid SPIR-V (not 4-byte aligned).",
                    shader.file_path
                )));
            }

            if shader.metadata.stage == ShaderStage::Compute {
                return Err(Error::invalid_argument(
                    "Compute shaders cannot be used in a graphics pipeline.",
                ));
            }
            if shader.metadata.stage == ShaderStage::Geometry
                && !enabled_features.contains(&GraphicsDeviceFeature::GeometryShaders)
            {
                return Err(Error::invalid_operation(
                    "Geometry shaders require the `GeometryShaders` feature to be enabled.",
                ));
            }
            if matches!(
                shader.metadata.stage,
                ShaderStage::TessellationControl | ShaderStage::TessellationEvaluation
            ) && !enabled_features.contains(&GraphicsDeviceFeature::TessellationShaders)
            {
                return Err(Error::invalid_operation(
                    "Tessellation shaders require the `TessellationShaders` feature to be enabled.",
                ));
            }
            if matches!(shader.metadata.stage, ShaderStage::Task | ShaderStage::Mesh)
                && !enabled_features.contains(&GraphicsDeviceFeature::MeshShaders)
            {
                return Err(Error::invalid_operation(
                    "Task and mesh shaders require the `MeshShaders` feature to be enabled.",
                ));
            }
            let shader_stage = ShaderStageFlags::from(shader.metadata.stage);
            if shader_stages.contains(&shader_stage) {
                return Err(Error::invalid_argument(
                    "Graphics pipeline cannot contain multiple shaders of the same stage.",
                ));
            }
            match shader.metadata.stage {
                ShaderStage::Vertex | ShaderStage::Mesh if pipeline_type != PipelineType::None => {
                    return Err(Error::invalid_argument(
                        "Mesh and vertex shaders cannot be used in the same pipeline.",
                    ));
                }
                ShaderStage::Vertex => {
                    pipeline_type = PipelineType::Vertex;
                    vertex_bindings = shader.metadata.vertex_bindings.as_slice();
                }
                ShaderStage::Mesh => pipeline_type = PipelineType::Mesh,
                _ => {}
            }
            if shader.metadata.stage == ShaderStage::Geometry {
                let primitive = shaders
                    .iter()
                    .find_map(|shader| shader.metadata.output_primitive)
                    .or(match PrimitiveTopology::from(options.topology) {
                        PrimitiveTopology::POINT_LIST => Some(ShaderPrimitive::Point),
                        PrimitiveTopology::LINE_LIST | PrimitiveTopology::LINE_STRIP => {
                            Some(ShaderPrimitive::Line)
                        }
                        PrimitiveTopology::TRIANGLE_LIST | PrimitiveTopology::TRIANGLE_STRIP => {
                            Some(ShaderPrimitive::Triangle)
                        }
                        _ => None,
                    });
                if shader.metadata.input_primitive != primitive {
                    return Err(Error::invalid_argument(
                        "Geometry shader input primitive does not match the incoming primitives.",
                    ));
                }
            }
            shader_modules.push(unsafe {
                device.create_shader_module(&ShaderModuleCreateInfo::default().code(code), None)?
            });
            shader_stages.push(shader_stage);
            entry_names.push(CString::new(shader.metadata.entry_name.as_str())?);
        }
        if pipeline_type == PipelineType::None {
            return Err(Error::invalid_argument(
                "Graphics pipeline requires a vertex or mesh shader.",
            ));
        }
        if shader_stages.contains(&ShaderStageFlags::TASK_EXT)
            && pipeline_type != PipelineType::Mesh
        {
            return Err(Error::invalid_argument(
                "Task shaders must be used with a mesh shader.",
            ));
        }
        if pipeline_type == PipelineType::Mesh
            && shader_stages.iter().any(|stage| {
                stage.intersects(
                    ShaderStageFlags::TESSELLATION_CONTROL
                        | ShaderStageFlags::TESSELLATION_EVALUATION
                        | ShaderStageFlags::GEOMETRY,
                )
            })
        {
            return Err(Error::invalid_argument(
                "Mesh shaders cannot be used with tessellation or geometry shaders.",
            ));
        }
        let has_control_shader = shader_stages.contains(&ShaderStageFlags::TESSELLATION_CONTROL);
        if has_control_shader != shader_stages.contains(&ShaderStageFlags::TESSELLATION_EVALUATION)
        {
            return Err(Error::invalid_argument(
                "Tessellation control and evaluation shaders must be used together.",
            ));
        }
        if has_control_shader
            != (PrimitiveTopology::from(options.topology) == PrimitiveTopology::PATCH_LIST)
        {
            return Err(Error::invalid_argument(
                "Tessellation shaders must be used with the `PatchList` topology.",
            ));
        }
        let stage_infos = shader_stages
            .iter()
            .zip(shader_modules.iter())
            .zip(&entry_names)
            .map(|((stage, module), entry_name)| {
                PipelineShaderStageCreateInfo::default()
                    .stage(*stage)
                    .module(*module)
                    .name(entry_name)
            })
            .collect::<Vec<_>>();

        // Merge descriptor bindings used by different shader stages.
        let group_count = shaders
            .iter()
            .map(|shader| shader.descriptor_group_count())
            .max()
            .unwrap_or(0);
        let mut binding_sets = Vec::with_capacity(group_count);
        binding_sets.resize_with(group_count, Vec::new);

        for (shader, shader_stage) in shaders.iter().zip(&shader_stages) {
            for binding in &shader.metadata.descriptor_bindings {
                let binding_set = &mut binding_sets[binding.group as usize];
                let descriptor_type: DescriptorType = binding.binding_type.into();

                if let Some(existing_binding) =
                    binding_set
                        .iter_mut()
                        .find(|existing: &&mut DescriptorSetLayoutBinding| {
                            existing.binding == binding.binding
                        })
                {
                    if existing_binding.descriptor_type != descriptor_type {
                        return Err(Error::invalid_argument(format!(
                            "Shader binding {} in group {} has conflicting types.",
                            binding.binding, binding.group
                        )));
                    }

                    existing_binding.stage_flags |= *shader_stage;
                } else {
                    binding_set.push(
                        DescriptorSetLayoutBinding::default()
                            .binding(binding.binding)
                            .descriptor_type(descriptor_type)
                            .descriptor_count(1)
                            .stage_flags(*shader_stage),
                    );
                }
            }
        }

        let mut descriptor_set_layouts = DropGuard::new(
            Vec::with_capacity(binding_sets.len()),
            |descriptor_set_layouts| unsafe {
                for descriptor_set_layout in descriptor_set_layouts {
                    device.destroy_descriptor_set_layout(descriptor_set_layout, None);
                }
            },
        );
        let descriptor_binding_counts = binding_sets.iter().map(Vec::len).collect();
        for binding_set in &mut binding_sets {
            binding_set.sort_by_key(|binding| binding.binding);

            descriptor_set_layouts.push(unsafe {
                device.create_descriptor_set_layout(
                    &DescriptorSetLayoutCreateInfo::default().bindings(binding_set),
                    None,
                )?
            });
        }

        let pipeline_layout_info =
            PipelineLayoutCreateInfo::default().set_layouts(&descriptor_set_layouts);
        let pipeline_layout = DropGuard::new(
            unsafe { device.create_pipeline_layout(&pipeline_layout_info, None)? },
            |pipeline_layout| unsafe {
                device.destroy_pipeline_layout(pipeline_layout, None);
            },
        );

        let mut offset = 0;
        let mut attribute_descriptions = Vec::with_capacity(vertex_bindings.len());
        for binding in vertex_bindings {
            let format = match (
                binding.scalar_kind,
                binding.scalar_width,
                binding.components,
            ) {
                (ScalarKind::Float, 4, 1) => ash::vk::Format::R32_SFLOAT,
                (ScalarKind::Float, 4, 2) => ash::vk::Format::R32G32_SFLOAT,
                (ScalarKind::Float, 4, 3) => ash::vk::Format::R32G32B32_SFLOAT,
                (ScalarKind::Float, 4, 4) => ash::vk::Format::R32G32B32A32_SFLOAT,
                _ => {
                    return Err(Error::invalid_argument("Unsupported vertex format."));
                }
            };

            attribute_descriptions.push(
                VertexInputAttributeDescription::default()
                    .location(binding.location)
                    .binding(0)
                    .format(format)
                    .offset(offset),
            );

            offset += binding.scalar_width as u32 * binding.components;
        }
        let vertex_buffer_bindings = if attribute_descriptions.is_empty() {
            Vec::new()
        } else {
            vec![
                VertexInputBindingDescription::default()
                    .binding(0)
                    .stride(offset)
                    .input_rate(VertexInputRate::VERTEX),
            ]
        };
        let vertex_input = PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&vertex_buffer_bindings)
            .vertex_attribute_descriptions(&attribute_descriptions);

        let input_assembly = PipelineInputAssemblyStateCreateInfo::default()
            .topology(PrimitiveTopology::from(options.topology))
            // Metal cannot disable primitive restart for strips, so it is enabled on every
            // platform for consistent behavior.
            .primitive_restart_enable(matches!(
                options.topology,
                crate::renderers::PrimitiveTopology::LineStrip
                    | crate::renderers::PrimitiveTopology::TriangleStrip
            ));

        // Viewport and scissor are dynamic so pipelines don't need to be recreated when the
        // swapchain extent changes.
        let viewport_state = PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let rasterization = PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(options.depth_clamp)
            .rasterizer_discard_enable(false)
            .polygon_mode(options.polygon_mode.into())
            .cull_mode(options.culling.into())
            .front_face(options.front_face.into())
            .depth_bias_enable(options.depth_bias.is_some())
            .depth_bias_constant_factor(options.depth_bias.map_or(0.0, |bias| bias.constant))
            .depth_bias_slope_factor(options.depth_bias.map_or(0.0, |bias| bias.slope))
            .depth_bias_clamp(
                options
                    .depth_bias
                    .and_then(|bias| bias.clamp)
                    .unwrap_or(0.0),
            )
            .line_width(1.0);
        let multisampling = PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(match options.depth_only {
                true => ash::vk::SampleCountFlags::TYPE_1,
                false => self.msaa.into(),
            })
            .sample_shading_enable(options.min_sample_shading.is_some())
            .min_sample_shading(options.min_sample_shading.unwrap_or(0.0));
        let depth_stencil = PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(options.depth_test)
            .depth_write_enable(options.depth_write)
            .depth_compare_op(ash::vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);
        let color_blend_attachments =
            vec![
                ash::vk::PipelineColorBlendAttachmentState::from(options.blending);
                color_formats.len()
            ];
        let color_blend =
            PipelineColorBlendStateCreateInfo::default().attachments(&color_blend_attachments);
        let dynamic_states = [DynamicState::VIEWPORT, DynamicState::SCISSOR];
        let dynamic_state =
            PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
        let tessellation_state = match options.topology {
            crate::renderers::PrimitiveTopology::PatchList { control_points } => Some(
                PipelineTessellationStateCreateInfo::default().patch_control_points(control_points),
            ),
            _ => None,
        };
        // The render pass only describes the canvases the pipeline is compatible with, thus it can
        // be destroyed once the pipeline is created.
        let render_pass = DropGuard::new(
            match options.depth_only {
                true => Self::create_depth_render_pass(device)?,
                false => Self::create_render_pass(device, &color_formats, self.msaa.into())?,
            },
            |render_pass| unsafe {
                device.destroy_render_pass(render_pass, None);
            },
        );
        let mut pipeline_info = GraphicsPipelineCreateInfo::default()
            .stages(&stage_infos)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization)
            .multisample_state(&multisampling)
            .depth_stencil_state(&depth_stencil)
            .color_blend_state(&color_blend)
            .dynamic_state(&dynamic_state)
            .layout(*pipeline_layout)
            .render_pass(*render_pass)
            .subpass(0);
        if pipeline_type == PipelineType::Vertex {
            pipeline_info = pipeline_info
                .vertex_input_state(&vertex_input)
                .input_assembly_state(&input_assembly);
        }
        if let Some(tessellation_state) = &tessellation_state {
            pipeline_info = pipeline_info.tessellation_state(tessellation_state);
        }
        let pipeline = unsafe {
            device.create_graphics_pipelines(PipelineCache::null(), &[pipeline_info], None)?[0]
        };
        Ok(VulkanGraphicsPipeline {
            pipeline,
            layout: DropGuard::dismiss(pipeline_layout),
            descriptor_layouts: DropGuard::dismiss(descriptor_set_layouts),
            descriptor_binding_counts,
            descriptor_set_cache: DescriptorSetCache::new(),
            depth_only: options.depth_only,
            color_formats,
            mesh_stage: match pipeline_type {
                PipelineType::Mesh => match shader_stages.contains(&ShaderStageFlags::TASK_EXT) {
                    true => Some(ShaderStage::Task),
                    false => Some(ShaderStage::Mesh),
                },
                _ => None,
            },
        })
    }
    fn destroy_graphics_pipeline(
        &self,
        device: &ash::Device,
        pipeline: &mut VulkanGraphicsPipeline,
    ) -> Result<()> {
        unsafe {
            device.destroy_pipeline(pipeline.pipeline, None);
            device.destroy_pipeline_layout(pipeline.layout, None);
            for descriptor_layout in &pipeline.descriptor_layouts {
                device.destroy_descriptor_set_layout(*descriptor_layout, None);
            }
        }
        pipeline.pipeline = ash::vk::Pipeline::null();
        pipeline.layout = ash::vk::PipelineLayout::null();
        pipeline.descriptor_layouts.clear();
        pipeline.descriptor_binding_counts.clear();
        Ok(())
    }
    fn create_color_canvas_framebuffer(
        &self,
        device: &ash::Device,
        canvas: &mut VulkanColorCanvas,
    ) -> Result<()> {
        let color_formats = canvas
            .textures
            .iter()
            .map(|texture| texture.texture_options.format)
            .collect::<Vec<_>>();
        canvas.render_pass = Self::create_render_pass(device, &color_formats, canvas.msaa.into())?;
        let views = match canvas.msaa_color_images.is_empty() {
            true => canvas
                .textures
                .iter()
                .map(|texture| texture.image_view)
                .chain([canvas.depth_image.view])
                .collect::<Vec<_>>(),
            false => canvas
                .msaa_color_images
                .iter()
                .map(|msaa_color_image| msaa_color_image.view)
                .chain([canvas.depth_image.view])
                .chain(canvas.textures.iter().map(|texture| texture.image_view))
                .collect::<Vec<_>>(),
        };
        canvas.framebuffer = unsafe {
            device.create_framebuffer(
                &FramebufferCreateInfo::default()
                    .render_pass(canvas.render_pass)
                    .attachments(&views)
                    .width(canvas.width())
                    .height(canvas.height())
                    .layers(canvas.layer_count),
                None,
            )?
        };
        Ok(())
    }
    fn create_depth_canvas_framebuffer(
        &self,
        device: &ash::Device,
        canvas: &mut VulkanDepthCanvas,
    ) -> Result<()> {
        canvas.render_pass = Self::create_depth_render_pass(device)?;
        canvas.framebuffer = unsafe {
            device.create_framebuffer(
                &FramebufferCreateInfo::default()
                    .render_pass(canvas.render_pass)
                    .attachments(std::slice::from_ref(&canvas.texture.image_view))
                    .width(canvas.width())
                    .height(canvas.height())
                    .layers(canvas.layer_count),
                None,
            )?
        };
        Ok(())
    }
    fn begin(
        &self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        canvas: &VulkanColorCanvas,
    ) -> Result<()> {
        let begin_info = RenderPassBeginInfo::default()
            .render_pass(canvas.render_pass)
            .framebuffer(canvas.framebuffer)
            .render_area(ash::vk::Rect2D {
                offset: ash::vk::Offset2D { x: 0, y: 0 },
                extent: ash::vk::Extent2D {
                    width: canvas.width(),
                    height: canvas.height(),
                },
            });
        unsafe {
            device.cmd_begin_render_pass(command_buffer, &begin_info, SubpassContents::INLINE);
        }
        Ok(())
    }
    fn begin_depth(
        &self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        canvas: &VulkanDepthCanvas,
    ) -> Result<()> {
        let begin_info = RenderPassBeginInfo::default()
            .render_pass(canvas.render_pass)
            .framebuffer(canvas.framebuffer)
            .render_area(ash::vk::Rect2D {
                offset: ash::vk::Offset2D { x: 0, y: 0 },
                extent: ash::vk::Extent2D {
                    width: canvas.width(),
                    height: canvas.height(),
                },
            });
        unsafe {
            device.cmd_begin_render_pass(command_buffer, &begin_info, SubpassContents::INLINE);
        }
        Ok(())
    }
    fn end(&self, device: &ash::Device, command_buffer: CommandBuffer) -> Result<()> {
        unsafe {
            device.cmd_end_render_pass(command_buffer);
        }
        Ok(())
    }
    fn destroy(&mut self, _: &ash::Device) -> Result<()> {
        Ok(())
    }
}

impl RenderPassRendering {
    /// Creates a render pass that renders into a canvas. The colors and depth are loaded so the
    /// canvas keeps its content between render passes. Render passes created with the same color
    /// formats and sample count are compatible with each other.
    fn create_render_pass(
        device: &ash::Device,
        color_formats: &[TextureFormat],
        sample_count: SampleCountFlags,
    ) -> Result<RenderPass> {
        let color_attachments = color_formats
            .iter()
            .map(|color_format| {
                AttachmentDescription::default()
                    .format((*color_format).into())
                    .samples(sample_count)
                    .load_op(AttachmentLoadOp::LOAD)
                    .store_op(AttachmentStoreOp::STORE)
                    .stencil_load_op(AttachmentLoadOp::DONT_CARE)
                    .stencil_store_op(AttachmentStoreOp::DONT_CARE)
                    .initial_layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .final_layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            })
            .collect::<Vec<_>>();
        let depth_attachment = AttachmentDescription::default()
            .format(ash::vk::Format::D32_SFLOAT)
            .samples(sample_count)
            .load_op(AttachmentLoadOp::LOAD)
            .store_op(AttachmentStoreOp::STORE)
            .stencil_load_op(AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(AttachmentStoreOp::DONT_CARE)
            .initial_layout(ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .final_layout(ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let color_count = color_formats.len() as u32;
        let color_references = (0..color_count)
            .map(|index| {
                AttachmentReference::default()
                    .attachment(index)
                    .layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            })
            .collect::<Vec<_>>();
        let depth_reference = AttachmentReference::default()
            .attachment(color_count)
            .layout(ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let resolve_references = (0..color_count)
            .map(|index| {
                AttachmentReference::default()
                    .attachment(color_count + 1 + index)
                    .layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            })
            .collect::<Vec<_>>();
        let mut subpass = SubpassDescription::default()
            .pipeline_bind_point(PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_references)
            .depth_stencil_attachment(&depth_reference);
        let mut attachments = color_attachments.clone();
        attachments.push(depth_attachment);
        if sample_count != SampleCountFlags::TYPE_1 {
            subpass = subpass.resolve_attachments(&resolve_references);
            attachments.extend(color_attachments.iter().map(|color_attachment| {
                color_attachment
                    .samples(SampleCountFlags::TYPE_1)
                    .load_op(AttachmentLoadOp::DONT_CARE)
            }));
        }
        unsafe {
            Ok(device.create_render_pass(
                &RenderPassCreateInfo::default()
                    .attachments(&attachments)
                    .subpasses(std::slice::from_ref(&subpass)),
                None,
            )?)
        }
    }

    fn create_depth_render_pass(device: &ash::Device) -> Result<RenderPass> {
        let depth_attachment = AttachmentDescription::default()
            .format(ash::vk::Format::D32_SFLOAT)
            .samples(SampleCountFlags::TYPE_1)
            .load_op(AttachmentLoadOp::LOAD)
            .store_op(AttachmentStoreOp::STORE)
            .stencil_load_op(AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(AttachmentStoreOp::DONT_CARE)
            .initial_layout(ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .final_layout(ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let depth_reference = AttachmentReference::default()
            .attachment(0)
            .layout(ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let subpass = SubpassDescription::default()
            .pipeline_bind_point(PipelineBindPoint::GRAPHICS)
            .depth_stencil_attachment(&depth_reference);
        unsafe {
            Ok(device.create_render_pass(
                &RenderPassCreateInfo::default()
                    .attachments(std::slice::from_ref(&depth_attachment))
                    .subpasses(std::slice::from_ref(&subpass)),
                None,
            )?)
        }
    }
}
