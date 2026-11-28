use crate::{
    Error, Result,
    renderers::{
        BufferUsage, RendererBuffer, RendererResource, ResourceBinding, ResourceBindingType,
        downcast_ref,
        vulkan::{VulkanRenderer, device::DeviceContext, resources::VulkanBuffer},
    },
    shader::ShaderStage,
};

impl VulkanRenderer {
    /// Binds the vertex and index buffers, then records the draw command of a vertex pipeline.
    pub(super) fn record_vertex_draw(
        &self,
        device: &ash::Device,
        command_buffer: ash::vk::CommandBuffer,
        index_buffer: Option<&dyn RendererBuffer>,
        vertex_bindings: &[ResourceBinding<'_>],
        draw_count: u32,
        instance_count: u32,
    ) -> Result<()> {
        for vertex_binding in vertex_bindings {
            let ResourceBindingType::Buffer {
                buffer,
                offset,
                size,
            } = vertex_binding.resource
            else {
                return Err(Error::invalid_argument(
                    "Vertex binding type must be a buffer",
                ));
            };
            if buffer.usage() != BufferUsage::Vertex {
                return Err(Error::invalid_argument(
                    "Vertex buffer's usage must be `BufferUsage::Vertex`.",
                ));
            }

            let vertex_buffer = downcast_ref::<VulkanBuffer>(buffer)?;
            if vertex_buffer.renderer_id != self.id {
                return Err(Error::invalid_argument(
                    "Cannot bind a vertex buffer created by another renderer or old device.",
                ));
            }
            if !vertex_buffer.is_allocated() {
                return Err(Error::invalid_argument(
                    "Cannot bind a destroyed vertex buffer.",
                ));
            }
            if offset
                .checked_add(size)
                .is_none_or(|x| x > vertex_buffer.size)
            {
                return Err(Error::invalid_argument(
                    "Buffer overflow while binding the vertex buffer.",
                ));
            }
            unsafe {
                device.cmd_bind_vertex_buffers(
                    command_buffer,
                    vertex_binding.binding,
                    std::slice::from_ref(&vertex_buffer.buffer),
                    std::slice::from_ref(&(offset as u64)),
                );
            }
        }

        let Some(index_buffer) = index_buffer else {
            unsafe {
                device.cmd_draw(command_buffer, draw_count, instance_count, 0, 0);
            }
            return Ok(());
        };
        if index_buffer.usage() != BufferUsage::Index {
            return Err(Error::invalid_argument(
                "Index buffer's usage must be `BufferUsage::Index`.",
            ));
        }

        let index_buffer = downcast_ref::<VulkanBuffer>(index_buffer)?;
        if index_buffer.renderer_id != self.id {
            return Err(Error::invalid_argument(
                "Cannot bind an index buffer created by another renderer or old device.",
            ));
        }
        if !index_buffer.is_allocated() {
            return Err(Error::invalid_argument(
                "Cannot bind a destroyed index buffer.",
            ));
        }
        unsafe {
            device.cmd_bind_index_buffer(
                command_buffer,
                index_buffer.buffer,
                0,
                ash::vk::IndexType::UINT32,
            );
            device.cmd_draw_indexed(command_buffer, draw_count, instance_count, 0, 0, 0);
        }
        Ok(())
    }

    /// Records the draw command of a mesh pipeline whose first stage is `mesh_stage`.
    pub(super) fn record_mesh_draw(
        device_context: &DeviceContext,
        command_buffer: ash::vk::CommandBuffer,
        mesh_stage: ShaderStage,
        group_count: (u32, u32, u32),
    ) -> Result<()> {
        let (max_count, max_total_count) = match mesh_stage {
            ShaderStage::Task => (
                device_context.max_task_work_group_count,
                device_context.max_task_work_group_total_count,
            ),
            _ => (
                device_context.max_mesh_work_group_count,
                device_context.max_mesh_work_group_total_count,
            ),
        };
        let (x, y, z) = group_count;
        if x > max_count[0]
            || y > max_count[1]
            || z > max_count[2]
            || (u64::from(x) * u64::from(y))
                .checked_mul(u64::from(z))
                .is_none_or(|total_count| total_count > u64::from(max_total_count))
        {
            return Err(Error::InvalidArgument(format!(
                "The group count ({x}, {y}, {z}) must not exceed ({}, {}, {}), and the product \
                 of its components must not exceed {max_total_count}.",
                max_count[0], max_count[1], max_count[2]
            )));
        }

        let mesh_shader_loader =
            device_context
                .mesh_shader_loader
                .as_ref()
                .ok_or(Error::invalid_operation(
                    "Device is not initialized with the `MeshShaders` feature.",
                ))?;
        unsafe {
            mesh_shader_loader.cmd_draw_mesh_tasks(command_buffer, x, y, z);
        }
        Ok(())
    }
}
