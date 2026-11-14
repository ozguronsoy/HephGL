use crate::renderers::debug::Severity;

/// Represents a message reported by the renderer's debug output, along with its severity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Log {
    pub(super) message: String,
    pub(super) severity: Severity,
}

impl Log {
    /// Gets the message of the log.
    pub fn message(&self) -> &'_ str {
        &self.message
    }

    /// Gets the severity of the log.
    pub fn severity(&self) -> Severity {
        self.severity
    }
}
