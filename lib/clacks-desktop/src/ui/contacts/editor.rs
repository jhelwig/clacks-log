//! Independent saved-contact editing with a retained expected record.

use super::super::validation::ValidationState;
use crate::logging::form::ContactForm;
use crate::operating::FieldError;
use clacks_model::ids::QsoId;
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordState, RecordValue},
};

/// Saved facts being edited, never the unfinished new-contact entry.
pub(super) struct ContactEditor {
    /// Complete record used by the store's stale-save protection.
    pub expected: ClientRecord,
    /// Editable copied historical facts.
    pub form: ContactForm,
    /// Initial form for changed-state display.
    pub baseline: ContactForm,
    /// Current debounced native validation state.
    pub validation: ValidationState,
    /// Field errors from the complete contact.
    pub errors: Vec<FieldError>,
}

impl ContactEditor {
    /// Open only actual live local working contact facts.
    pub fn from_record(expected: ClientRecord) -> anyhow::Result<Self> {
        let Some(RecordState::Live(RecordValue::Qso(qso))) = expected.working_state() else {
            anyhow::bail!("This contact is no longer available as a live record")
        };
        let form = ContactForm::from_qso(qso);
        Ok(Self {
            expected,
            baseline: form.clone(),
            form,
            validation: Default::default(),
            errors: vec![],
        })
    }

    /// Return the native identity retained by this saved editor.
    pub fn id(&self) -> anyhow::Result<QsoId> {
        match self.expected.key.id {
            RecordId::Qso(id) => Ok(id),
            _ => anyhow::bail!("Editor does not contain a contact"),
        }
    }
}
