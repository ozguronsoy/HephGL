use std::path::Path;

use naga::ShaderStage;

#[derive(Clone, Copy)]
pub(crate) enum ShaderBindingType {
    UniformBuffer,
    StorageBuffer,
}

pub(crate) struct ShaderBinding {
    pub group: u32,
    pub binding: u32,
    pub binding_type: ShaderBindingType,
}

pub(crate) struct ShaderMetadata {
    pub entry_name: String,
    pub stage: ShaderStage,
    pub size: [u32; 3],
    pub bindings: Vec<ShaderBinding>,
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
                bindings: Vec::default(),
            },
        };
        shader.verify_shader_language()?;
        shader.extract_metadata()?;
        Ok(shader)
    }

    /// Calculates the number of resource groups.
    pub fn group_count(&self) -> u32 {
        let mut max_group = -1;
        for binding in &self.metadata.bindings {
            max_group = max_group.max(binding.group as i32);
        }
        (max_group + 1) as u32
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
        self.metadata.bindings.clear();
        for (_, global) in module.global_variables.iter() {
            let Some(binding) = &global.binding else {
                continue;
            };

            let binding_type = match global.space {
                naga::AddressSpace::Uniform => ShaderBindingType::UniformBuffer,
                naga::AddressSpace::Storage { access: _ } => ShaderBindingType::StorageBuffer,
                _ => continue,
            };

            self.metadata.bindings.push(ShaderBinding {
                group: binding.group,
                binding: binding.binding,
                binding_type,
            });
        }

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
