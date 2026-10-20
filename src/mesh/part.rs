use crate::{
    Error, Result,
    material::MaterialProperties,
    math::Mat4,
    mesh::{Mesh, MeshPrimitive, MeshVertexAttribute, texture::MeshTexture},
    renderers::settings::{PrimitiveTopology, SamplerAddressMode, SamplerFilter},
};

/// Represents one part of a 3D shape.
#[derive(Clone, Copy)]
pub struct MeshPart<'a> {
    pub(super) mesh: &'a Mesh,
    pub(super) primitive: &'a MeshPrimitive,
}

impl<'a> MeshPart<'a> {
    /// Gets the primitive topology of the part.
    pub fn topology(&self) -> PrimitiveTopology {
        self.primitive.topology
    }

    /// Gets the number of components in a vertex with the requested layout.
    pub fn vertex_component_count(&self, vertex_attributes: &[MeshVertexAttribute]) -> usize {
        vertex_attributes
            .iter()
            .map(|attribute| match attribute {
                MeshVertexAttribute::Position | MeshVertexAttribute::Normal => 3,
                MeshVertexAttribute::Tangent
                | MeshVertexAttribute::Color(_)
                | MeshVertexAttribute::JointIndices(_)
                | MeshVertexAttribute::JointWeights(_) => 4,
                MeshVertexAttribute::TextureCoordinates(_) => 2,
            })
            .sum()
    }

    /// Gets the size of a vertex in bytes with the requested layout.
    pub fn vertex_size_bytes(&self, vertex_attributes: &[MeshVertexAttribute]) -> usize {
        self.vertex_component_count(vertex_attributes) * std::mem::size_of::<f32>()
    }

    /// Gets vertices in the requested layout; skinned positions are local to the skin, while other
    /// positions include the file's default transforms.
    pub fn vertices(&self, vertex_attributes: &[MeshVertexAttribute]) -> Result<Vec<f32>> {
        if vertex_attributes.is_empty() {
            return Err(Error::invalid_argument(
                "Mesh vertex attributes cannot be empty.",
            ));
        }

        let component_count = self.vertex_component_count(vertex_attributes);
        let mut vertices = vec![0.0; self.primitive.vertex_count * component_count];
        let mut offset = 0;
        for &attribute in vertex_attributes {
            let (range, width) = match attribute {
                MeshVertexAttribute::Position => (self.primitive.positions_range, 3),
                MeshVertexAttribute::Normal => (self.primitive.normals_range, 3),
                MeshVertexAttribute::Tangent => (self.primitive.tangents_range, 4),
                MeshVertexAttribute::Color(set) => (
                    self.primitive
                        .colors_ranges
                        .iter()
                        .find(|(index, _)| *index as usize == set)
                        .map(|(_, range)| *range),
                    4,
                ),
                MeshVertexAttribute::TextureCoordinates(set) => (
                    self.primitive
                        .tex_coords_ranges
                        .iter()
                        .find(|(index, _)| *index as usize == set)
                        .map(|(_, range)| *range),
                    2,
                ),
                MeshVertexAttribute::JointIndices(set) => (
                    self.primitive
                        .joint_indices_ranges
                        .iter()
                        .find(|(index, _)| *index as usize == set)
                        .map(|(_, range)| *range),
                    4,
                ),
                MeshVertexAttribute::JointWeights(set) => (
                    self.primitive
                        .joint_weights_ranges
                        .iter()
                        .find(|(index, _)| *index as usize == set)
                        .map(|(_, range)| *range),
                    4,
                ),
            };
            let Some((begin, end)) = range else {
                return Err(Error::fail(format!(
                    "Mesh vertex attribute {attribute:?} is not present in the mesh file.",
                )));
            };

            for (vertex, values) in vertices
                .chunks_exact_mut(component_count)
                .zip(self.mesh.vertices[begin..end].chunks_exact(width))
            {
                vertex[offset..offset + width].copy_from_slice(values);
            }
            offset += width;
        }
        Ok(vertices)
    }

