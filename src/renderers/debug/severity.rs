/// Defines the severity of a debug log reported by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The operation is invalid or failed, and the result is likely incorrect.
    Error,
    /// The operation is valid but likely unintended or inefficient.
    Warning,
    /// Informational message that does not indicate a problem.
    Info,
}
