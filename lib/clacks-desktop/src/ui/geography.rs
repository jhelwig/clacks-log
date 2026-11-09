//! Caller-owned geographic replies and candidate presentation, using shared native rules.

use crate::{
    geography::{GeographyFailure, GeographyOperation, GeographyStatus},
    operating::station::StationForm,
    session::SessionCommand,
};
use clacks_geography::{
    artifact::Field,
    lookup::{Coverage, FieldLookup, LookupResult},
};
use clacks_model::position::Position;
use eframe::egui;
use tokio::sync::{mpsc, oneshot};

#[cfg(test)]
mod tests;

/// One station operation's interest, never shared with an abandoned or submitted form.
#[derive(Default)]
pub(super) struct GeographyView {
    /// Native basis for the current requested/displayed result.
    basis: Option<Position>,
    /// An admitted operation's owned reply.
    pending: Option<oneshot::Receiver<Result<LookupResult, GeographyFailure>>>,
    /// A valid intent awaiting ordinary command admission/readiness.
    needed: bool,
    /// Candidates for the current position only.
    result: Option<LookupResult>,
    /// Safe user-facing assistance diagnosis.
    message: Option<&'static str>,
}

impl GeographyView {
    /// Drop interest without changing recorded or pending station values.
    pub(super) fn retire(&mut self) {
        *self = Self::default();
    }

    /// Advance explicit position intent and apply eligible replies before parent submission.
    pub(super) fn advance(
        &mut self,
        form: &mut StationForm,
        commands: &mpsc::Sender<SessionCommand>,
        status: &GeographyStatus,
        frozen: bool,
        context: &'static str,
    ) -> bool {
        if frozen {
            self.retire();
            let _ = form.take_geographic_intent();
            return false;
        }
        if let Some(intent) = form.take_geographic_intent() {
            self.retire();
            if let Ok(Some(position)) = intent {
                self.basis = Some(position);
                self.needed = true;
            }
        }
        if let Some(basis) = &self.basis {
            let current = form.position.build_recorded();
            if !matches!(current, Ok(Some(ref position)) if basis.same_geographic_position(position))
            {
                self.retire();
            }
        }
        let mut changed = false;
        if let Some(reply) = &mut self.pending {
            match reply.try_recv() {
                Ok(Ok(result)) => {
                    changed = form.apply_geography(&result);
                    self.result = Some(result);
                    self.pending = None;
                    self.message = None;
                }
                Ok(Err(error)) => {
                    self.message = Some(error.message());
                    self.pending = None;
                }
                Err(oneshot::error::TryRecvError::Closed) => {
                    self.message =
                        Some("Geographic assistance did not finish. Edit the position to retry.");
                    self.pending = None;
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
            }
        }
        if self.needed && self.pending.is_none() {
            match status {
                GeographyStatus::Ready(_) => {
                    if let Some(position) = &self.basis {
                        let (reply, receiver) = oneshot::channel();
                        match commands.try_send(SessionCommand::Geography(GeographyOperation {
                            position: position.clone(),
                            reply,
                            context,
                        })) {
                            Ok(()) => {
                                self.pending = Some(receiver);
                                self.needed = false;
                                self.message = None;
                            }
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                self.message =
                                    Some("Geographic assistance is waiting for current work.");
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                self.needed = false;
                                self.message =
                                    Some("Geographic assistance is unavailable while closing.");
                            }
                        }
                    }
                }
                GeographyStatus::Opening => self.message = Some("Opening geographic data…"),
                GeographyStatus::Unavailable(error) => self.message = Some(error.message()),
            }
        }
        changed
    }

    /// Show nonmodal status beside quick position entry without duplicating candidate controls.
    pub(super) fn summary(&self, ui: &mut egui::Ui) {
        if let Some(message) = self.message {
            ui.small(message);
        }
        if self.pending.is_some() {
            ui.small("Looking up geographic details…");
        }
    }

    /// Present independent candidates and coverage, recording only explicit selections.
    pub(super) fn show(&self, ui: &mut egui::Ui, form: &mut StationForm) -> bool {
        if form.has_geographic_intent() || self.basis.as_ref().is_some_and(|basis| {
            !matches!(form.position.build_recorded(), Ok(Some(ref current)) if basis.same_geographic_position(current))
        }) {
            return false;
        }
        self.summary(ui);
        let Some(result) = &self.result else {
            return false;
        };
        let mut changed = false;
        for field in Field::ALL {
            let selected = match field {
                Field::Dxcc => choose(ui, "DXCC entity", &result.dxcc, Clone::clone),
                Field::PrimarySubdivision => choose(
                    ui,
                    "State / province",
                    &result.primary_subdivision,
                    Clone::clone,
                ),
                Field::SecondarySubdivision => choose(
                    ui,
                    "County / subdivision",
                    &result.secondary_subdivision,
                    Clone::clone,
                ),
                Field::CqZone => choose(ui, "CQ zone", &result.cq_zone, |value| {
                    value.get().to_string()
                }),
                Field::ItuZone => choose(ui, "ITU zone", &result.itu_zone, |value| {
                    value.get().to_string()
                }),
            };
            if let Some(value) = selected {
                match field {
                    Field::Dxcc => form.dxcc = value,
                    Field::PrimarySubdivision => form.primary_subdivision = value,
                    Field::SecondarySubdivision => form.secondary_subdivision = value,
                    Field::CqZone => form.cq_zone = value,
                    Field::ItuZone => form.itu_zone = value,
                }
                form.geographic_protection.insert(field);
                changed = true;
            }
        }
        changed
    }
}

/// Show choices only where the independent native result is not eligible for autofill.
fn choose<T>(
    ui: &mut egui::Ui,
    label: &str,
    lookup: &FieldLookup<T>,
    text: impl Fn(&T) -> String,
) -> Option<String> {
    if lookup.coverage == Coverage::Complete && lookup.candidates.len() == 1 {
        return None;
    }
    let coverage = match lookup.coverage {
        Coverage::Complete => "Several geographic values",
        Coverage::Partial => "Incomplete geographic coverage",
        Coverage::None => "No geographic coverage",
    };
    let mut selected = None;
    ui.push_id(label, |ui| {
        ui.small(format!("{label}: {coverage}"));
        if !lookup.candidates.is_empty() {
            ui.menu_button(format!("Choose {label}"), |ui| {
                for candidate in &lookup.candidates {
                    let label = match &candidate.parent_scope {
                        clacks_geography::artifact::ParentScope::Global => {
                            candidate.display_label.clone()
                        }
                        clacks_geography::artifact::ParentScope::Entity(entity) => {
                            format!("{} · DXCC {entity}", candidate.display_label)
                        }
                        clacks_geography::artifact::ParentScope::Subdivision {
                            entity,
                            primary,
                        } => format!("{} · {primary} · DXCC {entity}", candidate.display_label),
                    };
                    if ui.button(label).clicked() {
                        selected = Some(text(&candidate.value));
                        ui.close();
                    }
                }
            });
        }
    });
    selected
}
