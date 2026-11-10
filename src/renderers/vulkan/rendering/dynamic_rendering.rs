use std::ffi::CString;

use ash::vk::{
    AttachmentLoadOp, AttachmentStoreOp, CommandBuffer, DescriptorSetLayoutBinding,
    DescriptorSetLayoutCreateInfo, DescriptorType, DynamicState, GraphicsPipelineCreateInfo,
    ImageLayout, PipelineCache, PipelineColorBlendStateCreateInfo,
    PipelineDepthStencilStateCreateInfo, PipelineDynamicStateCreateInfo,
    PipelineInputAssemblyStateCreateInfo, PipelineLayoutCreateInfo,
    PipelineMultisampleStateCreateInfo, PipelineRasterizationStateCreateInfo,
    PipelineRenderingCreateInfo, PipelineShaderStageCreateInfo,
    PipelineTessellationStateCreateInfo, PipelineVertexInputStateCreateInfo,
    PipelineViewportStateCreateInfo, PrimitiveTopology, RenderingAttachmentInfo, RenderingInfo,
    ShaderModuleCreateInfo, ShaderStageFlags, VertexInputAttributeDescription,
    VertexInputBindingDescription, VertexInputRate,
};

use crate::{
    Error, Result,
    graphics_device::GraphicsDeviceFeature,
    renderers::{
        RendererCanvas,
        settings::{GraphicsPipelineOptions, Msaa, PolygonMode, Settings},
        vulkan::{
            canvas::VulkanCanvas,
            pipeline::VulkanGraphicsPipeline,
            rendering::{
                VulkanRendering,
                lifetime_guards::{PipelineResources, ShaderModules},
            },
            resources::DescriptorSetCache,
            swapchain::SwapchainContext,
        },
    },
    shader::{ScalarKind, Shader, ShaderPrimitive, ShaderStage},
};

/// Implements dynamic rendering.
pub(super) struct DynamicRendering {
    color_format: ash::vk::Format,
    depth_format: ash::vk::Format,
    msaa: Msaa,
}

