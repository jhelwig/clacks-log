//! Checked conversion between contact input and saved values.

use super::{BandFrequency, OperatorInformation, OwnOperatingContext, Qso, StationSnapshot};
use crate::ids::{QsoId, TagId};
use crate::scalar::UtcSecond;

use crate::recognition::mode::Mode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Editable contact input checked before becoming a saved QSO.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QsoFields {
    /// Stable account-wide contact identity, independent of tags.
    pub id: QsoId,
    /// Required whole-second UTC start timestamp.
    pub start: UtcSecond,
    /// Optional whole-second UTC end, no earlier than start.
    pub end: Option<UtcSecond>,
    /// Own station snapshot captured for this contact.
    pub own_station: StationSnapshot,
    /// Contacted station snapshot captured for this contact.
    pub contacted_station: StationSnapshot,
    /// Own operator, power and equipment for this contact.
    pub own_context: OwnOperatingContext,
    /// Separately recorded contacted operator information, when known.
    pub contacted_operator: Option<OperatorInformation>,
    /// Recorded primary frequency/band pair.
    pub primary: BandFrequency,
    /// Optional separate receive pair; see [`crate::contact::BandFrequency::effective_receive`].
    pub receive_override: Option<BandFrequency>,
    /// Recorded mode hierarchy and labels independent of recognition.
    pub mode: Mode,
    /// Optional sent signal report; report syntax is not mode-restricted here.
    pub sent_report: Option<String>,
    /// Optional received signal report; report syntax is not mode-restricted here.
    pub received_report: Option<String>,
    /// Optional recorded comment text.
    pub comment: Option<String>,
    /// Complete unordered set of logbook memberships; an empty set leaves the contact untagged.
    pub tags: BTreeSet<TagId>,
}

impl TryFrom<QsoFields> for Qso {
    type Error = anyhow::Error;

    fn try_from(fields: QsoFields) -> anyhow::Result<Self> {
        let value = Self {
            id: fields.id,
            start: fields.start,
            end: fields.end,
            own_station: fields.own_station,
            contacted_station: fields.contacted_station,
            own_context: fields.own_context,
            contacted_operator: fields.contacted_operator,
            primary: fields.primary,
            receive_override: fields.receive_override,
            mode: fields.mode,
            sent_report: fields.sent_report,
            received_report: fields.received_report,
            comment: fields.comment,
            tags: fields.tags,
        };
        value.validate()?;
        Ok(value)
    }
}

impl From<Qso> for QsoFields {
    fn from(value: Qso) -> Self {
        Self {
            id: value.id,
            start: value.start,
            end: value.end,
            own_station: value.own_station,
            contacted_station: value.contacted_station,
            own_context: value.own_context,
            contacted_operator: value.contacted_operator,
            primary: value.primary,
            receive_override: value.receive_override,
            mode: value.mode,
            sent_report: value.sent_report,
            received_report: value.received_report,
            comment: value.comment,
            tags: value.tags,
        }
    }
}
