#[derive(Clone, Copy)]
pub(crate) enum ShaderBindingType {
    UniformBuffer,
    StorageBuffer,
    Texture,
    Sampler,
}

pub(crate) struct ShaderVertexBinding {
    pub(crate) location: u32,
    pub(crate) scalar_kind: naga::ScalarKind,
    pub(crate) scalar_width: u8,
    pub(crate) components: u32,
}

pub(crate) struct ShaderDescriptorBinding {
    pub(crate) group: u32,
    pub(crate) binding: u32,
    pub(crate) binding_type: ShaderBindingType,
}
