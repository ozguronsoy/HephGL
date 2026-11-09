use crate::shader::{ShaderDescriptorBinding, ShaderVertexBinding};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShaderStage {
    Vertex,
    Geometry,
    Fragment,
    Compute,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShaderPrimitive {
    Point,
    Line,
    Triangle,
}

pub(crate) struct ShaderMetadata {
    pub(crate) entry_name: String,
    pub(crate) stage: ShaderStage,
    pub(crate) size: [u32; 3],
    pub(crate) input_primitive: Option<ShaderPrimitive>,
    pub(crate) vertex_bindings: Vec<ShaderVertexBinding>,
    pub(crate) descriptor_bindings: Vec<ShaderDescriptorBinding>,
}
