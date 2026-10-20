mod error;
mod gltf;
mod part;
mod primitive;
mod sampler;
mod skin;
mod texture;

use std::path::Path;

pub use part::MeshPart;

use crate::{
    Error, Result,
    animation::Animation,
    material::MaterialProperties,
    mesh::{primitive::MeshPrimitive, sampler::MeshSampler, skin::MeshSkin},
    renderers::settings::PrimitiveTopology,
    skeleton::Skeleton,
};

/// Defines the supported file types for loading meshes.
enum MeshFileType {
    Gltf,
}

/// Defines the vertex attributes and their order in a mesh part's vertex layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeshVertexAttribute {
    /// The 3D position of the vertex.
    Position,
    /// The 3D surface normal at the vertex.
    Normal,
    /// The 3D tangent direction and the sign used to reconstruct the bitangent.
    Tangent,
    /// The 2D texture coordinates from the specified UV set.
    TextureCoordinates(usize),
    /// The vertex color from the specified color set.
    Color(usize),
    /// Four skin-local joint indices from the specified set, represented as f32 components.
    JointIndices(usize),
    /// Four joint weights from the specified set, normalized across all weight sets.
    JointWeights(usize),
}

/// Represents a 3D shape assembled from parts, with each part describing points on its surface and
/// how they connect.
#[derive(Default, Clone)]
pub struct Mesh {
    vertices: Vec<f32>,
    indices: Vec<u32>,
    primitives: Vec<MeshPrimitive>,
    materials: Vec<MaterialProperties>,
    skins: Vec<MeshSkin>,
    skeleton: Option<Skeleton>,
    animations: Vec<Animation>,
    samplers: Vec<MeshSampler>,
    texture_images: Vec<(String, Vec<u8>)>,
}

impl Mesh {
    /// Gets the number of parts in the mesh.
    pub fn part_count(&self) -> usize {
        self.primitives.len()
    }

