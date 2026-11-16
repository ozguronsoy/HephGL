mod log;
mod object;
mod severity;
pub(super) mod writer;

use std::sync::{Mutex, MutexGuard, PoisonError};

pub use super::debug::{log::*, object::*, severity::*};
use crate::{Error, Result, renderers::debug::writer::DebugWriter};

/// Provides debugging facilities for inspecting and diagnosing the renderer.
#[derive(Default)]
pub struct Debugger {
    logs: Mutex<Vec<Log>>,
    writer: Mutex<Option<Box<dyn DebugWriter>>>,
}

impl Debugger {
    /// Gets the logs recorded so far.
    pub fn logs(&self) -> MutexGuard<'_, Vec<Log>> {
        self.logs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sets the name of the object, which shows up in validation messages and graphics debuggers.
    pub fn set_object_name(&self, object: &dyn RendererDebugObject, name: &str) -> Result<()> {
        self.writer
            .lock()?
            .as_ref()
            .ok_or(Error::invalid_operation(
                "Debugger is not attached to a renderer.",
            ))?
            .set_object_name(object, name)
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

    pub(super) fn set_writer(&self, writer: Option<Box<dyn DebugWriter>>) {
        *self.writer.lock().unwrap_or_else(PoisonError::into_inner) = writer;
    }

    pub(super) fn clone(&self) -> Self {
        let clone = Self::default();
        for log in self.logs().iter() {
            clone.log(log.message(), log.severity());
        }
        clone
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
