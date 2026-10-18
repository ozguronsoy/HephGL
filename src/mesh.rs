use std::{
    fs::File,
    io::{BufReader, Read, Seek},
    path::Path,
};

use gltf::texture::MinFilter;
use renkrs::RGBA;

use crate::{
    Error, Result,
    material::MaterialProperties,
    math::{Mat3, Mat4, Vec3},
    renderers::settings::{PrimitiveTopology, SamplerAddressMode, SamplerFilter},
};

/// Defines the supported file types for loading meshes.
enum MeshFileType {
    Gltf,
}

/// Defines the vertex attributes to read when loading a mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeshVertexAttribute {
    /// The 3D position of the vertex.
    Position,
    /// The 3D surface normal at the vertex.
    Normal,
    /// The 3D tangent direction and the sign used to reconstruct the bitangent.
    Tangent,
    /// The 2D texture coordinates from the specified UV set.
    TextureCoordinates(u32),
    /// The vertex color from the specified color set.
    Color(u32),
}

#[derive(Default, Clone, Copy)]
struct MeshTexture {
    image_index: usize,
    uv_set: usize,
    sampler_index: usize,
}

#[derive(Default, Clone, Copy)]
struct MeshSampler {
    min_filter: SamplerFilter,
    mag_filter: SamplerFilter,
    mipmap_filter: Option<SamplerFilter>,
    address_mode_u: SamplerAddressMode,
    address_mode_v: SamplerAddressMode,
}

/// One part of a 3D shape, described by points and references that connect them.
// TODO: We use too many vectors here. Refactor this to have begin/end iterators, so we can store
// all indices and vertices in two vectors.
#[derive(Default, Clone)]
struct MeshPrimitive {
    vertices: Vec<f32>,
    indices: Vec<u32>,
    material_index: usize,
    base_color_texture: Option<MeshTexture>,
    arm_texture: Option<MeshTexture>,
    normal_texture: Option<MeshTexture>,
    emissive_texture: Option<MeshTexture>,
    occlusion_texture: Option<MeshTexture>,
}

/// Represents one part of a 3D shape.
#[derive(Clone, Copy)]
pub struct MeshPart<'a> {
    mesh: &'a Mesh,
    primitive: &'a MeshPrimitive,
}

/// Represents a 3D shape assembled from parts, with each part describing points on its surface and
/// how they connect.
#[derive(Default, Clone)]
pub struct Mesh {
    vertex_component_count: usize,
    primitives: Vec<MeshPrimitive>,
    materials: Vec<MaterialProperties>,
    samplers: Vec<MeshSampler>,
    texture_images: Vec<Vec<u8>>,
}

impl Mesh {
    /// Gets the number of components in a mesh vertex.
    pub fn vertex_component_count(&self) -> usize {
        self.vertex_component_count
    }

    /// Gets the size of a mesh vertex in bytes.
    pub fn vertex_size_bytes(&self) -> usize {
        self.vertex_component_count * std::mem::size_of::<f32>()
    }

    /// Gets the vertices of each primitive.
    pub fn vertices(&self) -> Vec<&Vec<f32>> {
        self.primitives.iter().map(|p| &p.vertices).collect()
    }

    /// Gets the flattened vertices.
    pub fn flattened_vertices(&self) -> Vec<f32> {
        self.primitives
            .iter()
            .flat_map(|p| p.vertices.clone())
            .collect()
    }

    /// Gets the indices of each primitive.
    pub fn indices(&self) -> Vec<&Vec<u32>> {
        self.primitives.iter().map(|p| &p.indices).collect()
    }

    /// Gets the flattened indices.
    pub fn flattened_indices(&self) -> Vec<u32> {
        let mut flattened_indices =
            Vec::with_capacity(self.primitives.iter().map(|p| p.indices.len()).sum());
        let mut vertex_offset = 0;
        for primitive in &self.primitives {
            flattened_indices.extend(primitive.indices.iter().map(|index| vertex_offset + index));
            vertex_offset += (primitive.vertices.len() / self.vertex_component_count) as u32;
        }
        flattened_indices
    }

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

