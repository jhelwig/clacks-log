//! Separate provider facts never flow into editable recorded controls.

use crate::directory::lookup::{LookupClient, LookupDemand, LookupHandle, LookupState};
use clacks_model::{
    directory::values::{DirectorySnapshot, LookupKey},
    scalar::RecordedCallsign,
};
use eframe::egui;
use std::sync::Arc;
use tokio::{sync::watch, time::Instant};

/// Directory hints for contact controls, separate from recorded save input.
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

/// Caller-owned display state; provider operations remain shared by lookup key.
#[derive(Default)]
pub(in crate::ui) struct DirectoryView {
    /// Effective entry text is distinct from its directory request key.
    observed: Option<String>,
    /// Current requested station, absent for empty/unusable input.
    key: Option<LookupKey>,
    /// Original last-change debounce deadline.
    due: Option<Instant>,
    /// Only this consumer's interest is dropped on clear/change.
    handle: Option<LookupHandle>,
    /// Shared presentation outcome, retained without a per-frame copy of its facts.
    state: Option<Arc<LookupState>>,
    /// Eligible prior success survives failure/not-found for the same key.
    snapshot: Option<DirectorySnapshot>,
    /// Workspace identity prevents cross-workspace consumer reuse.
    client: Option<LookupClient>,
    /// Configuration recovery resumes an unsuccessful active consumer.
    configuration: Option<watch::Receiver<()>>,
}

/// Use the model's request transformation without rewriting recorded facts.
fn key(call: &str) -> Option<LookupKey> {
    RecordedCallsign::new(call)
        .ok()
        .as_ref()
        .map(LookupKey::from_recorded)
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
    /// Opening an unchanged saved contact does not request provider refresh.
    pub(in crate::ui) fn saved(call: &str) -> Self {
        Self {
            observed: Some(call.trim().to_uppercase()),
            key: key(call),
            ..Default::default()
        }
    }

    /// Present durable eligible information separately from recorded controls.
    pub(in crate::ui) fn retain(&mut self, snapshot: Option<Box<DirectorySnapshot>>) {
        self.snapshot = snapshot.map(|value| *value);
    }

    /// Restart debounce for effective text changes, including equal-key slash changes.
    pub(in crate::ui) fn observe(&mut self, call: &str, now: Instant) {
        let observed = call.trim().to_uppercase();
        if self.observed.as_ref() == Some(&observed) {
            return;
        }
        let next = key(call);
        if self.key != next || next.is_none() {
            self.snapshot = None;
        }
        self.observed = Some(observed);
        self.key = next;
        self.due = self
            .key
            .as_ref()
            .map(|_| now + std::time::Duration::from_millis(500));
        self.handle = None;
        self.state = None;
    }

    /// Poll caller deadlines and shared results even when the form pane is hidden.
    pub(in crate::ui) fn advance(
        &mut self,
        context: &egui::Context,
        lookup: Option<&LookupClient>,
        now: Instant,
    ) {
        let Some(lookup) = lookup else {
            if self.client.take().is_some() {
                self.configuration = None;
                self.handle = None;
                self.snapshot = None;
                self.state = None;
            }
            return;
        };
        if self
            .client
            .as_ref()
            .is_none_or(|current| !current.same_workspace(lookup))
        {
            if self.client.is_some() {
                self.handle = None;
                self.snapshot = None;
                self.state = None;
            }
            self.client = Some(lookup.clone());
            self.configuration = Some(lookup.configuration_changes());
        }
        if let Some(configuration) = &mut self.configuration
            && configuration.has_changed().unwrap_or(false)
        {
            let outcome = self.handle.as_ref().and_then(LookupHandle::ready);
            // Keep the watch transition unread while old material is still running.
            // Its success remains eligible; its failure can then resume with current material.
            if self.handle.is_none() || outcome.is_some() {
                configuration.borrow_and_update();
                let requested = self.due.is_some() || self.handle.is_some();
                let succeeded = matches!(outcome.as_deref(), Some(LookupState::Found(_)));
                if requested && !succeeded && self.snapshot.is_none() && self.key.is_some() {
                    self.handle = None;
                    self.due = Some(self.due.unwrap_or(now).max(now));
                }
            }
        }
        if let Some(due) = self.due {
            if due <= now {
                if let Some(key) = &self.key {
                    self.handle = Some(lookup.request(key.clone()));
                }
                self.due = None;
                self.state = Some(Arc::new(LookupState::Pending));
            } else {
                context.request_repaint_after(due - now);
            }
        }
        if matches!(
            self.handle
                .as_ref()
                .and_then(LookupHandle::ready)
                .as_deref(),
            Some(LookupState::PausedAuthentication)
        ) && lookup.can_resume_authentication()
            && let Some(key) = &self.key
        {
            self.handle = Some(lookup.request(key.clone()));
            self.state = Some(Arc::new(LookupState::Pending));
        }
        if let Some(outcome) = self.handle.as_ref().and_then(LookupHandle::ready) {
            if let LookupState::Found(snapshot) = outcome.as_ref()
                && self.key.as_ref() == Some(snapshot.key())
                && self
                    .snapshot
                    .as_ref()
                    .is_none_or(|current| current.compare_lookup(snapshot).is_lt())
            {
                self.snapshot = Some(snapshot.as_ref().clone());
            }
            self.state = Some(outcome);
        }
    }

    /// Freeze this caller's key/deadline for an independently submitted save.
    pub(in crate::ui) fn submission(&self) -> Option<LookupDemand> {
        if self.due.is_none() && self.handle.is_none() {
            return None;
        }
        let key = self.key.clone()?;
        let due = self.due.unwrap_or_else(Instant::now);
        Some(LookupDemand {
            key: key.clone(),
            due,
            handle: self
                .handle
                .clone()
                .or_else(|| self.snapshot.clone().map(LookupHandle::from_snapshot))
                .or_else(|| {
                    self.client
                        .as_ref()
                        .map(|client| client.request_at(key, due))
                }),
        })
    }

    /// Retry failure or rejected authentication without refreshing a successful lookup.
    pub(in crate::ui) fn show(&mut self, ui: &mut egui::Ui) {
        let retry = match self.state.as_deref() {
            Some(LookupState::Pending) => {
                ui.small("Looking up callsign…");
                false
            }
            Some(LookupState::NotFound) => {
                ui.small("Callsign not found in the selected directory.");
                false
            }
            Some(LookupState::PausedAuthentication) => {
                ui.small("Directory credentials were rejected.");
                ui.button("Retry directory login").clicked()
            }
            Some(LookupState::Failed(message)) => {
                ui.small(message);
                ui.button("Retry directory lookup").clicked()
            }
            _ => false,
        };
        if retry && let (Some(client), Some(key)) = (&self.client, &self.key) {
            self.handle = Some(client.retry(key.clone()));
            self.state = Some(Arc::new(LookupState::Pending));
        }
    }
}
