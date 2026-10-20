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
    mesh::{Mesh, MeshPrimitive, MeshSampler, texture::MeshTexture},
    renderers::settings::{PrimitiveTopology, SamplerAddressMode, SamplerFilter},
};

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

pub(super) fn is_gltf_file(path: &Path) -> Result<bool> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;

    if &magic == b"glTF" {
        return Ok(true);
    }

    reader.rewind()?;
    Ok(gltf::json::deserialize::from_reader::<_, gltf::json::Root>(reader).is_ok())
}

pub(super) fn from_file(path: &Path) -> Result<Mesh> {
    let mut mesh = Mesh::default();

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
                gltf::texture::WrappingMode::MirroredRepeat => SamplerAddressMode::MirroredRepeat,
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
        match image.source() {
            gltf::image::Source::View { view, .. } => {
                let buffer = &buffers[view.buffer().index()];
                mesh.texture_images.push((
                    String::default(),
                    buffer[view.offset()..view.offset() + view.length()].to_vec(),
                ));
            }
            gltf::image::Source::Uri { uri, .. } => {
                // Push an empty vector to keep the indices of primitives valid.
                mesh.texture_images.push((uri.to_owned(), Vec::default()));
            }
        };
    }

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
            let mut primitive = MeshPrimitive {
                topology: gltf_primitive.mode().try_into()?,
                vertex_count: gltf_primitive
                    .attributes()
                    .next()
                    .map_or(0, |(_, accessor)| accessor.count()),
                ..Default::default()
            };
            let swap_vertices = swap_triangle_winding && gltf_primitive.indices().is_none();
            let reader = gltf_primitive.reader(|buffer| Some(buffers[buffer.index()].as_slice()));
            for (semantic, _) in gltf_primitive.attributes() {
                match semantic {
                    gltf::Semantic::Positions => {
                        let positions = reader
                            .read_positions()
                            .ok_or(Error::fail("Failed to read the mesh primitive positions."))?
                            .map(|p| transform.transform_point3(Vec3::from(p)).to_array());
                        primitive.positions_range = Some(mesh.append_vertex_attribute(
                            positions,
                            primitive.vertex_count,
                            primitive.topology,
                            swap_vertices,
                        )?);
                    }
                    gltf::Semantic::Normals => {
                        let normals = reader
                            .read_normals()
                            .ok_or(Error::fail("Failed to read the mesh primitive normals."))?
                            .map(|n| {
                                (normal_transform * Vec3::from_array(n))
                                    .normalize()
                                    .to_array()
                            });
                        primitive.normals_range = Some(mesh.append_vertex_attribute(
                            normals,
                            primitive.vertex_count,
                            primitive.topology,
                            swap_vertices,
                        )?);
                    }
                    gltf::Semantic::Tangents => {
                        let tangents = reader
                            .read_tangents()
                            .ok_or(Error::fail("Failed to read the mesh primitive tangents."))?
                            .map(|t| {
                                let tangent = transform
                                    .transform_vector3(Vec3::new(t[0], t[1], t[2]))
                                    .normalize();
                                let handedness = match swap_triangle_winding {
                                    true => -t[3],
                                    false => t[3],
                                };
                                [tangent.x, tangent.y, tangent.z, handedness]
                            });
                        primitive.tangents_range = Some(mesh.append_vertex_attribute(
                            tangents,
                            primitive.vertex_count,
                            primitive.topology,
                            swap_vertices,
                        )?);
                    }
                    gltf::Semantic::TexCoords(set) => {
                        let tex_coords = reader
                            .read_tex_coords(set)
                            .ok_or(Error::fail(
                                "Failed to read the mesh primitive texture coordinates.",
                            ))?
                            .into_f32();
                        let range = mesh.append_vertex_attribute(
                            tex_coords,
                            primitive.vertex_count,
                            primitive.topology,
                            swap_vertices,
                        )?;
                        primitive.tex_coords_ranges.push((set, range));
                    }
                    gltf::Semantic::Colors(set) => {
                        let colors = reader
                            .read_colors(set)
                            .ok_or(Error::fail("Failed to read the mesh primitive colors."))?
                            .into_rgba_f32();
                        let range = mesh.append_vertex_attribute(
                            colors,
                            primitive.vertex_count,
                            primitive.topology,
                            swap_vertices,
                        )?;
                        primitive.colors_ranges.push((set, range));
                    }
                    _ => {}
                }
            }
            primitive
                .tex_coords_ranges
                .sort_unstable_by_key(|(set, _)| *set);

            if let Some(indices) = reader.read_indices() {
                let ib = mesh.indices.len();
                mesh.indices.extend(indices.into_u32());
                if swap_triangle_winding {
                    Mesh::swap_triangle_winding(primitive.topology, &mut mesh.indices, ib, 1);
                }
                primitive.indices_range = (ib, mesh.indices.len());
            } else if swap_vertices
                && matches!(primitive.topology, PrimitiveTopology::TriangleStrip)
                && primitive.vertex_count >= 3
            {
                primitive.vertex_count += 1;
            }

            let material = gltf_primitive.material();
            primitive.material_index = material.index().map_or(0, |i| i + 1);
            let pbr = material.pbr_metallic_roughness();
            primitive.base_color_texture = pbr
                .base_color_texture()
                .map(|i| read_texture(&i.texture(), i.tex_coord()));
            primitive.arm_texture = pbr
                .metallic_roughness_texture()
                .map(|i| read_texture(&i.texture(), i.tex_coord()));
            primitive.normal_texture = material
                .normal_texture()
                .map(|i| read_texture(&i.texture(), i.tex_coord()));
            primitive.emissive_texture = material
                .emissive_texture()
                .map(|i| read_texture(&i.texture(), i.tex_coord()));
            primitive.occlusion_texture = material
                .occlusion_texture()
                .map(|i| read_texture(&i.texture(), i.tex_coord()));

            mesh.primitives.push(primitive);
        }
    }

    Ok(mesh)
}

