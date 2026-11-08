use std::ffi::CString;

use ash::vk::{
    AttachmentDescription, AttachmentLoadOp, AttachmentReference, AttachmentStoreOp, CommandBuffer,
    DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo, DescriptorType, DynamicState,
    FramebufferCreateInfo, GraphicsPipelineCreateInfo, PipelineBindPoint, PipelineCache,
    PipelineColorBlendStateCreateInfo, PipelineDepthStencilStateCreateInfo,
    PipelineDynamicStateCreateInfo, PipelineInputAssemblyStateCreateInfo, PipelineLayoutCreateInfo,
    PipelineMultisampleStateCreateInfo, PipelineRasterizationStateCreateInfo,
    PipelineShaderStageCreateInfo, PipelineVertexInputStateCreateInfo,
    PipelineViewportStateCreateInfo, PrimitiveTopology, RenderPass, RenderPassBeginInfo,
    RenderPassCreateInfo, SampleCountFlags, ShaderModuleCreateInfo, ShaderStageFlags,
    SubpassContents, SubpassDescription, VertexInputAttributeDescription,
    VertexInputBindingDescription, VertexInputRate,
};

use crate::{
    Error, Result,
    renderers::{
        RendererCanvas,
        settings::{GraphicsPipelineOptions, Msaa, Settings},
        vulkan::{
            VulkanRenderer,
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
    shader::Shader,
};

/// Implements rendering via render pass.
pub(super) struct RenderPassRendering {
    render_pass: RenderPass,
    msaa: Msaa,
}

impl VulkanRendering for RenderPassRendering {
    fn new(
        settings: &Settings,
        device: &ash::Device,
        swapchain_context: &SwapchainContext,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            render_pass: Self::create_render_pass(
                device,
                swapchain_context.format,
                settings.msaa.into(),
            )?,
            msaa: settings.msaa,
        })
    }
    fn create_graphics_pipeline(
        &self,
        device: &ash::Device,
        shaders: &[&Shader],
        options: &GraphicsPipelineOptions,
    ) -> Result<VulkanGraphicsPipeline> {
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

            let stage = VulkanRenderer::convert_shader_stage(shader.metadata.stage)?;
            if stage == ShaderStageFlags::COMPUTE {
                return Err(Error::invalid_argument(
                    "Compute shaders cannot be used in a graphics pipeline.",
                ));
            }
            shader_modules.modules.push(unsafe {
                device.create_shader_module(&ShaderModuleCreateInfo::default().code(code), None)?
            });
            shader_stages.push(stage);
            entry_names.push(CString::new(shader.metadata.entry_name.as_str())?);
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
            .find(|shader| shader.metadata.stage == naga::ShaderStage::Vertex)
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
                (naga::ScalarKind::Float, 4, 1) => ash::vk::Format::R32_SFLOAT,
                (naga::ScalarKind::Float, 4, 2) => ash::vk::Format::R32G32_SFLOAT,
                (naga::ScalarKind::Float, 4, 3) => ash::vk::Format::R32G32B32_SFLOAT,
                (naga::ScalarKind::Float, 4, 4) => ash::vk::Format::R32G32B32A32_SFLOAT,
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
            .sample_shading_enable(false);
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
        let pipeline_info = GraphicsPipelineCreateInfo::default()
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
            .render_pass(self.render_pass)
            .subpass(0);
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
    fn create_canvas_framebuffer(
        &self,
        device: &ash::Device,
        canvas: &mut VulkanCanvas,
    ) -> Result<()> {
        canvas.render_pass = Self::create_render_pass(
            device,
            canvas.texture.texture_options.format.into(),
            canvas.msaa.into(),
        )?;
        let views: &[ash::vk::ImageView] = match &canvas.msaa_color_image {
            None => &[canvas.texture.image_view, canvas.depth_image.view],
            Some(msaa_color_image) => &[
                msaa_color_image.view,
                canvas.depth_image.view,
                canvas.texture.image_view,
            ],
        };
        canvas.framebuffer = unsafe {
            device.create_framebuffer(
                &FramebufferCreateInfo::default()
                    .render_pass(canvas.render_pass)
                    .attachments(views)
                    .width(canvas.width())
                    .height(canvas.height())
                    .layers(canvas.layer_count),
                None,
            )?
        };
        Ok(())
    }
    fn begin(
        &mut self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        canvas: &VulkanCanvas,
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
    fn end(&mut self, device: &ash::Device, command_buffer: CommandBuffer) -> Result<()> {
        unsafe {
            device.cmd_end_render_pass(command_buffer);
        }
        Ok(())
    }
    fn destroy(&mut self, device: &ash::Device) -> Result<()> {
        unsafe {
            device.destroy_render_pass(self.render_pass, None);
        }
        Ok(())
    }
}

impl RenderPassRendering {
    /// Creates a render pass that renders into a canvas. The color and depth are loaded so the
    /// canvas keeps its content between render passes. Render passes created with the same color
    /// format and sample count are compatible with each other.
    fn create_render_pass(
        device: &ash::Device,
        color_format: ash::vk::Format,
        sample_count: SampleCountFlags,
    ) -> Result<RenderPass> {
        let color_attachment = AttachmentDescription::default()
            .format(color_format)
            .samples(sample_count)
            .load_op(AttachmentLoadOp::LOAD)
            .store_op(AttachmentStoreOp::STORE)
            .stencil_load_op(AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(AttachmentStoreOp::DONT_CARE)
            .initial_layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .final_layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
        let depth_attachment = AttachmentDescription::default()
            .format(ash::vk::Format::D32_SFLOAT)
            .samples(sample_count)
            .load_op(AttachmentLoadOp::LOAD)
            .store_op(AttachmentStoreOp::STORE)
            .stencil_load_op(AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(AttachmentStoreOp::DONT_CARE)
            .initial_layout(ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .final_layout(ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let resolve_attachment = color_attachment
            .samples(SampleCountFlags::TYPE_1)
            .load_op(AttachmentLoadOp::DONT_CARE);
        let color_reference = [AttachmentReference::default()
            .attachment(0)
            .layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
        let depth_reference = AttachmentReference::default()
            .attachment(1)
            .layout(ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let resolve_reference = [AttachmentReference::default()
            .attachment(2)
            .layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
        let mut subpass = SubpassDescription::default()
            .pipeline_bind_point(PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_reference)
            .depth_stencil_attachment(&depth_reference);
        let attachments: &[AttachmentDescription] = if sample_count == SampleCountFlags::TYPE_1 {
            &[color_attachment, depth_attachment]
        } else {
            subpass = subpass.resolve_attachments(&resolve_reference);
            &[color_attachment, depth_attachment, resolve_attachment]
        };
        unsafe {
            Ok(device.create_render_pass(
                &RenderPassCreateInfo::default()
                    .attachments(attachments)
                    .subpasses(std::slice::from_ref(&subpass)),
                None,
            )?)
        }
    }
}
