use std::ffi::CString;

use ash::vk::{
    AttachmentLoadOp, AttachmentStoreOp, ClearDepthStencilValue, ColorComponentFlags,
    CommandBuffer, CullModeFlags, DescriptorSetLayout, DescriptorSetLayoutBinding,
    DescriptorSetLayoutCreateInfo, DescriptorType, DynamicState, FrontFace,
    GraphicsPipelineCreateInfo, ImageLayout, PipelineCache, PipelineColorBlendAttachmentState,
    PipelineColorBlendStateCreateInfo, PipelineDepthStencilStateCreateInfo,
    PipelineDynamicStateCreateInfo, PipelineInputAssemblyStateCreateInfo, PipelineLayout,
    PipelineLayoutCreateInfo, PipelineMultisampleStateCreateInfo,
    PipelineRasterizationStateCreateInfo, PipelineRenderingCreateInfo,
    PipelineShaderStageCreateInfo, PipelineVertexInputStateCreateInfo,
    PipelineViewportStateCreateInfo, PolygonMode, PrimitiveTopology, RenderingAttachmentInfo,
    RenderingInfo, ShaderModuleCreateInfo, ShaderStageFlags, VertexInputAttributeDescription,
    VertexInputBindingDescription, VertexInputRate,
};

use crate::{
    renderers::{
        Renderer, RendererResult,
        error::RendererError,
        settings::{Msaa, Settings},
        vulkan::{
            VulkanRenderer, rendering::VulkanRendering, resources::VulkanGraphicsPipeline,
            swapchain::SwapchainContext,
        },
    },
    shader::Shader,
};

/// Implements dynamic rendering.
pub struct DynamicRendering {
    color_format: ash::vk::Format,
    depth_format: ash::vk::Format,
    msaa: Msaa,
    stereoscopic_3d_rendering: bool,
}

