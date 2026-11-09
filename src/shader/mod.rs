mod binding;
mod error;
mod metadata;

use std::path::Path;

use spirq::{
    ReflectConfig,
    constant::ConstantValue,
    spirv::{ExecutionMode, ExecutionModel},
    ty::{DescriptorType, ScalarType, Type},
    var::Variable,
};

pub(crate) use crate::shader::{binding::*, metadata::*};
use crate::{Error, Result};

/// Represents a loaded shader resource containing compiled bytecode.
pub struct Shader {
    /// The file path of the shader.
    pub file_path: String,
    /// The compiled bytecode.
    pub(crate) data: Vec<u8>,
    /// The metadata extracted from the shader.
    pub(crate) metadata: ShaderMetadata,
}

impl Shader {
    /// Loads the shader from a file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut shader = Self::from_bytes(std::fs::read(path.as_ref())?)?;
        shader.file_path = path.as_ref().to_string_lossy().into_owned();
        Ok(shader)
    }

    /// Loads the shader from raw bytes.
    pub fn from_bytes<T>(data: T) -> Result<Self>
    where
        T: Into<Vec<u8>>,
    {
        let mut shader = Self {
            file_path: String::default(),
            data: data.into(),
            metadata: ShaderMetadata {
                entry_name: String::default(),
                stage: ShaderStage::Vertex,
                size: [0, 0, 0],
                input_primitive: None,
                descriptor_bindings: Vec::default(),
                vertex_bindings: Vec::default(),
            },
        };
        shader.verify_shader_language()?;
        shader.extract_metadata()?;
        Ok(shader)
    }

    /// Calculates the number of descriptor resource groups.
    pub fn descriptor_group_count(&self) -> usize {
        let mut max_group = -1;
        for binding in &self.metadata.descriptor_bindings {
            max_group = max_group.max(binding.group as i32);
        }
        (max_group + 1) as usize
    }

    fn extract_metadata(&mut self) -> Result<()> {
        let entry_points = ReflectConfig::new()
            .spv(self.data.as_slice())
            .ref_all_rscs(true)
            .reflect()?;
        if entry_points.len() != 1 || entry_points[0].name != "main" {
            return Err(Error::fail(
                "Shader must contain exactly one entry point named main.",
            ));
        }

        let entry_point = &entry_points[0];
        self.metadata.entry_name = entry_point.name.clone();
        self.metadata.stage = match entry_point.exec_model {
            ExecutionModel::Vertex => ShaderStage::Vertex,
            ExecutionModel::Geometry => ShaderStage::Geometry,
            ExecutionModel::Fragment => ShaderStage::Fragment,
            ExecutionModel::GLCompute => ShaderStage::Compute,
            _ => return Err(Error::fail("Unsupported shader stage.")),
        };
        self.metadata.size = [0, 0, 0];
        self.metadata.input_primitive = None;
        self.metadata.vertex_bindings.clear();
        self.metadata.descriptor_bindings.clear();

        for exec_mode in &entry_point.exec_modes {
            match exec_mode.exec_mode {
                ExecutionMode::LocalSize => {
                    for (size, operand) in self.metadata.size.iter_mut().zip(&exec_mode.operands) {
                        let ConstantValue::U32(value) = &operand.value else {
                            return Err(Error::fail("Invalid workgroup size."));
                        };
                        *size = *value;
                    }
                }
                ExecutionMode::InputPoints => {
                    self.metadata.input_primitive = Some(ShaderPrimitive::Point);
                }
                ExecutionMode::InputLines => {
                    self.metadata.input_primitive = Some(ShaderPrimitive::Line);
                }
                ExecutionMode::Triangles => {
                    self.metadata.input_primitive = Some(ShaderPrimitive::Triangle);
                }
                ExecutionMode::InputLinesAdjacency | ExecutionMode::InputTrianglesAdjacency => {
                    return Err(Error::fail("Adjacency primitives are not supported."));
                }
                _ => {}
            }
        }
        if self.metadata.stage == ShaderStage::Geometry && self.metadata.input_primitive.is_none() {
            return Err(Error::fail(
                "Geometry shader must declare an input primitive.",
            ));
        }

        for var in &entry_point.vars {
            match var {
                Variable::Input {
                    location,
                    ty: input_type,
                    ..
                } if self.metadata.stage == ShaderStage::Vertex => {
                    let (scalar_type, components) = match input_type {
                        Type::Scalar(scalar_type) => (scalar_type, 1),
                        Type::Vector(vector_type) => (&vector_type.scalar_ty, vector_type.nscalar),
                        _ => return Err(Error::fail("Unsupported vertex input type.")),
                    };

                    let (scalar_kind, scalar_width) = match scalar_type {
                        ScalarType::Float { bits } => (ScalarKind::Float, (bits / 8) as u8),
                        _ => return Err(Error::fail("Unsupported scalar kind.")),
                    };

                    self.metadata.vertex_bindings.push(ShaderVertexBinding {
                        location: location.loc(),
                        scalar_kind,
                        scalar_width,
                        components,
                    });
                }
                Variable::Descriptor {
                    desc_bind,
                    desc_ty: descriptor_type,
                    ..
                } => {
                    let binding_type = match descriptor_type {
                        DescriptorType::UniformBuffer() => ShaderBindingType::UniformBuffer,
                        DescriptorType::StorageBuffer(_) => ShaderBindingType::StorageBuffer,
                        DescriptorType::SampledImage() => ShaderBindingType::Texture,
                        DescriptorType::Sampler() => ShaderBindingType::Sampler,
                        _ => continue,
                    };

                    self.metadata
                        .descriptor_bindings
                        .push(ShaderDescriptorBinding {
                            group: desc_bind.set(),
                            binding: desc_bind.bind(),
                            binding_type,
                        });
                }
                _ => {}
            }
        }

        self.metadata.vertex_bindings.sort_by_key(|b| b.location);
        self.metadata
            .descriptor_bindings
            .sort_by_key(|b| (b.group, b.binding));

        Ok(())
    }

    /// Checks whether the entered shader is written in a supported language.
    fn verify_shader_language(&self) -> Result<()> {
        const SPIRV_MAGIC: u32 = 0x0723_0203;
        if self.data.len() < 4 {
            return Err(Error::UnsupportedShaderLanguage);
        }
        let magic = u32::from_le_bytes(self.data[..4].try_into().unwrap());
        if magic != SPIRV_MAGIC {
            Err(Error::UnsupportedShaderLanguage)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_file() {
        {
            const SHADER_PATH: &str = "shaders/addition.spv";
            let shader = Shader::from_file(SHADER_PATH).unwrap();
            assert_eq!(shader.file_path, SHADER_PATH);
            assert!(!shader.data.is_empty());
        }

        {
            assert!(matches!(
                Shader::from_file("shaders/addition.comp"),
                Err(Error::UnsupportedShaderLanguage)
            ));
        }

        {
            assert!(matches!(Shader::from_file(""), Err(Error::Io(_))));
        }
    }

    #[test]
    fn test_from_bytes() {
        const SHADER_DATA: &[u8] = include_bytes!("../../shaders/addition.spv");
        let shader = Shader::from_bytes(SHADER_DATA).unwrap();
        assert!(shader.file_path.is_empty());
        assert!(!shader.data.is_empty());
    }
}
