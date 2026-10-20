use crate::renderers::settings::{SamplerAddressMode, SamplerFilter};

#[derive(Default, Clone, Copy)]
pub(super) struct MeshSampler {
    pub(super) min_filter: SamplerFilter,
    pub(super) mag_filter: SamplerFilter,
    pub(super) mipmap_filter: Option<SamplerFilter>,
    pub(super) address_mode_u: SamplerAddressMode,
    pub(super) address_mode_v: SamplerAddressMode,
}
