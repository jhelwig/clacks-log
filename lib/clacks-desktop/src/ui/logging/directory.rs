//! Separate provider facts never flow into editable recorded controls.

use crate::{
    directory::lookup::{LookupCommand, LookupState, LookupUpdate},
    session::settings::{SettingsAction, SettingsRequest},
};
use clacks_model::{
    directory::values::{DirectorySnapshot, LookupKey},
    ids::QsoId,
    scalar::RecordedCallsign,
};
use eframe::egui;
use tokio::sync::{mpsc, oneshot};

/// Selected directory hints for the existing contact controls, never save input.
#[derive(Default)]
pub(in crate::ui) struct DirectoryFallback {
    /// Selected directory's visible source name.
    pub source: &'static str,
    /// Name used only while recorded name is absent.
    pub name: Option<String>,
    /// Whole selected directory location and location-bearing references.
    pub station: Option<crate::operating::station::StationForm>,
    /// Provider locality metadata, separate from geographic identifiers.
    pub locality: Option<String>,
}

impl DirectoryFallback {
    /// Supply the selected name as a native nonrecording field hint.
    pub fn name(&self) -> Option<crate::ui::fields::FieldFallback<'_>> {
        self.name
            .as_deref()
            .map(|value| crate::ui::fields::FieldFallback {
                value,
                source: self.source,
            })
    }

    /// Supply the selected whole location to the shared station controls.
    pub fn station(&self) -> Option<crate::ui::operating::fields::StationFallback<'_>> {
        self.station
            .as_ref()
            .map(|station| crate::ui::operating::fields::StationFallback {
                station,
                source: self.source,
            })
    }

    /// Present directory-only metadata in Contact details, not the entry actions.
    pub fn details(&self, ui: &mut egui::Ui) {
        if let Some(locality) = &self.locality {
            ui.small(format!("{} locality: {locality}", self.source));
        }
        if self.source == "HamQTH" {
            ui.hyperlink_to(
                "Callbook data provided by HamQTH.com",
                "https://www.hamqth.com/",
            );
        }
    }
}

/// Presentation remembers only the active form's observed key and successful preview.
#[derive(Default)]
pub(in crate::ui) struct DirectoryView {
    /// Last notification admitted to the session.
    observed: Option<(QsoId, Option<LookupKey>)>,
    /// Latest safe result for this form.
    state: Option<LookupState>,
    /// Retained success is not removed by failure or not-found.
    snapshot: Option<DirectorySnapshot>,
}

/// Derive the same checked key used by lookup persistence.
fn key(call: &str) -> Option<LookupKey> {
    RecordedCallsign::new(call)
        .ok()
        .as_ref()
        .map(LookupKey::from_recorded)
}

/// Submit through the normal ordered application channel; retry on a later frame if full.
fn send(requests: &mpsc::Sender<SettingsRequest>, command: LookupCommand) -> bool {
    requests
        .try_send(SettingsRequest {
            action: SettingsAction::Directory(command),
            reply: oneshot::channel().0,
        })
        .is_ok()
}

