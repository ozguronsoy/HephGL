mod log;
mod severity;

use std::sync::{Mutex, MutexGuard, PoisonError};

pub use super::debug::{log::*, severity::*};

/// Provides debugging facilities for inspecting and diagnosing the renderer.
#[derive(Default)]
pub struct Debugger {
    logs: Mutex<Vec<Log>>,
}

impl Debugger {
    /// Gets the logs recorded so far.
    pub fn logs(&self) -> MutexGuard<'_, Vec<Log>> {
        self.logs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Appends a single log to the recorded logs.
    pub(super) fn log(&self, message: impl Into<String>, severity: Severity) {
        if let Ok(logs) = &mut self.logs.lock() {
            logs.push(Log {
                message: message.into(),
                severity,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logs() {
        const MESSAGE: &str = "Hello World!";
        const SEVERITY: Severity = Severity::Warning;

        let debugger = Debugger::default();
        assert!(debugger.logs().is_empty());
        debugger.log(MESSAGE, SEVERITY);
        assert_eq!(debugger.logs().len(), 1);
        assert_eq!(debugger.logs()[0].message(), MESSAGE);
        assert_eq!(debugger.logs()[0].severity(), SEVERITY);
    }
}
