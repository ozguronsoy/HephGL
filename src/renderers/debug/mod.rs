mod log;
mod object;
mod severity;
mod writer;

use std::sync::{Mutex, MutexGuard, PoisonError};

pub(super) use super::debug::writer::*;
pub use super::debug::{log::*, object::*, severity::*};
use crate::{Error, Result};

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

    /// Sets the writer of the debugger, or removes it if `writer` is `None`. Returns the previous
    /// writer if there is one.
    pub(super) fn set_writer(
        &self,
        writer: Option<Box<dyn DebugWriter>>,
    ) -> Option<Box<dyn DebugWriter>> {
        std::mem::replace(
            &mut *self.writer.lock().unwrap_or_else(PoisonError::into_inner),
            writer,
        )
    }

    /// Clones the debugger with only its logs.
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
    use std::sync::Arc;

    use super::*;

    struct TestObject;
    impl RendererDebugObject for TestObject {}

    struct TestWriter {
        names: Arc<Mutex<Vec<String>>>,
    }
    impl DebugWriter for TestWriter {
        fn set_object_name(&self, _: &dyn RendererDebugObject, name: &str) -> Result<()> {
            self.names.lock()?.push(name.to_string());
            Ok(())
        }
    }

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

    #[test]
    fn test_set_object_name() {
        const NAME: &str = "Test Object";

        let debugger = Debugger::default();
        assert!(matches!(
            debugger.set_object_name(&TestObject, NAME),
            Err(Error::InvalidOperation(_))
        ));

        let names = Arc::new(Mutex::new(Vec::new()));
        let _ = debugger.set_writer(Some(Box::new(TestWriter {
            names: names.clone(),
        })));
        assert!(debugger.set_object_name(&TestObject, NAME).is_ok());
        assert_eq!(*names.lock().unwrap(), [NAME]);

        let _ = debugger.set_writer(None);
        assert!(matches!(
            debugger.set_object_name(&TestObject, NAME),
            Err(Error::InvalidOperation(_))
        ));
        assert_eq!(names.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_clone() {
        const MESSAGE: &str = "Hello World!";
        const SEVERITY: Severity = Severity::Warning;

        let debugger = Debugger::default();
        debugger.log(MESSAGE, SEVERITY);
        let _ = debugger.set_writer(Some(Box::new(TestWriter {
            names: Arc::new(Mutex::new(Vec::new())),
        })));

        let clone = debugger.clone();
        assert_eq!(*clone.logs(), *debugger.logs());
        assert!(matches!(
            clone.set_object_name(&TestObject, "Test Object"),
            Err(Error::InvalidOperation(_))
        ));
    }
}
