use std::any::Any;

use crate::{Result, renderers::debug::RendererDebugObject};

/// Writes debug information to the underlying graphics API.
pub(in super::super) trait DebugWriter: Any + Send + Sync {
    /// Sets the name of the object, which shows up in validation messages and graphics debuggers.
    fn set_object_name(&self, object: &dyn RendererDebugObject, name: &str) -> Result<()>;
}