impl VulkanRendering for DynamicRendering {
    fn new(
        settings: &Settings,
        _: &ash::Device,
        swapchain_context: &SwapchainContext,
    ) -> RendererResult<Self>
    where
        Self: Sized,
    {
        // This is recreated when settings change, so we can store the settings we need here.
        Ok(Self {
            color_format: swapchain_context.format,
            // TODO: Get this from swapchain context?
            depth_format: ash::vk::Format::D32_SFLOAT,
            msaa: settings.msaa,
            stereoscopic_3d_rendering: settings.stereoscopic_3d_rendering,
        })
    }
    fn create_graphics_pipeline(
        &mut self,
        device: &ash::Device,
        shaders: &[&Shader],
    ) -> RendererResult<<VulkanRenderer as Renderer>::GraphicsPipelineHandle> {
        struct ShaderModules<'a> {
            device: &'a ash::Device,
            modules: Vec<ash::vk::ShaderModule>,
        }
        impl Drop for ShaderModules<'_> {
            fn drop(&mut self) {
                unsafe {
                    for module in &self.modules {
                        self.device.destroy_shader_module(*module, None);
                    }
                }
            }
        }

        struct PipelineResources<'a> {
            device: &'a ash::Device,
            layout: Option<PipelineLayout>,
            descriptor_layouts: Vec<DescriptorSetLayout>,
        }
        impl Drop for PipelineResources<'_> {
            fn drop(&mut self) {
                unsafe {
                    if let Some(layout) = self.layout {
                        self.device.destroy_pipeline_layout(layout, None);
                    }

                    for descriptor_layout in &self.descriptor_layouts {
                        self.device
                            .destroy_descriptor_set_layout(*descriptor_layout, None);
                    }
                }
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
                return Err(RendererError::fail(format!(
                    "Shader data from '{}' is not valid SPIR-V (not 4-byte aligned).",
                    shader.file_path
                )));
            }

            let stage = VulkanRenderer::convert_shader_stage(shader.metadata.stage)?;
            if stage == ShaderStageFlags::COMPUTE {
                return Err(RendererError::invalid_argument(
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
                        return Err(RendererError::invalid_argument(format!(
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
            .ok_or(RendererError::invalid_argument(
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
                    return Err(RendererError::invalid_argument(
                        "Unsupported vertex format.",
                    ));
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
            .topology(PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        // Viewport and scissor are dynamic so pipelines don't need to be recreated when the
        // swapchain extent changes.
        let viewport_state = PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let rasterization = PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(PolygonMode::FILL)
            .cull_mode(CullModeFlags::NONE)
            .front_face(FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(false)
            .line_width(1.0);
        let multisampling = PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(self.msaa.into())
            .sample_shading_enable(false);
        let depth_stencil = PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(ash::vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);
        let color_blend_attachment = PipelineColorBlendAttachmentState::default()
            .blend_enable(false)
            .color_write_mask(
                ColorComponentFlags::R
                    | ColorComponentFlags::G
                    | ColorComponentFlags::B
                    | ColorComponentFlags::A,
            );
        let color_blend = PipelineColorBlendStateCreateInfo::default()
            .attachments(std::slice::from_ref(&color_blend_attachment));
        let dynamic_states = [DynamicState::VIEWPORT, DynamicState::SCISSOR];
        let dynamic_state =
            PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
        let color_formats = [self.color_format];
        let mut rendering_info = PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&color_formats)
            .depth_attachment_format(self.depth_format);

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
            .render_pass(ash::vk::RenderPass::null())
            .push_next(&mut rendering_info);
        let pipeline = unsafe {
            device.create_graphics_pipelines(PipelineCache::null(), &[pipeline_info], None)?[0]
        };
        resources.layout = None;
        let descriptor_layouts = std::mem::take(&mut resources.descriptor_layouts);
        Ok(VulkanGraphicsPipeline {
            pipeline,
            layout: pipeline_layout,
            descriptor_layouts,
        })
    }
    fn destroy_graphics_pipeline(
        &mut self,
        device: &ash::Device,
        pipeline: &<VulkanRenderer as Renderer>::GraphicsPipelineHandle,
    ) -> RendererResult<()> {
        unsafe {
            device.destroy_pipeline(pipeline.pipeline, None);
            device.destroy_pipeline_layout(pipeline.layout, None);
            for descriptor_layout in &pipeline.descriptor_layouts {
                device.destroy_descriptor_set_layout(*descriptor_layout, None);
            }
        }
        Ok(())
    }
    fn begin(
        &mut self,
        device: &ash::Device,
        command_buffer: CommandBuffer,
        swapchain_context: &SwapchainContext,
    ) -> RendererResult<()> {
        let color_attachment_info = if self.msaa == Msaa::X1 {
            RenderingAttachmentInfo::default()
                .image_view(swapchain_context.image_views[swapchain_context.current_image_index])
                .image_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(AttachmentLoadOp::DONT_CARE)
                .store_op(AttachmentStoreOp::STORE)
        } else {
            RenderingAttachmentInfo::default()
                .image_view(swapchain_context.msaa_color_image_view)
                .image_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(AttachmentLoadOp::DONT_CARE)
                .store_op(AttachmentStoreOp::DONT_CARE)
                .resolve_mode(ash::vk::ResolveModeFlags::AVERAGE)
                .resolve_image_view(
                    swapchain_context.image_views[swapchain_context.current_image_index],
                )
                .resolve_image_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        };
        let depth_attachment_info = RenderingAttachmentInfo::default()
            .image_view(swapchain_context.depth_image_view)
            .image_layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(AttachmentLoadOp::CLEAR)
            .store_op(AttachmentStoreOp::STORE)
            .clear_value(ash::vk::ClearValue {
                depth_stencil: ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            });
        let rendering_info = RenderingInfo::default()
            .render_area(ash::vk::Rect2D {
                offset: ash::vk::Offset2D { x: 0, y: 0 },
                extent: swapchain_context.extent,
            })
            .layer_count(match self.stereoscopic_3d_rendering {
                true => 2,
                false => 1,
            })
            .color_attachments(std::slice::from_ref(&color_attachment_info))
            .depth_attachment(&depth_attachment_info);
        unsafe {
            device.cmd_begin_rendering(command_buffer, &rendering_info);
        }
        Ok(())
    }
    fn end(&mut self, device: &ash::Device, command_buffer: CommandBuffer) -> RendererResult<()> {
        unsafe {
            device.cmd_end_rendering(command_buffer);
        }
        Ok(())
    }
    fn destroy(&mut self, _: &ash::Device) -> RendererResult<()> {
        Ok(())
    }
}
