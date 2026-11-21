//! Prepared ordinary and Parks on the Air ADI exports.

mod mapping;
mod pota;
pub(crate) mod prepare;
pub(crate) mod publish;

#[cfg(test)]
mod tests;

use clacks_model::{
    export::select::{ExportDecision, ExportWarning as ReferenceWarning},
    ids::QsoId,
};
use std::{collections::BTreeMap, fmt};

/// ADI mapping and file layout chosen by the operator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ExportFormat {
    /// One file and one row per contact, using standard interchange references.
    #[default]
    Ordinary,
    /// Separate files for each activated park and location, expanded by contacted park.
    Pota,
}

/// One preparation result for the complete selected scope.
#[derive(Clone, Debug, Default)]
pub(crate) struct PreparedExport {
    /// Format used to prepare these files.
    pub format: ExportFormat,
    /// Logical outputs before a destination is chosen.
    pub files: Vec<PreparedFile>,
    /// Required choices to resolve before publication.
    pub decisions: Vec<ExportDecision>,
    /// Contacts that need input to satisfy the target format's requirements.
    pub skipped: Vec<SkippedContact>,
    /// Reportable conversions and omissions.
    pub warnings: Vec<ExportWarning>,
    /// Number of distinct requested contacts.
    pub requested_count: usize,
}

impl PreparedExport {
    /// Count distinct requested contacts that can be written.
    pub(crate) fn eligible_count(&self) -> usize {
        self.requested_count.saturating_sub(self.skipped.len())
    }
}

/// ADI entries sharing one logical output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedFile {
    /// Park and location for POTA, or an empty key for ordinary interchange.
    pub partition: Vec<String>,
    /// Rows to write into this file.
    pub records: Vec<PreparedRecord>,
}

/// One ADI record tied to its native contact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedRecord {
    /// Saved native identity shared by any expanded rows.
    pub qso_id: QsoId,
    /// Canonical ADI record tags and encoded text values.
    pub fields: BTreeMap<String, String>,
}

/// A contact with unmet requirements for POTA output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SkippedContact {
    /// Native saved contact identity.
    pub qso_id: QsoId,
    /// Exact contacted callsign for local review.
    pub callsign: String,
    /// Fields requiring attention to include this contact.
    pub missing: Vec<ReadinessField>,
}

/// A missing or unrepresentable value required by the chosen export format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ReadinessField {
    /// A contacted callsign meeting the receiver's syntax.
    #[error("Valid contacted callsign (CALL)")]
    ContactedCallsign,
    /// The station's on-air callsign.
    #[error("Valid station callsign (STATION_CALLSIGN)")]
    StationCallsign,
    /// A contacted identity different from our station or operator.
    #[error("A contacted callsign different from your station and operator")]
    DistinctCallsign,
    /// The saved UTC start date and time within the output format's allowed range.
    #[error("Valid QSO start date and time (UTC)")]
    StartTime,
    /// The activator's saved transmit-band label recognized by the receiver.
    #[error("Supported transmit band (BAND)")]
    TransmitBand,
    /// A mode or submode recognized by the receiver.
    #[error("Supported mode or submode")]
    Mode,
    /// At least one valid own park reference.
    #[error("Valid activation park (MY_SIG_INFO)")]
    OwnPark,
    /// A consistent applicable location for each own park.
    #[error("Valid activation location (MY_STATE or park location qualifier)")]
    ActivationLocation,
}

/// A field conversion or omission attributed to a native contact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExportWarning {
    /// Contact affected by the conversion.
    pub qso_id: QsoId,
    /// Recorded contacted callsign for the local report.
    pub callsign: String,
    /// Affected field or source fact.
    pub field: String,
    /// Local explanation of the conversion or omission.
    pub detail: String,
}

impl fmt::Display for ExportWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} — {}. {}", self.callsign, self.field, self.detail)
    }
}

impl ExportWarning {
    /// Attach a reportable adapter decision to the contact being mapped.
    fn new(
        contact: &clacks_client_store::read::export::ExportContact,
        field: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            qso_id: contact.qso.id(),
            callsign: contact.qso.contacted_station().callsign.as_str().into(),
            field: field.into(),
            detail: detail.into(),
        }
    }

    /// Describe a reference planner omission with the local contact label.
    fn reference(
        contact: &clacks_client_store::read::export::ExportContact,
        warning: ReferenceWarning,
    ) -> Self {
        match warning {
            ReferenceWarning::OmittedContextField {
                field, candidates, ..
            } => Self::new(
                contact,
                field,
                format!(
                    "Omitted as requested ({}).",
                    candidates.values().cloned().collect::<Vec<_>>().join(", ")
                ),
            ),
            ReferenceWarning::UnsupportedGroup { group_id, .. } => Self::new(
                contact,
                "References",
                format!("Omitted participation in unavailable group {group_id}."),
            ),
        }
    }
}
