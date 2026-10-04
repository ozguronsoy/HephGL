use ash::vk::{DescriptorSetLayout, PipelineLayout};

pub struct ShaderModules<'a> {
    pub device: &'a ash::Device,
    pub modules: Vec<ash::vk::ShaderModule>,
}

pub struct PipelineResources<'a> {
    pub device: &'a ash::Device,
    pub layout: Option<PipelineLayout>,
    pub descriptor_layouts: Vec<DescriptorSetLayout>,
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