    /// Gets the indices referencing the part's vertices.
    pub fn indices(&self) -> &[u32] {
        let (ib, ie) = self.primitive.indices_range;
        &self.mesh.indices[ib..ie]
    }

    /// Gets the part's material properties.
    pub fn material(&self) -> &MaterialProperties {
        &self.mesh.materials[self.primitive.material_index]
    }

    /// Gets the skeleton joint controlling this part's rigid transform, if present.
    pub fn joint_index(&self) -> Option<usize> {
        self.primitive.joint_index
    }

    /// Gets the mapping from vertex joint indices to skeleton joint indices, if a skin is present.
    pub fn skin_joint_indices(&self) -> Option<&[usize]> {
        self.primitive
            .skin_index
            .map(|index| self.mesh.skins[index].joint_indices.as_slice())
    }

    /// Gets inverse bind matrices in skin joint order, if a skin is present.
    pub fn inverse_bind_matrices(&self) -> Option<&[Mat4]> {
        self.primitive
            .skin_index
            .map(|index| self.mesh.skins[index].inverse_bind_matrices.as_slice())
    }

    /// Calculates skinning matrices from compatible global skeleton matrices in vertex joint index
    /// order.
    pub fn joint_matrices(&self, matrices: &[Mat4]) -> Result<Vec<Mat4>> {
        let skin = self
            .primitive
            .skin_index
            .map(|index| &self.mesh.skins[index])
            .ok_or(Error::invalid_operation("Mesh part has no skin."))?;
        skin.joint_indices
            .iter()
            .zip(&skin.inverse_bind_matrices)
            .map(|(&joint, inverse_bind)| {
                matrices
                    .get(joint)
                    .map(|matrix| *matrix * inverse_bind)
                    .ok_or(Error::invalid_argument(
                        "Skeleton matrix index is out of range.",
                    ))
            })
            .collect()
    }

    /// Calculates the animated transform relative to the part's stored vertices. Skinned parts use
    /// joint matrices and return identity here.
    pub fn animation_matrix(&self, matrices: &[Mat4]) -> Result<Mat4> {
        if self.primitive.skin_index.is_some() {
            return Ok(Mat4::IDENTITY);
        }
        let Some(joint) = self.primitive.joint_index else {
            return Ok(Mat4::IDENTITY);
        };

        let skeleton = self
            .mesh
            .skeleton
            .as_ref()
            .ok_or(Error::invalid_operation("Mesh has no skeleton."))?;

        let mut default_matrix = Mat4::IDENTITY;
        let mut current = Some(joint);
        while let Some(index) = current {
            let joint = &skeleton.joints()[index];
            default_matrix = joint.transform.matrix()? * default_matrix;
            current = joint.parent_index;
        }

        if default_matrix.determinant() == 0.0 {
            return Err(Error::invalid_operation(
                "Mesh part's default transform is not invertible.",
            ));
        }

        let matrix = matrices.get(joint).ok_or(Error::invalid_argument(
            "Skeleton matrix index is out of range.",
        ))?;

        Ok(*matrix * default_matrix.inverse())
    }

    /// Gets the number of UV sets present in this part.
    pub fn uv_set_count(&self) -> usize {
        self.primitive.tex_coords_ranges.len()
    }

    /// Gets the number of vertex color sets present in this part.
    pub fn color_set_count(&self) -> usize {
        self.primitive.colors_ranges.len()
    }

    /// Gets the base-color texture's image URI, if present in the mesh file.
    pub fn base_texture_image_uri(&self) -> Option<&str> {
        self.texture_image_uri(&self.primitive.base_color_texture)
    }
    /// Gets the base-color texture's encoded image bytes, if embedded in the mesh file.
    pub fn base_texture_image_data(&self) -> Option<&[u8]> {
        self.texture_image_data(&self.primitive.base_color_texture)
    }
    /// Gets the UV set used by the base-color texture, if present in the mesh file.
    pub fn base_texture_uv_set(&self) -> Option<usize> {
        self.texture_uv_set(&self.primitive.base_color_texture)
    }
    /// Gets the base-color texture's minification, magnification, and optional mipmap filters.
    /// Returns None if the texture is not present in the mesh file; a `None` mipmap filter disables
    /// mipmapping.
    pub fn base_texture_sampler_filters(
        &self,
    ) -> Option<(SamplerFilter, SamplerFilter, Option<SamplerFilter>)> {
        self.texture_sampler_filters(&self.primitive.base_color_texture)
    }
    /// Gets the base-color texture's U and V addressing modes, if present in the mesh file.
    pub fn base_texture_sampler_address_modes(
        &self,
    ) -> Option<(SamplerAddressMode, SamplerAddressMode)> {
        self.texture_sampler_address_modes(&self.primitive.base_color_texture)
    }

