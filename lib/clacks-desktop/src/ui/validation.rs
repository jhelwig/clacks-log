//! Debounced validation readiness using egui's existing monotonic input clock.

use crate::operating::FieldError;
use eframe::egui;

/// Delay after the last text change before live validation runs.
const VALIDATION_DELAY: f64 = 0.300;

/// Deadline and freshness of a form's existing field errors, not a second validator.
#[derive(Default)]
pub(super) struct ValidationState {
    /// Next check, absent before initial validation or after a settled result.
    deadline: Option<f64>,
    /// Whether the current text has a settled validation result.
    checked: bool,
}

impl ValidationState {
    /// Invalidate a previous result and debounce checking from this effective edit.
    pub fn edited(&mut self, now: f64) {
        self.deadline = Some(now + VALIDATION_DELAY);
        self.checked = false;
    }

    /// Require a fresh check after a source/definition change without adding a delay.
    pub fn invalidate(&mut self) {
        self.deadline = None;
        self.checked = false;
    }

    /// Report whether current form values should be checked during this pass.
    pub fn due(&self, now: f64) -> bool {
        !self.checked && self.deadline.is_none_or(|deadline| now >= deadline)
    }

    /// Associate the owning form's validation result with its current text.
    pub fn validated(&mut self) {
        self.deadline = None;
        self.checked = true;
    }

    /// Enable a submission only for current, error-free text.
    pub fn can_submit(&self, errors: &[FieldError]) -> bool {
        self.checked && errors.is_empty()
    }

    /// Wake the UI at the deadline without a continuously running redraw loop.
    pub fn request_repaint(&self, context: &egui::Context, now: f64) {
        if let Some(deadline) = self.deadline {
            context.request_repaint_after(std::time::Duration::from_secs_f64(
                (deadline - now).max(0.0),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_waits_300ms_and_restarts_for_new_text() {
        let mut state = ValidationState::default();
        state.edited(0.0);
        assert!(!state.due(0.299));
        assert!(state.due(0.300));
        state.edited(0.250);
        assert!(!state.due(0.549));
        assert!(state.due(0.550));
        assert!(!state.can_submit(&[]));
        state.validated();
        assert!(state.can_submit(&[]));
        state.edited(1.0);
        assert!(!state.can_submit(&[]));
    }
}
