use crate::renderers::{RendererBuffer, ResourceBinding};

/// The arguments of a graphics command, determined by the pipeline's shader stages.
pub enum GraphicsCommandArgs<'a> {
    /// Arguments for pipelines that use a vertex shader.
    VertexArgs {
        index_buffer: Option<&'a dyn RendererBuffer>,
        vertex_bindings: &'a [ResourceBinding<'a>],
        draw_count: u32,
        instance_count: u32,
    },

    /// Arguments for pipelines that use a mesh shader.
    MeshArgs { group_count: (u32, u32, u32) },
}