impl VulkanRendering for DynamicRendering {
    fn new(
        settings: &Settings,
        _: &ash::Device,
        swapchain_context: &SwapchainContext,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        // This is recreated when settings change, so we can store the settings we need here.
        Ok(Self {
            color_format: swapchain_context.format,
            // TODO: Get this from swapchain context?
            depth_format: ash::vk::Format::D32_SFLOAT,
            msaa: settings.msaa,
        })
    }
    fn create_graphics_pipeline(
        &self,
        device: &ash::Device,
        shaders: &[&Shader],
        options: &GraphicsPipelineOptions,
        enabled_features: &[GraphicsDeviceFeature],
        max_tessellation_patch_size: u32,
    ) -> Result<VulkanGraphicsPipeline> {
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

        // Create shader stage infos.
        let mut shader_modules = ShaderModules {
            device,
            modules: Vec::with_capacity(shaders.len()),
        };
        let mut entry_names = Vec::with_capacity(shaders.len());
        let mut shader_stages = Vec::with_capacity(shaders.len());
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
            let shader_stage = ShaderStageFlags::from(shader.metadata.stage);
            if shader_stages.contains(&shader_stage) {
                return Err(Error::invalid_argument(
                    "Graphics pipeline cannot contain multiple shaders of the same stage.",
                ));
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
            shader_modules.modules.push(unsafe {
                device.create_shader_module(&ShaderModuleCreateInfo::default().code(code), None)?
            });
            shader_stages.push(shader_stage);
            entry_names.push(CString::new(shader.metadata.entry_name.as_str())?);
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
            .zip(&shader_modules.modules)
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

        let mut resources = PipelineResources {
            device,
            layout: None,
            descriptor_layouts: Vec::with_capacity(binding_sets.len()),
        };
        let descriptor_binding_counts = binding_sets.iter().map(Vec::len).collect();
        for binding_set in &mut binding_sets {
            binding_set.sort_by_key(|binding| binding.binding);
            resources.descriptor_layouts.push(unsafe {
                device.create_descriptor_set_layout(
                    &DescriptorSetLayoutCreateInfo::default().bindings(binding_set),
                    None,
                )?
            });
        }

        let pipeline_layout_info =
            PipelineLayoutCreateInfo::default().set_layouts(&resources.descriptor_layouts);
        let pipeline_layout =
            unsafe { device.create_pipeline_layout(&pipeline_layout_info, None)? };
        resources.layout = Some(pipeline_layout);

        let vertex_shader = shaders
            .iter()
            .find(|shader| shader.metadata.stage == ShaderStage::Vertex)
            .ok_or(Error::invalid_argument(
                "Graphics pipeline requires a vertex shader.",
            ))?;
        let mut offset = 0;
        let mut attribute_descriptions =
            Vec::with_capacity(vertex_shader.metadata.vertex_bindings.len());
        for binding in &vertex_shader.metadata.vertex_bindings {
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
            .primitive_restart_enable(false);

        // Viewport and scissor are dynamic so pipelines don't need to be recreated when the
        // swapchain extent changes.
        let viewport_state = PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let rasterization = PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(options.polygon_mode.into())
            .cull_mode(options.culling.into())
            .front_face(options.front_face.into())
            .depth_bias_enable(false)
            .line_width(1.0);
        let multisampling = PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(self.msaa.into())
            .sample_shading_enable(options.min_sample_shading.is_some())
            .min_sample_shading(options.min_sample_shading.unwrap_or(0.0));
        let depth_stencil = PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(options.depth_test)
            .depth_write_enable(options.depth_write)
            .depth_compare_op(ash::vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);
        let color_blend_attachment = options.blending.into();
        let color_blend = PipelineColorBlendStateCreateInfo::default()
            .attachments(std::slice::from_ref(&color_blend_attachment));
        let dynamic_states = [DynamicState::VIEWPORT, DynamicState::SCISSOR];
        let dynamic_state =
            PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
        let color_formats = [self.color_format];
        let mut rendering_info = PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&color_formats)
            .depth_attachment_format(self.depth_format);
        let tessellation_state = match options.topology {
            crate::renderers::PrimitiveTopology::PatchList { control_points } => Some(
                PipelineTessellationStateCreateInfo::default().patch_control_points(control_points),
            ),
            _ => None,
        };
        let mut pipeline_info = GraphicsPipelineCreateInfo::default()
            .stages(&stage_infos)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization)
            .multisample_state(&multisampling)
            .depth_stencil_state(&depth_stencil)
            .color_blend_state(&color_blend)
            .dynamic_state(&dynamic_state)
            .layout(pipeline_layout)
            .render_pass(ash::vk::RenderPass::null())
            .push_next(&mut rendering_info);
        if let Some(tessellation_state) = &tessellation_state {
            pipeline_info = pipeline_info.tessellation_state(tessellation_state);
        }
        let pipeline = unsafe {
            device.create_graphics_pipelines(PipelineCache::null(), &[pipeline_info], None)?[0]
        };
        resources.layout = None;
        let descriptor_layouts = std::mem::take(&mut resources.descriptor_layouts);
        Ok(VulkanGraphicsPipeline {
            pipeline,
            layout: pipeline_layout,
            descriptor_layouts,
            descriptor_binding_counts,
            descriptor_set_cache: DescriptorSetCache::new(),
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
    fn create_canvas_framebuffer(&self, _: &ash::Device, _: &mut VulkanCanvas) -> Result<()> {
        // Dynamic rendering uses the image views of the canvas directly.
        Ok(())
    }
    fn begin(
        &mut self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        canvas: &VulkanCanvas,
    ) -> Result<()> {
        let color_attachment_info = match &canvas.msaa_color_image {
            None => RenderingAttachmentInfo::default()
                .image_view(canvas.texture.image_view)
                .image_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(AttachmentLoadOp::LOAD)
                .store_op(AttachmentStoreOp::STORE),
            Some(msaa_color_image) => RenderingAttachmentInfo::default()
                .image_view(msaa_color_image.view)
                .image_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(AttachmentLoadOp::LOAD)
                .store_op(AttachmentStoreOp::STORE)
                .resolve_mode(ash::vk::ResolveModeFlags::AVERAGE)
                .resolve_image_view(canvas.texture.image_view)
                .resolve_image_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
        };
        let depth_attachment_info = RenderingAttachmentInfo::default()
            .image_view(canvas.depth_image.view)
            .image_layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(AttachmentLoadOp::LOAD)
            .store_op(AttachmentStoreOp::STORE);
        let rendering_info = RenderingInfo::default()
            .render_area(ash::vk::Rect2D {
                offset: ash::vk::Offset2D { x: 0, y: 0 },
                extent: ash::vk::Extent2D {
                    width: canvas.width(),
                    height: canvas.height(),
                },
            })
            .layer_count(canvas.layer_count)
            .color_attachments(std::slice::from_ref(&color_attachment_info))
            .depth_attachment(&depth_attachment_info);
        unsafe {
            device.cmd_begin_rendering(command_buffer, &rendering_info);
        }
        Ok(())
    }
    fn end(&mut self, device: &ash::Device, command_buffer: CommandBuffer) -> Result<()> {
        unsafe {
            device.cmd_end_rendering(command_buffer);
        }
        Ok(())
    }
    fn destroy(&mut self, _: &ash::Device) -> Result<()> {
        Ok(())
    }
}
