use naga::ShaderStage;

use crate::shader::{ShaderDescriptorBinding, ShaderVertexBinding};

pub(crate) struct ShaderMetadata {
    pub(crate) entry_name: String,
    pub(crate) stage: ShaderStage,
    pub(crate) size: [u32; 3],
    pub(crate) vertex_bindings: Vec<ShaderVertexBinding>,
    pub(crate) descriptor_bindings: Vec<ShaderDescriptorBinding>,
}
