use std::any::Any;

use crate::{Result, renderers::debug::RendererDebugObject};

/// Writes debug information to the underlying graphics API.
pub(in crate::renderers) trait DebugWriter: Any + Send + Sync {
    fn set_object_name(&self, object: &dyn RendererDebugObject, name: &str) -> Result<()>;
}
