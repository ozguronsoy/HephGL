use crate::math::Mat4;

#[derive(Clone)]
pub(super) struct MeshSkin {
    pub(super) joint_indices: Vec<usize>,
    pub(super) inverse_bind_matrices: Vec<Mat4>,
}