    /// Checks whether any mesh part has an embedded base-color texture.
    pub fn has_embedded_base_color_texture(&self) -> bool {
        self.primitives
            .iter()
            .any(|p| p.base_color_texture.is_some())
    }
    /// Checks whether any mesh part has an embedded metallic-roughness texture.
    pub fn has_embedded_arm_texture(&self) -> bool {
        self.primitives.iter().any(|p| p.arm_texture.is_some())
    }
    /// Checks whether any mesh part has an embedded normal texture.
    pub fn has_embedded_normal_texture(&self) -> bool {
        self.primitives.iter().any(|p| p.normal_texture.is_some())
    }
    /// Checks whether any mesh part has an embedded emissive texture.
    pub fn has_embedded_emissive_texture(&self) -> bool {
        self.primitives.iter().any(|p| p.emissive_texture.is_some())
    }
    /// Checks whether any mesh part has an embedded occlusion texture.
    pub fn has_embedded_occlusion_texture(&self) -> bool {
        self.primitives
            .iter()
            .any(|p| p.occlusion_texture.is_some())
    }

    /// Loads the mesh from a file.
    pub fn from_file<P: AsRef<Path>>(
        path: P,
        vertex_attributes: &[MeshVertexAttribute],
    ) -> Result<Self> {
        if vertex_attributes.is_empty() {
            return Err(Error::invalid_argument(
                "Mesh vertex attributes cannot be empty.",
            ));
        }
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::io("Mesh file not found."));
        }

        match Self::get_file_type(path)? {
            MeshFileType::Gltf => Self::from_gltf_file(path, vertex_attributes),
        }
    }

    fn get_file_type(path: &Path) -> Result<MeshFileType> {
        let mut reader = BufReader::new(File::open(path)?);
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;

        if &magic == b"glTF" {
            return Ok(MeshFileType::Gltf);
        }

        reader.rewind()?;
        if gltf::json::deserialize::from_reader::<_, gltf::json::Root>(reader).is_ok() {
            // JSON glTF file.
            return Ok(MeshFileType::Gltf);
        }

        Err(Error::io("Unsupported mesh file type"))
    }

    fn calculate_vertex_component_count(vertex_attributes: &[MeshVertexAttribute]) -> usize {
        let mut vertex_component_count = 0;
        for &vertex_attribute in vertex_attributes {
            vertex_component_count += match vertex_attribute {
                MeshVertexAttribute::Position => 3,
                MeshVertexAttribute::Normal => 3,
                MeshVertexAttribute::Tangent => 4,
                MeshVertexAttribute::TextureCoordinates(_) => 2,
                // We always use RGBA,
                MeshVertexAttribute::Color(_) => 4,
            }
        }
        vertex_component_count
    }

    fn from_gltf_file(path: &Path, vertex_attributes: &[MeshVertexAttribute]) -> Result<Self> {
        let mut mesh = Self::default();

        let gltf = gltf::Gltf::open(path)?;
        let directory = path
            .parent()
            .ok_or(Error::io("Failed to open the gltf file"))?;

        let buffers = gltf
            .buffers()
            .map(|buffer| -> Result<Vec<u8>> {
                match buffer.source() {
                    gltf::buffer::Source::Uri(uri) => Ok(std::fs::read(directory.join(uri))?),
                    gltf::buffer::Source::Bin => gltf
                        .blob
                        .clone()
                        .ok_or(Error::io("Failed to read the glTF file.")),
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let scene = gltf
            .default_scene()
            .ok_or(Error::io("Failed to read the glTF file."))?;
        let mut nodes: Vec<_> = scene.nodes().map(|node| (node, Mat4::IDENTITY)).collect();
        let coordinate_conversion = Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0));

        // First material is the default glTF material.
        mesh.materials.reserve(gltf.materials().len() + 1);
        mesh.materials.push(MaterialProperties {
            metallicness: 1.0,
            ..Default::default()
        });
        for ref material in gltf.materials() {
            let pbr = material.pbr_metallic_roughness();
            let [r, g, b, a] = pbr.base_color_factor();
            mesh.materials.push(MaterialProperties {
                base_color: RGBA::new(r, g, b, a),
                roughness: pbr.roughness_factor(),
                metallicness: pbr.metallic_factor(),
                normal_scale: material
                    .normal_texture()
                    .map_or(1.0, |texture| texture.scale()),
                ..Default::default()
            });
        }

        // First sampler is the default glTF sampler.
        mesh.samplers.reserve(gltf.samplers().len() + 1);
        mesh.samplers.push(MeshSampler {
            address_mode_u: SamplerAddressMode::Repeat,
            address_mode_v: SamplerAddressMode::Repeat,
            ..Default::default()
        });
        for ref sampler in gltf.samplers() {
            fn convert_wrapping_mode(mode: gltf::texture::WrappingMode) -> SamplerAddressMode {
                match mode {
                    gltf::texture::WrappingMode::ClampToEdge => SamplerAddressMode::ClampToEdge,
                    gltf::texture::WrappingMode::MirroredRepeat => {
                        SamplerAddressMode::MirroredRepeat
                    }
                    gltf::texture::WrappingMode::Repeat => SamplerAddressMode::Repeat,
                }
            }
            let (min_filter, mipmap_filter) =
                sampler
                    .min_filter()
                    .map_or((SamplerFilter::default(), None), |f| match f {
                        MinFilter::Linear => (SamplerFilter::Linear, None),
                        MinFilter::LinearMipmapLinear => {
                            (SamplerFilter::Linear, Some(SamplerFilter::Linear))
                        }
                        MinFilter::LinearMipmapNearest => {
                            (SamplerFilter::Linear, Some(SamplerFilter::Nearest))
                        }
                        MinFilter::Nearest => (SamplerFilter::Nearest, None),
                        MinFilter::NearestMipmapLinear => {
                            (SamplerFilter::Nearest, Some(SamplerFilter::Linear))
                        }
                        MinFilter::NearestMipmapNearest => {
                            (SamplerFilter::Nearest, Some(SamplerFilter::Nearest))
                        }
                    });
            mesh.samplers.push(MeshSampler {
                address_mode_u: convert_wrapping_mode(sampler.wrap_s()),
                address_mode_v: convert_wrapping_mode(sampler.wrap_t()),
                min_filter,
                mipmap_filter,
                mag_filter: sampler
                    .mag_filter()
                    .map_or(SamplerFilter::default(), |f| match f {
                        gltf::texture::MagFilter::Linear => SamplerFilter::Linear,
                        gltf::texture::MagFilter::Nearest => SamplerFilter::Nearest,
                    }),
            });
        }

        mesh.texture_images.reserve(gltf.images().len());
        for ref image in gltf.images() {
            let view = match image.source() {
                gltf::image::Source::View { view, .. } => view,
                gltf::image::Source::Uri { .. } => {
                    // Push an empty vector to keep the indices of primitives valid.
                    mesh.texture_images.push(Vec::default());
                    continue;
                }
            };
            let buffer = &buffers[view.buffer().index()];
            mesh.texture_images
                .push(buffer[view.offset()..view.offset() + view.length()].to_vec());
        }

        mesh.vertex_component_count = Self::calculate_vertex_component_count(vertex_attributes);
        while let Some((node, parent_transform)) = nodes.pop() {
            let node_transform =
                parent_transform * Mat4::from_cols_array_2d(&node.transform().matrix());
            for child in node.children() {
                nodes.push((child, node_transform));
            }

            let Some(gltf_mesh) = node.mesh() else {
                continue;
            };
            let transform = coordinate_conversion * node_transform;
            let normal_transform = Mat3::from_mat4(transform).inverse().transpose();
            let swap_triangle_winding = transform.determinant() < 0.0;

            let gltf_primitives = gltf_mesh.primitives();
            mesh.primitives.reserve(gltf_primitives.len());
            for gltf_primitive in gltf_primitives {
                let mut primitive = MeshPrimitive::default();

                let reader =
                    gltf_primitive.reader(|buffer| Some(buffers[buffer.index()].as_slice()));
                let mut attribute_offset = 0;
                for &vertex_attribute in vertex_attributes {
                    let attribute_values: Vec<f32> = match vertex_attribute {
                        MeshVertexAttribute::Position => reader
                            .read_positions()
                            .ok_or(Error::fail("Failed to read the mesh primitive positions."))?
                            .flat_map(|p| transform.transform_point3(Vec3::from(p)).to_array())
                            .collect(),
                        MeshVertexAttribute::Normal => reader
                            .read_normals()
                            .ok_or(Error::fail("Failed to read the mesh primitive normals."))?
                            .flat_map(|n| {
                                (normal_transform * Vec3::from_array(n))
                                    .normalize()
                                    .to_array()
                            })
                            .collect(),
                        MeshVertexAttribute::Tangent => reader
                            .read_tangents()
                            .ok_or(Error::fail("Failed to read the mesh primitive tangents."))?
                            .flat_map(|t| {
                                let tangent = transform
                                    .transform_vector3(Vec3::new(t[0], t[1], t[2]))
                                    .normalize();
                                let handedness = match swap_triangle_winding {
                                    true => -t[3],
                                    false => t[3],
                                };
                                [tangent.x, tangent.y, tangent.z, handedness]
                            })
                            .collect(),
                        MeshVertexAttribute::TextureCoordinates(set) => reader
                            .read_tex_coords(set)
                            .ok_or(Error::fail(
                                "Failed to read the mesh primitive texture coordinates.",
                            ))?
                            .into_f32()
                            .flatten()
                            .collect(),
                        MeshVertexAttribute::Color(set) => reader
                            .read_colors(set)
                            .ok_or(Error::fail("Failed to read the mesh primitive colors."))?
                            .into_rgba_f32()
                            .flatten()
                            .collect(),
                    };
                    let component_count =
                        Self::calculate_vertex_component_count(&[vertex_attribute]);
                    let vertex_count = attribute_values.len() / component_count;
                    if attribute_offset == 0 {
                        primitive
                            .vertices
                            .resize(vertex_count * mesh.vertex_component_count, 0.0);
                    } else if vertex_count != primitive.vertices.len() / mesh.vertex_component_count
                    {
                        return Err(Error::fail(
                            "Mesh vertex attributes have different lengths.",
                        ));
                    }

                    for (vertex, attribute) in primitive
                        .vertices
                        .chunks_exact_mut(mesh.vertex_component_count)
                        .zip(attribute_values.chunks_exact(component_count))
                    {
                        vertex[attribute_offset..attribute_offset + component_count]
                            .copy_from_slice(attribute);
                    }
                    attribute_offset += component_count;
                }

                let topology = gltf_primitive.mode().try_into()?;
                if let Some(indices) = reader.read_indices() {
                    primitive.indices = indices.into_u32().collect();
                    if swap_triangle_winding {
                        Self::swap_triangle_winding(topology, &mut primitive.indices, 1);
                    }
                } else if swap_triangle_winding {
                    Self::swap_triangle_winding(
                        topology,
                        &mut primitive.vertices,
                        mesh.vertex_component_count,
                    );
                }

                let material = gltf_primitive.material();
                primitive.material_index = material.index().map_or(0, |i| i + 1);
                let pbr = material.pbr_metallic_roughness();
                primitive.base_color_texture = pbr
                    .base_color_texture()
                    .map(|i| MeshTexture::from_gltf_texture(&i.texture(), i.tex_coord()));
                primitive.arm_texture = pbr
                    .metallic_roughness_texture()
                    .map(|i| MeshTexture::from_gltf_texture(&i.texture(), i.tex_coord()));
                primitive.normal_texture = material
                    .normal_texture()
                    .map(|i| MeshTexture::from_gltf_texture(&i.texture(), i.tex_coord()));
                primitive.emissive_texture = material
                    .emissive_texture()
                    .map(|i| MeshTexture::from_gltf_texture(&i.texture(), i.tex_coord()));
                primitive.occlusion_texture = material
                    .occlusion_texture()
                    .map(|i| MeshTexture::from_gltf_texture(&i.texture(), i.tex_coord()));

                mesh.primitives.push(primitive);
            }
        }

        Ok(mesh)
    }

    fn swap_triangle_winding<T: Copy>(
        topology: PrimitiveTopology,
        elements: &mut Vec<T>,
        element_size: usize,
    ) {
        match topology {
            PrimitiveTopology::TriangleList => {
                for triangle in elements.chunks_exact_mut(3 * element_size) {
                    let (_, remaining) = triangle.split_at_mut(element_size);
                    let (second, third) = remaining.split_at_mut(element_size);
                    second.swap_with_slice(third);
                }
            }
            PrimitiveTopology::TriangleStrip => {
                if elements.len() / element_size >= 3 {
                    // A leading degenerate triangle shifts the strip parity, reversing the
                    // winding of every original triangle without changing its vertices.
                    elements.extend_from_within(..element_size);
                    elements.rotate_right(element_size);
                }
            }
            PrimitiveTopology::PointList
            | PrimitiveTopology::LineList
            | PrimitiveTopology::LineStrip => {}
        }
    }
}

