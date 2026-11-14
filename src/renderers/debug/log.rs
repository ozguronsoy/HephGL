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

impl std::fmt::Display for Log {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Severity: {:?}", self.severity)?;
        writeln!(f, "{}", self.message)?;
        Ok(())
    }
}
