//! Complete recorded contact input shared by new entry and saved editing.

use super::{radio::RadioForm, time::ContactTimestamp};
use crate::operating::{
    FieldError, equipment::EquipmentForm, issue, optional, power, station::StationForm,
};
use clacks_model::{
    contact::{OperatorInformation, OwnOperatingContext, Qso},
    group::DefinitionSet,
    ids::{QsoId, TagId},
    position::Position,
    recognition::{
        RecognitionCatalog,
        mode::{Mode, ModeIdentity},
    },
    scalar::RecordedCallsign,
};
use std::collections::BTreeSet;

/// Full contact text and original values retained across unrelated edits.
#[derive(Clone, Debug)]
pub(crate) struct ContactForm {
    /// Own station facts; profile selection does not belong to the contact.
    pub own: StationForm,
    /// Contacted station facts shared by quick entry and details.
    pub dx: StationForm,
    /// Independently optional contacted operator's recorded name.
    pub dx_name: String,
    /// Separately known contacted operator call, never inferred from station call.
    pub dx_operator: String,
    /// Own operator callsign.
    pub operator: String,
    /// Own operator's recorded name.
    pub operator_name: String,
    /// Decimal watts.
    pub power: String,
    /// Copied equipment descriptions.
    pub equipment: EquipmentForm,
    /// Required primary radio information.
    pub primary: RadioForm,
    /// Optional receive override.
    pub receive: RadioForm,
    /// Recorded effective mode label.
    pub mode: String,
    /// Optional explicit parent for a specialization, including unfamiliar labels.
    pub mode_parent: String,
    /// Required full UTC start.
    pub start: ContactTimestamp,
    /// Optional full UTC end.
    pub end: ContactTimestamp,
    /// Optional sent report.
    pub sent: String,
    /// Optional received report.
    pub received: String,
    /// Recorded general comment.
    pub comment: String,
    /// Whole-record memberships, unchanged during saved fact editing.
    pub tags: BTreeSet<TagId>,
    /// Original mode and displayed inputs preserve an unedited native hierarchy.
    original_mode: Option<Mode>,
    /// Unchanged coarse own positions stay valid when editing recorded facts.
    original_own_position: Option<Position>,
    /// Original complete values preserve calendar-out-of-range recorded timestamps.
    original: Option<Box<Qso>>,
}

impl ContactForm {
    /// Start incomplete input without inventing recorded contact facts.
    pub fn empty() -> Self {
        Self {
            own: StationForm::empty(),
            dx: StationForm::empty(),
            dx_name: String::new(),
            dx_operator: String::new(),
            operator: String::new(),
            operator_name: String::new(),
            power: String::new(),
            equipment: EquipmentForm::default(),
            primary: RadioForm::default(),
            receive: RadioForm::default(),
            mode: String::new(),
            mode_parent: String::new(),
            start: ContactTimestamp::recorded(None),
            end: ContactTimestamp::recorded(None),
            sent: String::new(),
            received: String::new(),
            comment: String::new(),
            tags: BTreeSet::new(),
            original_mode: None,
            original_own_position: None,
            original: None,
        }
    }

    /// Copy all recorded facts without applying current operating defaults.
    pub fn from_qso(qso: &Qso) -> Self {
        let mut form = Self::empty();
        form.own = StationForm::from_snapshot(&qso.own_station);
        form.dx = StationForm::from_snapshot(&qso.contacted_station);

        form.dx_name = qso
            .contacted_operator
            .as_ref()
            .and_then(|v| v.recorded_name.clone())
            .unwrap_or_default();
        form.dx_operator = qso
            .contacted_operator
            .as_ref()
            .and_then(|v| v.callsign.as_ref())
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();

        form.operator = qso
            .own_context
            .operator
            .as_ref()
            .and_then(|v| v.callsign.as_ref())
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        form.operator_name = qso
            .own_context
            .operator
            .as_ref()
            .and_then(|v| v.recorded_name.clone())
            .unwrap_or_default();

        form.power = power::format_watts(qso.own_context.transmit_power);
        form.equipment = EquipmentForm::from_values(qso.own_context.equipment.as_ref());

        form.primary = RadioForm::from_pair(Some(&qso.primary));
        form.receive = RadioForm::from_pair(qso.receive_override.as_ref());
        let (label, parent) = mode_text(&qso.mode);
        form.mode = label;
        form.mode_parent = parent;
        form.original_mode = Some(qso.mode.clone());
        form.original_own_position = qso
            .own_station
            .location
            .as_ref()
            .and_then(|v| v.position.clone());

        form.start = ContactTimestamp::recorded(Some(qso.start));
        form.end = ContactTimestamp::recorded(qso.end);

        form.sent = qso.sent_report.clone().unwrap_or_default();
        form.received = qso.received_report.clone().unwrap_or_default();
        form.comment = qso.comment.clone().unwrap_or_default();
        form.tags = qso.tags.clone();
        form.original = Some(Box::new(qso.clone()));

        form
    }

