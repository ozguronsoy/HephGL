use ash::vk::{DescriptorSetLayout, PipelineLayout};

pub(super) struct ShaderModules<'a> {
    pub(super) device: &'a ash::Device,
    pub(super) modules: Vec<ash::vk::ShaderModule>,
}

pub(super) struct PipelineResources<'a> {
    pub(super) device: &'a ash::Device,
    pub(super) layout: Option<PipelineLayout>,
    pub(super) descriptor_layouts: Vec<DescriptorSetLayout>,
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
