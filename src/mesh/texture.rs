#[derive(Default, Clone, Copy)]
pub(super) struct MeshTexture {
    pub(super) image_index: usize,
    pub(super) uv_set: usize,
    pub(super) sampler_index: usize,
}