    /// Gets the metallic-roughness texture's image URI, if present in the mesh file.
    pub fn arm_texture_image_uri(&self) -> Option<&str> {
        self.texture_image_uri(&self.primitive.arm_texture)
    }
    /// Gets the metallic-roughness texture's encoded image bytes, if present in the mesh file.
    pub fn arm_texture_image_data(&self) -> Option<&[u8]> {
        self.texture_image_data(&self.primitive.arm_texture)
    }
    /// Gets the UV set used by the metallic-roughness texture, if present in the mesh file.
    pub fn arm_texture_uv_set(&self) -> Option<usize> {
        self.texture_uv_set(&self.primitive.arm_texture)
    }
    /// Gets the metallic-roughness texture's minification, magnification, and optional mipmap
    /// filters. Returns None if the texture is not present in the mesh file; a missing mipmap
    /// filter disables mipmapping.
    pub fn arm_texture_sampler_filters(
        &self,
    ) -> Option<(SamplerFilter, SamplerFilter, Option<SamplerFilter>)> {
        self.texture_sampler_filters(&self.primitive.arm_texture)
    }
    /// Gets the metallic-roughness texture's U and V addressing modes, if present in the mesh file.
    pub fn arm_texture_sampler_address_modes(
        &self,
    ) -> Option<(SamplerAddressMode, SamplerAddressMode)> {
        self.texture_sampler_address_modes(&self.primitive.arm_texture)
    }

    /// Gets the normal texture's image URI, if present in the mesh file.
    pub fn normal_texture_image_uri(&self) -> Option<&str> {
        self.texture_image_uri(&self.primitive.normal_texture)
    }
    /// Gets the normal texture's encoded image bytes, if present in the mesh file.
    pub fn normal_texture_image_data(&self) -> Option<&[u8]> {
        self.texture_image_data(&self.primitive.normal_texture)
    }
    /// Gets the UV set used by the normal texture, if present in the mesh file.
    pub fn normal_texture_uv_set(&self) -> Option<usize> {
        self.texture_uv_set(&self.primitive.normal_texture)
    }
    /// Gets the normal texture's minification, magnification, and optional mipmap filters. Returns
    /// None if the texture is not present in the mesh file; a missing mipmap filter disables
    /// mipmapping.
    pub fn normal_texture_sampler_filters(
        &self,
    ) -> Option<(SamplerFilter, SamplerFilter, Option<SamplerFilter>)> {
        self.texture_sampler_filters(&self.primitive.normal_texture)
    }
    /// Gets the normal texture's U and V addressing modes, if present in the mesh file.
    pub fn normal_texture_sampler_address_modes(
        &self,
    ) -> Option<(SamplerAddressMode, SamplerAddressMode)> {
        self.texture_sampler_address_modes(&self.primitive.normal_texture)
    }