impl<'a> MeshPart<'a> {
    /// Gets the part's interleaved vertex components.
    pub fn vertices(&self) -> &Vec<f32> {
        &self.primitive.vertices
    }

    /// Gets the indices referencing the part's vertices.
    pub fn indices(&self) -> &Vec<u32> {
        &self.primitive.indices
    }

    /// Gets the part's material properties.
    pub fn material(&self) -> &MaterialProperties {
        &self.mesh.materials[self.primitive.material_index]
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

    fn texture_image_data(&self, texture: &Option<MeshTexture>) -> Option<&[u8]> {
        if let Some(texture) = texture {
            let data = self.mesh.texture_images[texture.image_index].as_slice();
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

impl MeshTexture {
    fn from_gltf_texture(texture: &gltf::Texture<'_>, uv_set: u32) -> Self {
        Self {
            image_index: texture.source().index(),
            uv_set: uv_set as usize,
            sampler_index: texture.sampler().index().map_or(0, |i| i + 1),
        }
    }
}

mod error {
    use crate::Error;

    impl From<gltf::Error> for Error {
        fn from(value: gltf::Error) -> Self {
            Self::fail(value.to_string())
        }
    }
}

mod primitive_topology {
    use super::*;

    impl TryFrom<gltf::mesh::Mode> for PrimitiveTopology {
        type Error = crate::error::Error;

        fn try_from(value: gltf::mesh::Mode) -> Result<Self> {
            match value {
                gltf::mesh::Mode::Points => Ok(PrimitiveTopology::PointList),
                gltf::mesh::Mode::Lines => Ok(PrimitiveTopology::LineList),
                gltf::mesh::Mode::LineStrip => Ok(PrimitiveTopology::LineStrip),
                gltf::mesh::Mode::Triangles => Ok(PrimitiveTopology::TriangleList),
                gltf::mesh::Mode::TriangleStrip => Ok(PrimitiveTopology::TriangleStrip),
                _ => Err(Error::fail("Unsupported mesh primitive topology.")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_file() {
        let vertex_attributes = [
            MeshVertexAttribute::Position,
            MeshVertexAttribute::Normal,
            MeshVertexAttribute::TextureCoordinates(0),
        ];
        {
            const FILE_PATH: &str = "assets/treasure_chest/treasure_chest_4k.gltf";
            assert!(Mesh::from_file(FILE_PATH, &vertex_attributes).is_ok());
        }

        {
            const FILE_PATH: &str = "assets/treasure_chest/treasure_chest_diff_4k.jpg";
            assert!(matches!(
                Mesh::from_file(FILE_PATH, &vertex_attributes),
                Err(Error::Io(_))
            ));
        }
    }

    #[test]
    fn test_from_file_interleaves_vertex_attributes() {
        const FILE_PATH: &str = "assets/treasure_chest/treasure_chest_4k.gltf";
        let positions = Mesh::from_file(FILE_PATH, &[MeshVertexAttribute::Position]).unwrap();
        let normals = Mesh::from_file(FILE_PATH, &[MeshVertexAttribute::Normal]).unwrap();
        let tex_coords =
            Mesh::from_file(FILE_PATH, &[MeshVertexAttribute::TextureCoordinates(0)]).unwrap();
        let mesh = Mesh::from_file(
            FILE_PATH,
            &[
                MeshVertexAttribute::TextureCoordinates(0),
                MeshVertexAttribute::Position,
                MeshVertexAttribute::Normal,
            ],
        )
        .unwrap();

        assert_eq!(mesh.vertex_component_count(), 8);
        assert_eq!(mesh.vertex_size_bytes(), 8 * std::mem::size_of::<f32>());
        assert_eq!(mesh.primitives.len(), positions.primitives.len());
        for (i, primitive) in mesh.primitives.iter().enumerate() {
            let positions = &positions.primitives[i].vertices;
            let normals = &normals.primitives[i].vertices;
            let tex_coords = &tex_coords.primitives[i].vertices;
            assert_eq!(primitive.vertices.len(), positions.len() / 3 * 8);
            for (j, vertex) in primitive.vertices.as_chunks::<8>().0.iter().enumerate() {
                assert_eq!(&vertex[..2], &tex_coords[j * 2..j * 2 + 2]);
                assert_eq!(&vertex[2..5], &positions[j * 3..j * 3 + 3]);
                assert_eq!(&vertex[5..], &normals[j * 3..j * 3 + 3]);
            }
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
            Mesh::swap_triangle_winding(topology, &mut indices, 1);
            assert_eq!(indices, vec![0, 1, 2, 3, 4, 5]);
        }

        {
            let mut indices = vec![0u32, 1, 2, 3, 4, 5];
            Mesh::swap_triangle_winding(PrimitiveTopology::TriangleList, &mut indices, 1);
            assert_eq!(indices, vec![0, 2, 1, 3, 5, 4]);
        }

        // Decode the strip to check that every original triangle has reversed winding.
        // Ignore degenerate triangles and normalize cyclic rotations for comparison.
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
            Mesh::swap_triangle_winding(PrimitiveTopology::TriangleStrip, &mut indices, 1);
            assert_eq!(strip_triangles(&indices), expected);
            if count < 3 {
                assert_eq!(indices, original);
            }
        }

        // Non-indexed winding must preserve the complete interleaved vertex records.
        let original: Vec<f32> = (0..48).map(|value| value as f32).collect();
        let mut vertices = original.clone();
        Mesh::swap_triangle_winding(PrimitiveTopology::TriangleList, &mut vertices, 8);
        let expected: Vec<f32> = [0, 2, 1, 3, 5, 4]
            .into_iter()
            .flat_map(|i| original[i * 8..i * 8 + 8].iter().copied())
            .collect();
        assert_eq!(vertices, expected);

        let mut vertices = original.clone();
        Mesh::swap_triangle_winding(PrimitiveTopology::TriangleStrip, &mut vertices, 8);
        assert_eq!(&vertices[..8], &original[..8]);
        assert_eq!(&vertices[8..], original.as_slice());
        for count in 0..3 {
            let mut vertices = original[..count * 8].to_vec();
            Mesh::swap_triangle_winding(PrimitiveTopology::TriangleStrip, &mut vertices, 8);
            assert_eq!(vertices, original[..count * 8]);
        }
    }

    #[test]
    fn test_primitive_topology_try_from() {
        assert!(matches!(
            PrimitiveTopology::try_from(gltf::mesh::Mode::Points),
            Ok(PrimitiveTopology::PointList)
        ));
        assert!(matches!(
            PrimitiveTopology::try_from(gltf::mesh::Mode::Lines),
            Ok(PrimitiveTopology::LineList)
        ));
        assert!(matches!(
            PrimitiveTopology::try_from(gltf::mesh::Mode::LineStrip),
            Ok(PrimitiveTopology::LineStrip)
        ));
        assert!(matches!(
            PrimitiveTopology::try_from(gltf::mesh::Mode::Triangles),
            Ok(PrimitiveTopology::TriangleList)
        ));
        assert!(matches!(
            PrimitiveTopology::try_from(gltf::mesh::Mode::TriangleStrip),
            Ok(PrimitiveTopology::TriangleStrip)
        ));
        assert!(matches!(
            PrimitiveTopology::try_from(gltf::mesh::Mode::LineLoop),
            Err(Error::Fail(_))
        ));
        assert!(matches!(
            PrimitiveTopology::try_from(gltf::mesh::Mode::TriangleFan),
            Err(Error::Fail(_))
        ));
    }
}
