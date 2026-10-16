use std::{
    fs::File,
    io::{BufReader, Read, Seek},
    path::Path,
};

use crate::{
    Error, Result,
    math::{Mat3, Mat4, Vec3},
};

/// Represents a mesh vertex containing a 3D position, a 3D normal, and a 2D texture coordinates, in
/// that order.
pub type MeshVertex = [f32; 8];

/// Defines the supported file types for loading meshes.
enum MeshFileType {
    Gltf,
}

/// One part of a 3D shape, described by points and references that connect them.
// TODO: We use too many vectors here. Refactor this to have begin/end iterators, so we can store
// all indices and vertices in two vectors.
#[derive(Default, Clone)]
struct Primitive {
    vertices: Vec<MeshVertex>,
    indices: Vec<u32>,
}

/// Represents a 3D shape assembled from parts, with each part describing points on its surface and
/// how they connect.
#[derive(Default, Clone)]
pub struct Mesh {
    primitives: Vec<Primitive>,
}

impl Mesh {
    /// Gets the vertices of each primitive.
    pub fn vertices(&self) -> Vec<&Vec<MeshVertex>> {
        self.primitives.iter().map(|p| &p.vertices).collect()
    }

    /// Gets the flattened vertices.
    pub fn flattened_vertices(&self) -> Vec<MeshVertex> {
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
            vertex_offset += primitive.vertices.len() as u32;
        }
        flattened_indices
    }

    /// Loads the mesh from a file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::io("Mesh file not found."));
        }

        match Self::get_file_type(path)? {
            MeshFileType::Gltf => Self::from_gltf_file(path),
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

    fn from_gltf_file(path: &Path) -> Result<Self> {
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

            let gltf_primitives = gltf_mesh.primitives();
            mesh.primitives.reserve(gltf_primitives.len());
            for gltf_primitive in gltf_primitives {
                let mut primitive = Primitive::default();

                let reader =
                    gltf_primitive.reader(|buffer| Some(buffers[buffer.index()].as_slice()));
                let positions = reader
                    .read_positions()
                    .ok_or(Error::fail("Failed to read the mesh primitive positions."))?;
                let normals = reader
                    .read_normals()
                    .ok_or(Error::fail("Failed to read the mesh primitive normals."))?;
                let tex_coords = reader
                    .read_tex_coords(0)
                    .ok_or(Error::io(
                        "Failed to read the primitive texture coordinates.",
                    ))?
                    .into_f32();

                primitive.vertices.reserve(positions.len());
                for ((position, normal), uv) in positions.zip(normals).zip(tex_coords) {
                    // Bake node transforms so the lid, body, handles, and lock keep their
                    // placement.
                    let position = transform.transform_point3(Vec3::from_array(position));
                    let normal = (normal_transform * Vec3::from_array(normal)).normalize();
                    primitive.vertices.push([
                        position.x, position.y, position.z, normal.x, normal.y, normal.z, uv[0],
                        uv[1],
                    ]);
                }

                if let Some(indices) = reader.read_indices() {
                    primitive.indices = indices.into_u32().collect();
                    // A reflection reverses triangle winding; swap two indices to preserve front
                    // faces.
                    if transform.determinant() < 0.0 {
                        for triangle in primitive.indices.as_chunks_mut::<3>().0 {
                            triangle.swap(1, 2);
                        }
                    }
                }

                mesh.primitives.push(primitive);
            }
        }

        Ok(mesh)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_file() {
        {
            const FILE_PATH: &str = "assets/treasure_chest/treasure_chest_4k.gltf";
            assert!(Mesh::from_file(FILE_PATH).is_ok());
        }

        {
            const FILE_PATH: &str = "assets/treasure_chest/treasure_chest_diff_4k.jpg";
            assert!(matches!(Mesh::from_file(FILE_PATH), Err(Error::Io(_))));
        }
    }

    #[test]
    fn test_flattened_indices_rebase_each_primitive() {
        let mesh = Mesh {
            primitives: vec![
                Primitive {
                    vertices: vec![[0.0; 8]; 3],
                    indices: vec![0, 2],
                },
                Primitive {
                    vertices: vec![[0.0; 8]; 4],
                    indices: vec![1, 3],
                },
            ],
        };
        assert_eq!(mesh.flattened_indices(), vec![0, 2, 4, 6]);
    }
}
