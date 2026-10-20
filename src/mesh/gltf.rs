use std::{
    fs::File,
    io::{BufReader, Read, Seek},
    path::Path,
};

use gltf::texture::MinFilter;
use renkrs::RGBA;

use crate::{
    Error, Result,
    animation::{Animation, AnimationChannel, AnimationInterpolation, AnimationProperty},
    material::MaterialProperties,
    math::{Mat3, Mat4, Quat, Vec3, Vec4},
    mesh::{Mesh, MeshPrimitive, MeshSampler, skin::MeshSkin, texture::MeshTexture},
    renderers::settings::{PrimitiveTopology, SamplerAddressMode, SamplerFilter},
    skeleton::{Joint, Skeleton},
    transform::Transform,
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
    let (skeleton, joint_indices) = read_skeleton(&gltf)?;
    mesh.skeleton = skeleton;
    mesh.skins = read_skins(&gltf, &buffers, &joint_indices)?;
    mesh.animations = read_animations(&gltf, &buffers, &joint_indices)?;

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
        // Skinned vertices use joint transforms, ignoring the transform of the mesh attachment.
        let transform = match node.skin() {
            Some(_) => coordinate_conversion,
            None => coordinate_conversion * node_transform,
        };
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
                joint_index: match node.skin().is_none() {
                    true => joint_indices[node.index()],
                    false => None,
                },
                skin_index: node.skin().map(|skin| skin.index()),
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
                    gltf::Semantic::Joints(set) => {
                        let joints = reader
                            .read_joints(set)
                            .ok_or(Error::fail("Failed to read mesh joint indices."))?
                            .into_u16()
                            .map(|indices| indices.map(f32::from));
                        let range = mesh.append_vertex_attribute(
                            joints,
                            primitive.vertex_count,
                            primitive.topology,
                            swap_vertices,
                        )?;
                        primitive.joint_indices_ranges.push((set, range));
                    }
                    gltf::Semantic::Weights(set) => {
                        let weights = reader
                            .read_weights(set)
                            .ok_or(Error::fail("Failed to read mesh joint weights."))?
                            .into_f32();
                        let range = mesh.append_vertex_attribute(
                            weights,
                            primitive.vertex_count,
                            primitive.topology,
                            swap_vertices,
                        )?;
                        primitive.joint_weights_ranges.push((set, range));
                    }
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
            normalize_joint_weights(&mut mesh, &primitive)?;

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

fn read_skeleton(gltf: &gltf::Gltf) -> Result<(Option<Skeleton>, Vec<Option<usize>>)> {
    let count = gltf.nodes().len();
    let mut joint_indices = vec![None; count];

    if gltf.skins().len() == 0 && gltf.animations().len() == 0 {
        return Ok((None, joint_indices));
    }

    let mut parents = vec![None; count];
    let mut children = vec![Vec::new(); count];
    for node in gltf.nodes() {
        for child in node.children() {
            if parents[child.index()].is_some() {
                return Err(Error::fail("glTF node has multiple parents."));
            }
            parents[child.index()] = Some(node.index());
            children[node.index()].push(child.index());
        }
    }

    let mut order: Vec<usize> = parents
        .iter()
        .enumerate()
        .filter_map(|(index, parent)| parent.is_none().then_some(index))
        .collect();
    let mut cursor = 0;
    while cursor < order.len() {
        order.extend_from_slice(&children[order[cursor]]);
        cursor += 1;
    }
    if order.len() != count {
        return Err(Error::fail("glTF node hierarchy contains a cycle."));
    }

    let mut animated = vec![false; count];
    let mut required = vec![false; count];
    for skin in gltf.skins() {
        for joint in skin.joints() {
            required[joint.index()] = true;
        }
    }
    for animation in gltf.animations() {
        for channel in animation.channels() {
            animated[channel.target().node().index()] = true;
            required[channel.target().node().index()] = true;
        }
    }
    for &index in &order {
        if let Some(parent) = parents[index] {
            animated[index] |= animated[parent];
        }
    }
    for node in gltf.nodes() {
        if node.mesh().is_some() && node.skin().is_none() && animated[node.index()] {
            required[node.index()] = true;
        }
    }

    // Preserve transform-only ancestors without retaining the glTF scene graph in Mesh.
    for &index in order.iter().rev() {
        if required[index]
            && let Some(parent) = parents[index]
        {
            required[parent] = true;
        }
    }

    let mut joint_count = 0;
    for (index, &required) in required.iter().enumerate() {
        if required {
            joint_indices[index] = Some(joint_count);
            joint_count += 1;
        }
    }

    let mut joints = Vec::with_capacity(joint_count);
    for node in gltf.nodes().filter(|node| required[node.index()]) {
        let (translation, rotation, scale) = node.transform().decomposed();
        let rotation = Quat::from_xyzw(-rotation[0], -rotation[1], rotation[2], rotation[3]);
        if rotation.length_squared() <= 0.0 {
            return Err(Error::fail("glTF joint rotation has zero length."));
        }
        joints.push(Joint {
            parent_index: parents[node.index()].and_then(|parent| joint_indices[parent]),
            transform: Transform::new(
                Vec3::new(translation[0], translation[1], -translation[2]),
                rotation.normalize(),
                Vec3::from_array(scale),
            ),
        });
    }

    Ok((Some(Skeleton::new(joints)?), joint_indices))
}

fn read_skins(
    gltf: &gltf::Gltf,
    buffers: &[Vec<u8>],
    joint_indices: &[Option<usize>],
) -> Result<Vec<MeshSkin>> {
    let conversion = Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0));
    gltf.skins()
        .map(|skin| {
            let joint_indices = skin
                .joints()
                .map(|joint| {
                    joint_indices[joint.index()]
                        .ok_or(Error::fail("glTF skin joint is missing from the skeleton."))
                })
                .collect::<Result<Vec<usize>>>()?;

            let inverse_bind_matrices = match skin
                .reader(|buffer| buffers.get(buffer.index()).map(Vec::as_slice))
                .read_inverse_bind_matrices()
            {
                Some(matrices) => matrices
                    .take(joint_indices.len())
                    .map(|matrix| conversion * Mat4::from_cols_array_2d(&matrix) * conversion)
                    .collect(),
                None => {
                    if skin.inverse_bind_matrices().is_some() {
                        return Err(Error::fail("Failed to read glTF inverse bind matrices."));
                    }
                    vec![Mat4::IDENTITY; joint_indices.len()]
                }
            };

            if inverse_bind_matrices.len() != joint_indices.len() {
                return Err(Error::fail(
                    "The number of inverse bind matrices does not match the number of joints.",
                ));
            }

            Ok(MeshSkin {
                joint_indices,
                inverse_bind_matrices,
            })
        })
        .collect()
}