    /// Iterates over the mesh's parts.
    pub fn parts<'a>(&'a self) -> impl Iterator<Item = MeshPart<'a>> {
        self.primitives.iter().map(move |primitive| MeshPart {
            mesh: self,
            primitive,
        })
    }

    /// Gets the shared skeleton used by the mesh's skin bindings and animations, if present.
    pub fn skeleton(&self) -> Option<&Skeleton> {
        self.skeleton.as_ref()
    }

    /// Gets animation clips in file order, targeting this mesh's skeleton.
    pub fn animations(&self) -> &[Animation] {
        &self.animations
    }

    /// Loads the mesh from a file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::io("Mesh file not found."));
        }

        match Self::get_file_type(path)? {
            MeshFileType::Gltf => gltf::from_file(path),
        }
    }

    fn get_file_type(path: &Path) -> Result<MeshFileType> {
        if gltf::is_gltf_file(path)? {
            return Ok(MeshFileType::Gltf);
        }

        Err(Error::io("Unsupported mesh file type"))
    }

    fn append_vertex_attribute<const N: usize>(
        &mut self,
        values: impl Iterator<Item = [f32; N]>,
        vertex_count: usize,
        topology: PrimitiveTopology,
        swap_triangle_winding: bool,
    ) -> Result<(usize, usize)> {
        let begin = self.vertices.len();
        self.vertices.extend(values.flatten());
        if self.vertices.len() - begin != vertex_count * N {
            return Err(Error::fail(
                "Mesh vertex attributes have different lengths.",
            ));
        }
        if swap_triangle_winding {
            Self::swap_triangle_winding(topology, &mut self.vertices, begin, N);
        }
        Ok((begin, self.vertices.len()))
    }

    fn swap_triangle_winding<T: Copy>(
        topology: PrimitiveTopology,
        elements: &mut Vec<T>,
        start_index: usize,
        element_size: usize,
    ) {
        match topology {
            PrimitiveTopology::TriangleList => {
                for triangle in elements[start_index..].chunks_exact_mut(3 * element_size) {
                    let (_, remaining) = triangle.split_at_mut(element_size);
                    let (second, third) = remaining.split_at_mut(element_size);
                    second.swap_with_slice(third);
                }
            }
            PrimitiveTopology::TriangleStrip => {
                if (elements.len() - start_index) / element_size >= 3 {
                    // A leading degenerate triangle shifts the strip parity, reversing the winding
                    // of every original triangle without changing its vertices.
                    elements.extend_from_within(start_index..start_index + element_size);
                    elements[start_index..].rotate_right(element_size);
                }
            }
            PrimitiveTopology::PointList
            | PrimitiveTopology::LineList
            | PrimitiveTopology::LineStrip => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_file() {
        {
            const FILE_PATH: &str = "assets/ship/ship_pinnace_4k.gltf";
            assert!(Mesh::from_file(FILE_PATH).is_ok());
        }

        {
            const FILE_PATH: &str = "assets/ship/textures/ship_pinnace_hull_diff_4k.jpg";
            assert!(matches!(Mesh::from_file(FILE_PATH), Err(Error::Io(_))));
        }
    }

    #[test]
    fn test_swap_triangle_winding() {
        for topology in [
            PrimitiveTopology::PointList,
            PrimitiveTopology::LineList,
            PrimitiveTopology::LineStrip,
        ] {
            let mut indices = vec![0u32, 1, 2, 3, 4, 5];
            Mesh::swap_triangle_winding(topology, &mut indices, 0, 1);
            assert_eq!(indices, vec![0, 1, 2, 3, 4, 5]);
        }

        {
            let mut indices = vec![0u32, 1, 2, 3, 4, 5];
            Mesh::swap_triangle_winding(PrimitiveTopology::TriangleList, &mut indices, 0, 1);
            assert_eq!(indices, vec![0, 2, 1, 3, 5, 4]);
        }

        // Decode the strip to check that every original triangle has reversed winding. Ignore
        // degenerate triangles and normalize cyclic rotations for comparison.
        let strip_triangles = |indices: &[u32]| {
            indices
                .windows(3)
                .enumerate()
                .filter_map(|(i, window)| {
                    let [a, b, c] = [window[0], window[1], window[2]];
                    if a == b || b == c || a == c {
                        return None;
                    }
                    let mut triangle = if i % 2 == 0 { [a, b, c] } else { [b, a, c] };
                    while triangle[0] > triangle[1] || triangle[0] > triangle[2] {
                        triangle.rotate_left(1);
                    }
                    Some(triangle)
                })
                .collect::<Vec<_>>()
        };
        for count in 0..=8 {
            let mut indices: Vec<u32> = (0..count).collect();
            let original = indices.clone();
            let mut expected = strip_triangles(&indices);
            for triangle in &mut expected {
                triangle.swap(1, 2);
            }
            Mesh::swap_triangle_winding(PrimitiveTopology::TriangleStrip, &mut indices, 0, 1);
            assert_eq!(strip_triangles(&indices), expected);
            if count < 3 {
                assert_eq!(indices, original);
            }

            // Earlier primitives must remain unchanged in shared index storage.
            let prefix = [10, 11, 12];
            let mut shared_indices = prefix.to_vec();
            shared_indices.extend_from_slice(&original);
            Mesh::swap_triangle_winding(
                PrimitiveTopology::TriangleStrip,
                &mut shared_indices,
                prefix.len(),
                1,
            );
            assert_eq!(&shared_indices[..prefix.len()], &prefix);
            assert_eq!(&shared_indices[prefix.len()..], indices.as_slice());
        }

        // Non-indexed winding must preserve the complete interleaved vertex records.
        let original: Vec<f32> = (0..48).map(|value| value as f32).collect();
        let mut vertices = original.clone();
        Mesh::swap_triangle_winding(PrimitiveTopology::TriangleList, &mut vertices, 0, 8);
        let expected: Vec<f32> = [0, 2, 1, 3, 5, 4]
            .into_iter()
            .flat_map(|i| original[i * 8..i * 8 + 8].iter().copied())
            .collect();
        assert_eq!(vertices, expected);

        let mut vertices = original.clone();
        Mesh::swap_triangle_winding(PrimitiveTopology::TriangleStrip, &mut vertices, 0, 8);
        assert_eq!(&vertices[..8], &original[..8]);
        assert_eq!(&vertices[8..], original.as_slice());
        for count in 0..3 {
            let mut vertices = original[..count * 8].to_vec();
            Mesh::swap_triangle_winding(PrimitiveTopology::TriangleStrip, &mut vertices, 0, 8);
            assert_eq!(vertices, original[..count * 8]);
        }
    }
}