    /// Gets the emissive texture's image URI, if present in the mesh file.
    pub fn emissive_texture_image_uri(&self) -> Option<&str> {
        self.texture_image_uri(&self.primitive.emissive_texture)
    }
    /// Gets the emissive texture's encoded image bytes, if present in the mesh file.
    pub fn emissive_texture_image_data(&self) -> Option<&[u8]> {
        self.texture_image_data(&self.primitive.emissive_texture)
    }
    /// Gets the UV set used by the emissive texture, if present in the mesh file.
    pub fn emissive_texture_uv_set(&self) -> Option<usize> {
        self.texture_uv_set(&self.primitive.emissive_texture)
    }
    /// Gets the emissive texture's minification, magnification, and optional mipmap filters.
    /// Returns None if the texture is not present in the mesh file; a missing mipmap filter
    /// disables mipmapping.
    pub fn emissive_texture_sampler_filters(
        &self,
    ) -> Option<(SamplerFilter, SamplerFilter, Option<SamplerFilter>)> {
        self.texture_sampler_filters(&self.primitive.emissive_texture)
    }
    /// Gets the emissive texture's U and V addressing modes, if present in the mesh file.
    pub fn emissive_texture_sampler_address_modes(
        &self,
    ) -> Option<(SamplerAddressMode, SamplerAddressMode)> {
        self.texture_sampler_address_modes(&self.primitive.emissive_texture)
    }

    /// Gets the occlusion texture's image URI, if present in the mesh file.
    pub fn occlusion_texture_image_uri(&self) -> Option<&str> {
        self.texture_image_uri(&self.primitive.occlusion_texture)
    }
    /// Gets the occlusion texture's encoded image bytes, if present in the mesh file.
    pub fn occlusion_texture_image_data(&self) -> Option<&[u8]> {
        self.texture_image_data(&self.primitive.occlusion_texture)
    }
    /// Gets the UV set used by the occlusion texture, if present in the mesh file.
    pub fn occlusion_texture_uv_set(&self) -> Option<usize> {
        self.texture_uv_set(&self.primitive.occlusion_texture)
    }
    /// Gets the occlusion texture's minification, magnification, and optional mipmap filters.
    /// Returns None if the texture is not present in the mesh file; a missing mipmap filter
    /// disables mipmapping.
    pub fn occlusion_texture_sampler_filters(
        &self,
    ) -> Option<(SamplerFilter, SamplerFilter, Option<SamplerFilter>)> {
        self.texture_sampler_filters(&self.primitive.occlusion_texture)
    }
    /// Gets the occlusion texture's U and V addressing modes, if present in the mesh file.
    pub fn occlusion_texture_sampler_address_modes(
        &self,
    ) -> Option<(SamplerAddressMode, SamplerAddressMode)> {
        self.texture_sampler_address_modes(&self.primitive.occlusion_texture)
    }

