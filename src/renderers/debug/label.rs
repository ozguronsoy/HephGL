/// Defines the possible states of a label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in super::super) enum DebugLabelState {
    /// The label is pushed by the user but no commands have been recorded yet.
    Idle,
    /// The label has been begun by the renderer.
    Begun,
    /// The label has been popped by the user and will be destroyed on the next command.
    Ended,
}

#[derive(Clone)]
pub(in super::super) struct DebugLabel {
    pub(super) state: DebugLabelState,
    pub(super) label: String,
}

impl DebugLabel {
    pub(super) fn new(label: impl Into<String>) -> Self {
        Self {
            state: DebugLabelState::Idle,
            label: label.into(),
        }
    }
}