impl DirectoryView {
    /// Select matching-key hints using the model's name and whole-location precedence.
    pub(in crate::ui) fn fallback(
        &self,
        form: &crate::logging::form::ContactForm,
        definitions: &clacks_model::group::DefinitionSet,
    ) -> DirectoryFallback {
        use clacks_model::effective::{
            self, EligibleStationFacts, InformationSource, StationFacts,
        };
        let Some(snapshot) = self
            .snapshot
            .as_ref()
            .filter(|snapshot| Some(snapshot.key()) == key(&form.dx.callsign).as_ref())
        else {
            return DirectoryFallback::default();
        };
        let facts = StationFacts {
            name: snapshot.facts().name.clone(),
            location: snapshot.facts().location.clone(),
            references: snapshot.facts().references.clone(),
        };
        let eligible = EligibleStationFacts {
            directory: Some(&facts),
            ..Default::default()
        };
        let name = effective::select_name(Some(&form.dx_name), &eligible)
            .filter(|selected| selected.source == InformationSource::Directory)
            .map(|selected| selected.value);
        // Unfinished non-location reference rows do not govern location precedence.
        let mut location_form = form.dx.clone();
        location_form.references.retain(|row| {
            row.group
                .and_then(|id| definitions.get(id))
                .is_some_and(|definition| definition.bears_location_context())
        });
        let station = location_form
            .build_recorded(definitions)
            .ok()
            .and_then(|recorded| {
                effective::select_location(&recorded, &eligible, definitions)
                    .filter(|selected| selected.source == InformationSource::Directory)
                    .map(|selected| {
                        crate::operating::station::StationForm::from_snapshot(
                            &clacks_model::contact::StationSnapshot {
                                callsign: recorded.callsign,
                                location: selected.value.location,
                                references: selected.value.references,
                            },
                        )
                    })
            });
        DirectoryFallback {
            source: match snapshot.provider() {
                clacks_model::directory::values::DirectoryProvider::Qrz => "QRZ",
                clacks_model::directory::values::DirectoryProvider::HamQth => "HamQTH",
            },
            name,
            station,
            locality: snapshot.facts().locality.clone(),
        }
    }
    /// Opening an unchanged saved contact presents retained facts without refreshing it.
    pub(in crate::ui) fn saved(qso: QsoId, call: &str) -> Self {
        Self {
            observed: Some((qso, key(call))),
            ..Default::default()
        }
    }

    /// Restore its already retained success without copying anything into recorded fields.
    pub(in crate::ui) fn retain(&mut self, snapshot: Option<Box<DirectorySnapshot>>) {
        self.snapshot = snapshot.map(|value| *value);
    }

    /// Notify only changed canonical calls, preserving a debounce across spelling-only changes.
    pub(in crate::ui) fn observe(
        &mut self,
        qso: QsoId,
        call: &str,
        requests: &mpsc::Sender<SettingsRequest>,
    ) {
        let next = (qso, key(call));
        if self.observed.is_none() && next.1.is_none() {
            self.observed = Some(next);
            return;
        }
        if self.observed.as_ref() == Some(&next) {
            return;
        }
        if let Some((previous, _)) = &self.observed
            && *previous != qso
            && !send(requests, LookupCommand::Deleted { qso: *previous })
        {
            return;
        }
        if send(
            requests,
            LookupCommand::CallChanged {
                qso,
                call: call.to_owned(),
            },
        ) {
            self.observed = Some(next);
            self.state = None;
            self.snapshot = None;
        }
    }

    /// Match immutable response ownership against the current form, including unsent edits.
    pub(in crate::ui) fn update(&mut self, qso: QsoId, call: &str, update: &LookupUpdate) {
        if update.qso != qso || update.key != key(call) {
            return;
        }
        if let LookupState::Found(snapshot) = &update.state
            && self
                .snapshot
                .as_ref()
                .is_none_or(|current| current.compare_lookup(snapshot).is_lt())
        {
            self.snapshot = Some(snapshot.as_ref().clone());
        }
        self.state = Some(update.state.clone());
    }

    /// Attach only a matching success to the atomic contact save.
    pub(in crate::ui) fn snapshot(&self, call: &str) -> Option<Box<DirectorySnapshot>> {
        self.snapshot
            .as_ref()
            .filter(|snapshot| Some(snapshot.key()) == key(call).as_ref())
            .cloned()
            .map(Box::new)
    }

    /// Present local lookup progress/failures without duplicating returned facts.
    pub(in crate::ui) fn show(
        &self,
        ui: &mut egui::Ui,
        qso: QsoId,
        requests: &mpsc::Sender<SettingsRequest>,
    ) {
        match &self.state {
            Some(LookupState::Pending) => {
                ui.small("Looking up callsign…");
            }
            Some(LookupState::NotFound) => {
                ui.small("Callsign not found in the selected directory.");
            }
            Some(LookupState::PausedAuthentication) => {
                ui.small("Directory credentials were rejected.");
                if ui.button("Retry directory login").clicked() {
                    send(requests, LookupCommand::Retry { qso });
                }
            }
            Some(LookupState::Failed(message)) => {
                ui.small(message);
                if ui.button("Retry directory lookup").clicked() {
                    send(requests, LookupCommand::Retry { qso });
                }
            }
            _ => {}
        }
    }
}