fn read_texture(texture: &gltf::Texture<'_>, uv_set: u32) -> MeshTexture {
    MeshTexture {
        image_index: texture.source().index(),
        uv_set: uv_set as usize,
        sampler_index: texture.sampler().index().map_or(0, |i| i + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::MeshVertexAttribute;

    #[test]
    fn test_from_file_attribute_blocks() {
        // The source is interleaved and has two UV sets and RGB colors.
        let source: [[f32; 17]; 3] = [
            [
                0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.1, 0.2, 1.0, 0.0, 0.0,
            ],
            [
                1.0, 0.0, 2.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.3, 0.4, 0.0, 1.0, 0.0,
            ],
            [
                0.0, 1.0, 3.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.5, 0.6, 0.0, 0.0, 1.0,
            ],
        ];
        let mut buffer: Vec<u8> = source
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        buffer.extend([0u32, 1, 2].into_iter().flat_map(u32::to_le_bytes));
        let json = br#"{
            "asset": {"version": "2.0"},
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [{"mesh": 0}],
            "buffers": [{"byteLength": 216}],
            "bufferViews": [
                {"buffer": 0, "byteLength": 204, "byteStride": 68},
                {"buffer": 0, "byteOffset": 204, "byteLength": 12}
            ],
            "accessors": [
                {"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0,0,1], "max": [1,1,3]},
                {"bufferView": 0, "byteOffset": 12, "componentType": 5126, "count": 3, "type": "VEC3"},
                {"bufferView": 0, "byteOffset": 24, "componentType": 5126, "count": 3, "type": "VEC4"},
                {"bufferView": 0, "byteOffset": 40, "componentType": 5126, "count": 3, "type": "VEC2"},
                {"bufferView": 0, "byteOffset": 48, "componentType": 5126, "count": 3, "type": "VEC2"},
                {"bufferView": 0, "byteOffset": 56, "componentType": 5126, "count": 3, "type": "VEC3"},
                {"bufferView": 1, "componentType": 5125, "count": 3, "type": "SCALAR"}
            ],
            "meshes": [{"primitives": [
                {"attributes": {"POSITION": 0, "NORMAL": 1, "TANGENT": 2, "TEXCOORD_1": 4, "TEXCOORD_0": 3, "COLOR_0": 5, "COLOR_1": 1}, "indices": 6, "mode": 5},
                {"attributes": {"POSITION": 0, "NORMAL": 1, "TANGENT": 2, "TEXCOORD_1": 4, "TEXCOORD_0": 3, "COLOR_0": 5, "COLOR_1": 1}, "mode": 5},
                {"attributes": {"POSITION": 0, "NORMAL": 1, "TANGENT": 2, "TEXCOORD_1": 4, "TEXCOORD_0": 3, "COLOR_0": 5, "COLOR_1": 1}, "mode": 4},
                {"attributes": {"POSITION": 0}, "mode": 4}
            ]}]
        }"#;
        let glb = gltf::binary::Glb {
            header: gltf::binary::Header {
                magic: *b"glTF",
                version: 2,
                length: 0,
            },
            json: std::borrow::Cow::Borrowed(json),
            bin: Some(std::borrow::Cow::Owned(buffer)),
        };
        let path = std::env::temp_dir().join(format!(
            "heph_mesh_attributes_{}_{}.glb",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        glb.to_writer(File::create(&path).unwrap()).unwrap();
        let result = Mesh::from_file(&path);
        std::fs::remove_file(&path).unwrap();
        let mesh = result.unwrap();
        let attributes = [
            MeshVertexAttribute::Color(0),
            MeshVertexAttribute::TextureCoordinates(0),
            MeshVertexAttribute::TextureCoordinates(1),
            MeshVertexAttribute::Tangent,
            MeshVertexAttribute::Normal,
            MeshVertexAttribute::Position,
        ];
        assert_eq!(mesh.part_count(), 4);
        let orders: [&[usize]; 3] = [&[0, 1, 2], &[0, 0, 1, 2], &[0, 2, 1]];
        for (i, part) in mesh.parts().take(3).enumerate() {
            assert_eq!(part.vertex_component_count(&attributes), 18);
            let colors = [MeshVertexAttribute::Color(1), MeshVertexAttribute::Color(0)];
            assert_eq!(part.vertex_component_count(&colors), 8);
            assert_eq!(part.vertex_size_bytes(&colors), 32);
            let colors = part.vertices(&colors).unwrap();
            assert_eq!(colors.len(), orders[i].len() * 8);
            for (vertex, &source_index) in colors.as_chunks::<8>().0.iter().zip(orders[i]) {
                assert_eq!(&vertex[..4], &[0.0, 0.0, 1.0, 1.0]);
                assert_eq!(&vertex[4..7], &source[source_index][14..17]);
                assert_eq!(vertex[7], 1.0);
            }
            assert!(part.vertices(&[MeshVertexAttribute::Color(2)]).is_err());
            let vertices = part.vertices(&attributes).unwrap();
            assert_eq!(vertices.len(), orders[i].len() * 18);
            for (vertex, &source_index) in vertices.as_chunks::<18>().0.iter().zip(orders[i]) {
                let original = source[source_index];
                assert_eq!(
                    &vertex[..4],
                    &[original[14], original[15], original[16], 1.0]
                );
                assert_eq!(&vertex[4..8], &original[10..14]);
                assert_eq!(&vertex[8..12], &[1.0, 0.0, 0.0, -1.0]);
                assert_eq!(&vertex[12..15], &[0.0, 0.0, -1.0]);
                assert_eq!(&vertex[15..], &[original[0], original[1], -original[2]]);
            }
            match i {
                0 => assert_eq!(part.indices(), &[0, 0, 1, 2]),
                _ => assert!(part.indices().is_empty()),
            }
        }
        let part = mesh.parts().last().unwrap();
        assert_eq!(
            part.vertices(&[MeshVertexAttribute::Position])
                .unwrap()
                .len(),
            9
        );
        assert!(part.vertices(&[MeshVertexAttribute::Normal]).is_err());
        assert!(
            part.vertices(&[MeshVertexAttribute::TextureCoordinates(0)])
                .is_err()
        );
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