fn read_animations(
    gltf: &gltf::Gltf,
    buffers: &[Vec<u8>],
    joint_indices: &[Option<usize>],
) -> Result<Vec<Animation>> {
    use gltf::animation::{Interpolation, util::ReadOutputs};

    gltf.animations()
        .map(|animation| {
            let channels = animation
                .channels()
                .map(|channel| {
                    let target = channel.target().node();
                    if matches!(target.transform(), gltf::scene::Transform::Matrix { .. }) {
                        return Err(Error::fail("glTF animation targets a matrix transform."));
                    }
                    let joint_index = joint_indices[target.index()].ok_or(Error::fail(
                        "glTF animation target is missing from the skeleton.",
                    ))?;
                    let reader =
                        channel.reader(|buffer| buffers.get(buffer.index()).map(Vec::as_slice));
                    let times = reader
                        .read_inputs()
                        .ok_or(Error::fail("Failed to read glTF animation times."))?
                        .collect();
                    let (property, values) = match reader
                        .read_outputs()
                        .ok_or(Error::fail("Failed to read glTF animation values."))?
                    {
                        ReadOutputs::Translations(values) => (
                            AnimationProperty::Translation,
                            values.map(|v| Vec4::new(v[0], v[1], -v[2], 0.0)).collect(),
                        ),
                        ReadOutputs::Rotations(values) => (
                            AnimationProperty::Rotation,
                            values
                                .into_f32()
                                .map(|v| Vec4::new(-v[0], -v[1], v[2], v[3]))
                                .collect(),
                        ),
                        ReadOutputs::Scales(values) => (
                            AnimationProperty::Scale,
                            values.map(|v| Vec3::from_array(v).extend(0.0)).collect(),
                        ),
                        ReadOutputs::MorphTargetWeights(_) => {
                            return Err(Error::fail("Morph target animations are not supported."));
                        }
                    };
                    let interpolation = match channel.sampler().interpolation() {
                        Interpolation::Step => AnimationInterpolation::Step,
                        Interpolation::Linear => AnimationInterpolation::Linear,
                        Interpolation::CubicSpline => AnimationInterpolation::CubicSpline,
                    };
                    Ok(AnimationChannel {
                        joint_index,
                        property,
                        interpolation,
                        times,
                        values,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Animation::new(channels)
        })
        .collect()
}

fn normalize_joint_weights(mesh: &mut Mesh, primitive: &MeshPrimitive) -> Result<()> {
    let Some(skin) = primitive.skin_index.map(|index| &mesh.skins[index]) else {
        return Ok(());
    };
    if primitive.joint_indices_ranges.is_empty()
        || primitive.joint_indices_ranges.len() != primitive.joint_weights_ranges.len()
    {
        return Err(Error::fail(
            "Skinned parts require matching joint index and weight sets.",
        ));
    }

    for (set, (begin, end)) in &primitive.joint_indices_ranges {
        if !primitive
            .joint_weights_ranges
            .iter()
            .any(|(weight_set, _)| weight_set == set)
            || mesh.vertices[*begin..*end]
                .iter()
                .any(|&index| index >= skin.joint_indices.len() as f32)
        {
            return Err(Error::fail(
                "Mesh joint indices do not match the skin or weight sets.",
            ));
        }
    }

    for vertex in 0..primitive.vertex_count {
        let mut total = 0.0;
        for (_, (begin, _)) in &primitive.joint_weights_ranges {
            for &weight in &mesh.vertices[begin + vertex * 4..begin + vertex * 4 + 4] {
                if weight < 0.0 {
                    return Err(Error::fail("Mesh joint weights must be nonnegative."));
                }
                total += weight;
            }
        }

        if total <= 0.0 {
            return Err(Error::fail(
                "Skinned vertices must have a positive total joint weight.",
            ));
        }

        for (_, (begin, _)) in &primitive.joint_weights_ranges {
            for weight in &mut mesh.vertices[begin + vertex * 4..begin + vertex * 4 + 4] {
                *weight /= total;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
