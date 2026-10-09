use std::path::Path;

use naga::ShaderStage;

#[derive(Clone, Copy)]
pub(crate) enum ShaderBindingType {
    UniformBuffer,
    StorageBuffer,
    Texture,
    Sampler,
}

pub(crate) struct ShaderVertexBinding {
    pub location: u32,
    pub scalar_kind: naga::ScalarKind,
    pub scalar_width: u8,
    pub components: u32,
}

pub(crate) struct ShaderDescriptorBinding {
    pub group: u32,
    pub binding: u32,
    pub binding_type: ShaderBindingType,
}

pub(crate) struct ShaderMetadata {
    pub entry_name: String,
    pub stage: ShaderStage,
    pub size: [u32; 3],
    pub vertex_bindings: Vec<ShaderVertexBinding>,
    pub descriptor_bindings: Vec<ShaderDescriptorBinding>,
}

/// Represents a loaded shader resource containing compiled bytecode.
pub struct Shader {
    /// The file path of the shader.
    pub file_path: String,
    /// The compiled bytecode.
    pub(crate) data: Vec<u8>,
    /// The metadata extracted from the shader.
    pub(crate) metadata: ShaderMetadata,
}

#[derive(Debug)]
pub enum ShaderError {
    UnsupportedShaderLanguage,
    Fail(String),
}

impl Shader {
    /// Loads the shader from a file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ShaderError> {
        let data = std::fs::read(path.as_ref())?;
        let mut shader = Self {
            file_path: path.as_ref().to_string_lossy().into_owned(),
            data,
            metadata: ShaderMetadata {
                entry_name: String::default(),
                stage: ShaderStage::Vertex,
                size: [0, 0, 0],
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

    fn extract_metadata(&mut self) -> Result<(), ShaderError> {
        let module =
            naga::front::spv::parse_u8_slice(&self.data, &naga::front::spv::Options::default())?;
        if module.entry_points.len() != 1 || module.entry_points[0].name != "main" {
            return Err(ShaderError::fail(
                "Shader must contain exactly one entry point named main.",
            ));
        }

        let entry_point = &module.entry_points[0];
        self.metadata.entry_name = entry_point.name.clone();
        self.metadata.stage = entry_point.stage;
        self.metadata.size = entry_point.workgroup_size;
        self.metadata.vertex_bindings.clear();
        self.metadata.descriptor_bindings.clear();

        if entry_point.stage == ShaderStage::Vertex {
            for argument in &entry_point.function.arguments {
                let Some(naga::Binding::Location { location, .. }) = &argument.binding else {
                    continue;
                };

                let (scalar_kind, scalar_width, components) = match &module.types[argument.ty].inner
                {
                    naga::TypeInner::Scalar(scalar) => (scalar.kind, scalar.width, 1),
                    naga::TypeInner::Vector { size, scalar } => {
                        let components = match size {
                            naga::VectorSize::Bi => 2,
                            naga::VectorSize::Tri => 3,
                            naga::VectorSize::Quad => 4,
                        };

                        (scalar.kind, scalar.width, components)
                    }
                    _ => {
                        return Err(ShaderError::fail("Unsupported vertex input type."));
                    }
                };

                self.metadata.vertex_bindings.push(ShaderVertexBinding {
                    location: *location,
                    scalar_kind,
                    scalar_width,
                    components,
                });
            }

            self.metadata
                .vertex_bindings
                .sort_by_key(|binding| binding.location);
        }

        for (_, global) in module.global_variables.iter() {
            let Some(binding) = &global.binding else {
                continue;
            };

            let binding_type = match global.space {
                naga::AddressSpace::Uniform => ShaderBindingType::UniformBuffer,
                naga::AddressSpace::Storage { access: _ } => ShaderBindingType::StorageBuffer,
                naga::AddressSpace::Handle => match &module.types[global.ty].inner {
                    naga::TypeInner::Image {
                        class: naga::ImageClass::Sampled { .. } | naga::ImageClass::Depth { .. },
                        ..
                    } => ShaderBindingType::Texture,
                    naga::TypeInner::Sampler { .. } => ShaderBindingType::Sampler,
                    _ => continue,
                },
                _ => continue,
            };

            self.metadata
                .descriptor_bindings
                .push(ShaderDescriptorBinding {
                    group: binding.group,
                    binding: binding.binding,
                    binding_type,
                });
        }

        self.metadata.vertex_bindings.sort_by_key(|b| b.location);
        self.metadata
            .descriptor_bindings
            .sort_by_key(|b| (b.group, b.binding));

        Ok(())
    }

    /// Checks whether the entered shader is written in a supported language.
    fn verify_shader_language(&self) -> Result<(), ShaderError> {
        const SPIRV_MAGIC: u32 = 0x0723_0203;
        if self.data.len() < 4 {
            return Err(ShaderError::UnsupportedShaderLanguage);
        }
        let magic = u32::from_le_bytes(self.data[..4].try_into().unwrap());
        if magic != SPIRV_MAGIC {
            Err(ShaderError::UnsupportedShaderLanguage)
        } else {
            Ok(())
        }
    }
}

impl From<std::io::Error> for ShaderError {
    fn from(value: std::io::Error) -> Self {
        Self::Fail(value.to_string())
    }
}
impl From<naga::front::spv::Error> for ShaderError {
    fn from(value: naga::front::spv::Error) -> Self {
        Self::Fail(value.to_string())
    }
}
impl ShaderError {
    fn fail(msg: impl Into<String>) -> Self {
        Self::Fail(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_file() {
        {
            const SHADER_PATH: &str = "shaders/addition.spv";
            let result = Shader::from_file(SHADER_PATH);
            assert!(result.is_ok());
            let shader = result.unwrap();
            assert_eq!(shader.file_path, SHADER_PATH);
            assert!(!shader.data.is_empty());
        }

        {
            assert!(Shader::from_file("shaders/addition.comp").is_err());
        }

        {
            assert!(Shader::from_file("").is_err());
        }
    }
}
