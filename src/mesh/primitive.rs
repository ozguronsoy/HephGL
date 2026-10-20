use crate::{mesh::texture::MeshTexture, renderers::settings::PrimitiveTopology};

#[derive(Default, Clone)]
pub(super) struct MeshPrimitive {
    pub(super) topology: PrimitiveTopology,
    pub(super) vertex_count: usize,
    // start index, end index for all range tuples.
    pub(super) positions_range: Option<(usize, usize)>,
    pub(super) normals_range: Option<(usize, usize)>,
    pub(super) tangents_range: Option<(usize, usize)>,
    // Color set and its start/end component indices.
    pub(super) colors_ranges: Vec<(u32, (usize, usize))>,
    // UV set and its start/end component indices.
    pub(super) tex_coords_ranges: Vec<(u32, (usize, usize))>,
    pub(super) joint_indices_ranges: Vec<(u32, (usize, usize))>,
    pub(super) joint_weights_ranges: Vec<(u32, (usize, usize))>,
    pub(super) indices_range: (usize, usize),
    pub(super) material_index: usize,
    pub(super) joint_index: Option<usize>,
    pub(super) skin_index: Option<usize>,
    pub(super) base_color_texture: Option<MeshTexture>,
    pub(super) arm_texture: Option<MeshTexture>,
    pub(super) normal_texture: Option<MeshTexture>,
    pub(super) emissive_texture: Option<MeshTexture>,
    pub(super) occlusion_texture: Option<MeshTexture>,
}
