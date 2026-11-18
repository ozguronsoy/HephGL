mod label;
mod log;
mod object;
mod severity;
mod writer;

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, PoisonError},
    thread::ThreadId,
};

pub(super) use super::debug::{label::*, writer::*};
pub use super::debug::{log::*, object::*, severity::*};
use crate::{Error, Result};

/// Provides debugging facilities for inspecting and diagnosing the renderer.
#[derive(Default)]
pub struct Debugger {
    logs: Mutex<Vec<Log>>,
    labels: Mutex<HashMap<ThreadId, Vec<DebugLabel>>>,
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

    /// Pushes a label to the debugger, which will begin while recording the next command.
    pub fn push_label(&self, label: &str) -> Result<()> {
        if label.contains('\0') {
            return Err(Error::invalid_argument(
                "Label cannot contain a null character.",
            ));
        }
        self.labels
            .lock()?
            .entry(std::thread::current().id())
            .or_default()
            .push(DebugLabel::new(label));
        Ok(())
    }

    /// Pops the most recently pushed label. The label either ends while recording the next command,
    /// or at the end of the current frame.
    pub fn pop_label(&self) -> Result<()> {
        let mut labels = self.labels.lock()?;
        let Some(labels) = labels.get_mut(&std::thread::current().id()) else {
            return Err(Error::invalid_operation("No label found to pop."));
        };
        let Some(index) = labels
            .iter()
            .rposition(|label| label.state != DebugLabelState::Ended)
        else {
            return Err(Error::invalid_operation("No label found to pop."));
        };
        if labels[index].state == DebugLabelState::Idle {
            let _ = labels.remove(index);
        } else {
            labels[index].state = DebugLabelState::Ended;
        }
        Ok(())
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

    pub(super) fn writer(&self) -> Result<MutexGuard<'_, Option<Box<dyn DebugWriter>>>> {
        Ok(self.writer.lock()?)
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

    /// Indicates whether the current thread has any labels that haven't ended yet.
    pub(super) fn has_labels(&self) -> bool {
        if let Ok(labels) = self.labels.lock()
            && let Some(labels) = labels.get(&std::thread::current().id())
        {
            !labels.is_empty()
        } else {
            false
        }
    }

    /// Sets the state of the current thread's `Idle` labels to `Begun` and stores the hash of the
    /// target they begin in. Returns their names from the outermost to the innermost.
    pub(super) fn begin_labels(&self, target_hash: u64) -> Vec<String> {
        if let Ok(mut labels) = self.labels.lock()
            && let Some(labels) = labels.get_mut(&std::thread::current().id())
        {
            labels
                .iter_mut()
                .filter(|label| label.state == DebugLabelState::Idle)
                .map(|label| {
                    label.state = DebugLabelState::Begun;
                    label.target_hash = target_hash;
                    label.label.clone()
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    }

    /// Removes the current thread's `Ended` labels that have begun in the target with the provided
    /// hash, and returns how many were removed.
    pub(super) fn end_labels(&self, target_hash: u64) -> usize {
        if let Ok(mut labels) = self.labels.lock()
            && let Some(labels) = labels.get_mut(&std::thread::current().id())
        {
            let count = labels.len();
            labels.retain(|label| {
                label.state != DebugLabelState::Ended || label.target_hash != target_hash
            });
            count - labels.len()
        } else {
            0
        }
    }

    /// Removes the labels of the current thread.
    pub(super) fn clear_labels(&self) {
        let _ = self
            .labels
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&std::thread::current().id());
    }

    /// Removes the labels of all threads.
    pub(super) fn clear_all_labels(&self) {
        self.labels
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
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
    fn test_labels() {
        const GRAPHICS: u64 = 1;
        const COMPUTE: u64 = 2;

        let debugger = Debugger::default();
        assert!(matches!(
            debugger.pop_label(),
            Err(Error::InvalidOperation(_))
        ));
        assert!(matches!(
            debugger.push_label("Sh\0ip"),
            Err(Error::InvalidArgument(_))
        ));
        assert!(!debugger.has_labels());

        assert!(debugger.push_label("Ship").is_ok());
        assert!(debugger.push_label("Deck").is_ok());
        assert_eq!(debugger.end_labels(GRAPHICS), 0);
        assert_eq!(debugger.begin_labels(GRAPHICS), ["Ship", "Deck"]);
        assert!(debugger.begin_labels(GRAPHICS).is_empty());

        assert!(debugger.pop_label().is_ok());
        assert!(debugger.push_label("Sails").is_ok());
        assert_eq!(debugger.end_labels(GRAPHICS), 1);
        assert_eq!(debugger.begin_labels(GRAPHICS), ["Sails"]);

        assert!(debugger.push_label("Mast").is_ok());
        assert!(debugger.pop_label().is_ok());
        assert!(debugger.begin_labels(GRAPHICS).is_empty());

        assert!(debugger.pop_label().is_ok());
        assert!(debugger.pop_label().is_ok());
        assert!(debugger.has_labels());
        assert!(matches!(
            debugger.pop_label(),
            Err(Error::InvalidOperation(_))
        ));
        assert_eq!(debugger.end_labels(COMPUTE), 0);
        assert_eq!(debugger.end_labels(GRAPHICS), 2);
        assert!(!debugger.has_labels());

        assert!(debugger.push_label("Ship").is_ok());
        assert_eq!(debugger.begin_labels(GRAPHICS), ["Ship"]);
        assert!(debugger.push_label("Cannon").is_ok());
        assert_eq!(debugger.begin_labels(COMPUTE), ["Cannon"]);
        assert!(debugger.pop_label().is_ok());
        assert!(debugger.pop_label().is_ok());
        assert_eq!(debugger.end_labels(GRAPHICS), 1);
        assert!(debugger.has_labels());
        assert_eq!(debugger.end_labels(COMPUTE), 1);
        assert!(!debugger.has_labels());

        assert!(debugger.push_label("Ship").is_ok());
        assert!(debugger.has_labels());
        debugger.clear_labels();
        assert!(!debugger.has_labels());
        assert!(debugger.begin_labels(GRAPHICS).is_empty());
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