    /// Build one validated contact, preserving untouched historical representations.
    pub fn build(&self, id: QsoId, definitions: &DefinitionSet) -> Result<Qso, Vec<FieldError>> {
        let own_station = self
            .own
            .build_recorded(definitions)
            .map_err(|v| side_errors("Our", v))?;
        if let Some(position) = own_station
            .location
            .as_ref()
            .and_then(|v| v.position.as_ref())
            && Some(position) != self.original_own_position.as_ref()
        {
            position
                .validate_operating_entry()
                .map_err(|e| vec![issue("Our position", e)])?;
        }
        let contacted_station = self
            .dx
            .build_recorded(definitions)
            .map_err(|v| side_errors("Contact", v))?;
        let primary = self
            .primary
            .build()
            .map_err(|e| vec![issue("Frequency / Band", e)])?
            .ok_or_else(|| vec![issue("Frequency / Band", "Enter a frequency or band")])?;
        let receive_override = self
            .receive
            .build()
            .map_err(|e| vec![issue("Receive", e)])?;
        let mode = self.build_mode().map_err(|e| vec![issue("Mode", e)])?;
        let original_start = self
            .original
            .as_ref()
            .filter(|v| self.start.text == super::time::format(v.start));
        let start = match original_start {
            Some(v) => v.start,
            None => self
                .start
                .value()
                .map_err(|e| vec![issue("Start", e)])?
                .ok_or_else(|| vec![issue("Start", "Enter UTC start date and time")])?,
        };
        let original_end = self
            .original
            .as_ref()
            .filter(|v| self.end.text == v.end.map(super::time::format).unwrap_or_default());
        let end = match original_end {
            Some(v) => v.end,
            None => self.end.value().map_err(|e| vec![issue("End", e)])?,
        };

        let equipment = if let Some(original) = &self.original
            && self.equipment == EquipmentForm::from_values(original.own_context.equipment.as_ref())
        {
            original.own_context.equipment.clone()
        } else {
            self.equipment.build()?
        };
        let mut value = Qso {
            id,
            start,
            end,
            own_station,
            contacted_station,
            own_context: OwnOperatingContext {
                operator: operator(&self.operator, &self.operator_name)
                    .map_err(|e| vec![issue("Our operator", e)])?,
                transmit_power: power::parse_watts(&self.power)
                    .map_err(|e| vec![issue("Power", e)])?,
                equipment,
            },
            contacted_operator: operator(&self.dx_operator, &self.dx_name)
                .map_err(|e| vec![issue("Contact operator", e)])?,
            primary,
            receive_override,
            mode,
            sent_report: optional(&self.sent),
            received_report: optional(&self.received),
            comment: optional(&self.comment),
            tags: self.tags.clone(),
        };
        value.normalize_optional_aggregates();
        value.validate().map_err(|e| {
            let field = if matches!(
                e.downcast_ref::<clacks_model::contact::RecordError>(),
                Some(clacks_model::contact::RecordError::EndBeforeStart { .. })
            ) {
                "End"
            } else {
                "QSO"
            };
            vec![issue(field, e)]
        })?;
        Ok(value)
    }

    /// Resolve explicitly edited mode input, retaining an untouched original hierarchy.
    fn build_mode(&self) -> anyhow::Result<Mode> {
        if let Some(original) = &self.original_mode
            && mode_text(original) == (self.mode.clone(), self.mode_parent.clone())
        {
            return Ok(original.clone());
        }
        let catalog = RecognitionCatalog::adif_318()?;
        let identity = if let Some(row) = catalog
            .submodes
            .iter()
            .find(|v| v.code.eq_ignore_ascii_case(self.mode.trim()))
        {
            ModeIdentity::Specialization {
                label: row.code.clone(),
                parent: Some(row.parent.clone()),
            }
        } else if !self.mode_parent.trim().is_empty() {
            ModeIdentity::Specialization {
                label: self.mode.trim().to_owned(),
                parent: Some(self.mode_parent.trim().to_owned()),
            }
        } else {
            ModeIdentity::Parent {
                label: self.mode.trim().to_owned(),
            }
        };
        Ok(Mode::from_recorded_identity(identity)?)
    }
}

/// Extract display inputs without throwing away an unfamiliar specialization's parent.
fn mode_text(mode: &Mode) -> (String, String) {
    match mode.identity() {
        ModeIdentity::Parent { label } => (label, String::new()),
        ModeIdentity::Specialization { label, parent } => (label, parent.unwrap_or_default()),
    }
}

/// Construct independently optional operator facts without inventing a callsign.
fn operator(call: &str, name: &str) -> anyhow::Result<Option<OperatorInformation>> {
    Ok(OperatorInformation {
        callsign: if call.trim().is_empty() {
            None
        } else {
            Some(RecordedCallsign::new(call)?)
        },
        recorded_name: optional(name),
    }
    .normalized())
}

/// Distinguish validation messages from the two station sides.
fn side_errors(side: &str, errors: Vec<FieldError>) -> Vec<FieldError> {
    errors
        .into_iter()
        .map(|mut e| {
            e.field = format!("{side} {}", e.field);
            e
        })
        .collect()
}
