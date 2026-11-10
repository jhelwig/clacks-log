//! Active-default following and explicit per-contact operating corrections.

use super::{form::ContactForm, time::ContactTimestamp};
use crate::operating::{FieldError, editor::OperatingEditor};
use clacks_client_store::{logbooks::selection::SelectedView, operating::OperatingState};
use clacks_model::{contact::Qso, group::DefinitionSet, ids::QsoId, scalar::UtcSecond};

/// Explicitly edited own facts; equal later defaults do not erase these choices.
#[derive(Clone, Debug, Default)]
struct Corrections {
    /// Own callsign was explicitly corrected.
    callsign: bool,
    /// Own position evidence was explicitly corrected.
    position: bool,
    /// Own recorded entity was explicitly corrected.
    dxcc: bool,
    /// Own state/province was explicitly corrected.
    primary_subdivision: bool,
    /// Own county/subdivision was explicitly corrected.
    secondary_subdivision: bool,
    /// Own CQ zone was explicitly corrected.
    cq_zone: bool,
    /// Own ITU zone was explicitly corrected.
    itu_zone: bool,
    /// Own participation/reference set was explicitly corrected.
    references: bool,
    /// Own operator callsign was explicitly corrected.
    operator: bool,
    /// Own operator name was explicitly corrected.
    operator_name: bool,
    /// Own transmit power was explicitly corrected or cleared.
    power: bool,
    /// Copied equipment description was explicitly corrected.
    description: bool,
    /// Copied antenna description was explicitly corrected.
    antenna: bool,
    /// Copied equipment notes were explicitly corrected.
    notes: bool,
}

/// One unfinished contact and the stable identity used by its save/retry operation.
pub(crate) struct NewEntry {
    /// Shared quick and adjacent-detail input.
    pub form: ContactForm,
    /// Stable until a successful save initializes the next contact.
    pub id: QsoId,
    /// Which own facts no longer follow active settings.
    corrections: Corrections,
}

impl NewEntry {
    /// Start one unfinished contact using current active values and view membership.
    pub fn new(active: &OperatingState, view: SelectedView, now: UtcSecond) -> Self {
        let mut entry = Self {
            form: ContactForm::empty(),
            id: QsoId::new(),
            corrections: Corrections::default(),
        };
        entry.form.start = ContactTimestamp::live(now);
        entry.form.end = ContactTimestamp::live(now);
        entry.refresh_defaults(active, view);
        entry
    }

    /// Mark only effective explicit edits; automatic refresh never calls this method.
    pub fn capture_corrections(&mut self, before: &ContactForm) {
        macro_rules! changed {
            ($flag:ident, $($field:ident).+) => {
                self.corrections.$flag |= before.$($field).+ != self.form.$($field).+;
            };
        }
        changed!(callsign, own.callsign);
        changed!(position, own.position);
        if before.own.position == self.form.own.position {
            changed!(dxcc, own.dxcc);
            changed!(primary_subdivision, own.primary_subdivision);
            changed!(secondary_subdivision, own.secondary_subdivision);
            changed!(cq_zone, own.cq_zone);
            changed!(itu_zone, own.itu_zone);
        } else {
            self.corrections.dxcc = false;
            self.corrections.primary_subdivision = false;
            self.corrections.secondary_subdivision = false;
            self.corrections.cq_zone = false;
            self.corrections.itu_zone = false;
        }
        changed!(references, own.references);

        changed!(operator, operator);
        changed!(operator_name, operator_name);
        changed!(power, power);

        changed!(description, equipment.description);
        changed!(antenna, equipment.antenna);
        changed!(notes, equipment.notes);
    }

    /// Refresh default-following facts and replace the new-contact tag proposal.
    pub fn refresh_defaults(&mut self, active: &OperatingState, view: SelectedView) {
        let source = OperatingEditor::new(active);
        macro_rules! follow {
            ($flag:ident, $($target:ident).+, $value:expr) => {
                if !self.corrections.$flag { self.form.$($target).+ = $value; }
            };
        }
        if !self.corrections.position {
            if self.form.own.follow_position(&source.station) {
                self.corrections.dxcc = false;
                self.corrections.primary_subdivision = false;
                self.corrections.secondary_subdivision = false;
                self.corrections.cq_zone = false;
                self.corrections.itu_zone = false;
            }
            follow!(dxcc, own.dxcc, source.station.dxcc);
            follow!(
                primary_subdivision,
                own.primary_subdivision,
                source.station.primary_subdivision
            );
            follow!(
                secondary_subdivision,
                own.secondary_subdivision,
                source.station.secondary_subdivision
            );
            follow!(cq_zone, own.cq_zone, source.station.cq_zone);
            follow!(itu_zone, own.itu_zone, source.station.itu_zone);
        }
        follow!(callsign, own.callsign, source.station.callsign);
        follow!(references, own.references, source.station.references);

        follow!(operator, operator, source.operator_callsign);
        follow!(operator_name, operator_name, source.operator_name);
        follow!(power, power, source.power_watts);
        follow!(
            description,
            equipment.description,
            source.equipment.description
        );
        follow!(antenna, equipment.antenna, source.equipment.antenna);
        follow!(notes, equipment.notes, source.equipment.notes);

        self.form.tags = match view {
            SelectedView::Logbook(id) => [id].into(),
            _ => Default::default(),
        };
    }

    /// Capture current live clocks once and validate all submitted recorded facts.
    pub fn submit(
        &mut self,
        now: UtcSecond,
        definitions: &DefinitionSet,
    ) -> Result<Qso, Vec<FieldError>> {
        self.form
            .start
            .tick(now)
            .map_err(|e| vec![crate::operating::issue("Start", e)])?;
        self.form
            .end
            .tick(now)
            .map_err(|e| vec![crate::operating::issue("End", e)])?;
        self.form.build(self.id, definitions)
    }

    /// Initialize a fresh contact, retaining only radio information.
    pub fn reset(&mut self, active: &OperatingState, view: SelectedView, now: UtcSecond) {
        let primary = self.form.primary.clone();
        let receive = self.form.receive.clone();
        let mode = self.form.mode.clone();
        let parent = self.form.mode_parent.clone();
        *self = Self::new(active, view, now);
        self.form.primary = primary;
        self.form.receive = receive;
        self.form.mode = mode;
        self.form.mode_parent = parent;
    }
}