    fn texture_image_uri(&self, texture: &Option<MeshTexture>) -> Option<&str> {
        texture.map(|t| self.mesh.texture_images[t.image_index].0.as_str())
    }
    fn texture_image_data(&self, texture: &Option<MeshTexture>) -> Option<&[u8]> {
        if let Some(texture) = texture {
            let data = self.mesh.texture_images[texture.image_index].1.as_slice();
            match data.is_empty() {
                true => None,
                false => Some(data),
            }
        } else {
            None
        }
    }
    fn texture_uv_set(&self, texture: &Option<MeshTexture>) -> Option<usize> {
        texture.map(|t| t.uv_set)
    }
    fn texture_sampler_filters(
        &self,
        texture: &Option<MeshTexture>,
    ) -> Option<(SamplerFilter, SamplerFilter, Option<SamplerFilter>)> {
        texture.map(|t| {
            let sampler = &self.mesh.samplers[t.sampler_index];
            (
                sampler.min_filter,
                sampler.mag_filter,
                sampler.mipmap_filter,
            )
        })
    }
    fn texture_sampler_address_modes(
        &self,
        texture: &Option<MeshTexture>,
    ) -> Option<(SamplerAddressMode, SamplerAddressMode)> {
        texture.map(|t| {
            let sampler = &self.mesh.samplers[t.sampler_index];
            (sampler.address_mode_u, sampler.address_mode_v)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertices_interleaves_vertex_attributes() {
        const FILE_PATH: &str = "assets/ship/ship_pinnace_4k.gltf";
        let mesh = Mesh::from_file(FILE_PATH).unwrap();
        let attributes = [
            MeshVertexAttribute::TextureCoordinates(0),
            MeshVertexAttribute::Position,
            MeshVertexAttribute::Normal,
        ];
        for part in mesh.parts() {
            let positions = part.vertices(&[MeshVertexAttribute::Position]).unwrap();
            let normals = part.vertices(&[MeshVertexAttribute::Normal]).unwrap();
            let tex_coords = part
                .vertices(&[MeshVertexAttribute::TextureCoordinates(0)])
                .unwrap();
            let vertices = part.vertices(&attributes).unwrap();
            assert_eq!(part.vertex_component_count(&attributes), 8);
            assert_eq!(
                part.vertex_size_bytes(&attributes),
                8 * std::mem::size_of::<f32>()
            );
            assert_eq!(vertices.len(), positions.len() / 3 * 8);
            for (j, vertex) in vertices.as_chunks::<8>().0.iter().enumerate() {
                assert_eq!(&vertex[..2], &tex_coords[j * 2..j * 2 + 2]);
                assert_eq!(&vertex[2..5], &positions[j * 3..j * 3 + 3]);
                assert_eq!(&vertex[5..], &normals[j * 3..j * 3 + 3]);
            }
        }
    }

    #[test]
    fn test_vertices_missing_attributes() {
        let mesh = Mesh {
            vertices: vec![1.0, 2.0, 3.0],
            primitives: vec![MeshPrimitive {
                vertex_count: 1,
                positions_range: Some((0, 3)),
                ..Default::default()
            }],
            ..Default::default()
        };
        let part = mesh.parts().next().unwrap();
        assert_eq!(
            part.vertices(&[MeshVertexAttribute::Position]).unwrap(),
            vec![1.0, 2.0, 3.0]
        );
        assert!(matches!(part.vertices(&[]), Err(Error::InvalidArgument(_))));
        for attribute in [
            MeshVertexAttribute::Normal,
            MeshVertexAttribute::Tangent,
            MeshVertexAttribute::TextureCoordinates(0),
            MeshVertexAttribute::Color(0),
        ] {
            assert!(matches!(part.vertices(&[attribute]), Err(Error::Fail(_))));
            assert!(matches!(
                part.vertices(&[MeshVertexAttribute::Position, attribute]),
                Err(Error::Fail(_))
            ));
        }
        let mesh = Mesh {
            primitives: vec![MeshPrimitive::default()],
            ..Default::default()
        };
        assert!(matches!(
            mesh.parts()
                .next()
                .unwrap()
                .vertices(&[MeshVertexAttribute::Position]),
            Err(Error::Fail(_))
        ));
    }

    #[test]
    fn test_vertices_uv_sets() {
        let mesh = Mesh {
            vertices: vec![
                1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8,
            ],
            primitives: vec![MeshPrimitive {
                vertex_count: 2,
                positions_range: Some((0, 6)),
                tex_coords_ranges: vec![(0, (6, 10)), (1, (10, 14))],
                ..Default::default()
            }],
            ..Default::default()
        };
        let part = mesh.parts().next().unwrap();
        let attributes = [
            MeshVertexAttribute::TextureCoordinates(1),
            MeshVertexAttribute::Position,
            MeshVertexAttribute::TextureCoordinates(0),
        ];
        assert_eq!(part.vertex_component_count(&attributes), 7);
        assert_eq!(part.vertex_size_bytes(&attributes), 28);
        assert_eq!(
            part.vertices(&attributes).unwrap(),
            vec![
                0.5, 0.6, 1.0, 2.0, 3.0, 0.1, 0.2, 0.7, 0.8, 4.0, 5.0, 6.0, 0.3, 0.4,
            ]
        );
        let attributes = [MeshVertexAttribute::TextureCoordinates(1)];
        assert_eq!(part.vertex_component_count(&attributes), 2);
        assert_eq!(part.vertex_size_bytes(&attributes), 8);
        assert_eq!(
            part.vertices(&attributes).unwrap(),
            vec![0.5, 0.6, 0.7, 0.8]
        );
        for set in [2, usize::MAX] {
            assert!(
                part.vertices(&[MeshVertexAttribute::TextureCoordinates(set)])
                    .is_err()
            );
        }
    }
}
