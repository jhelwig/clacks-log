//! Pending operating edits and separately saved reusable-record forms.

use super::{FieldError, equipment::EquipmentForm, issue, optional, power, station::StationForm};
use clacks_client_store::{operating::OperatingState, settings::SettingsSnapshot};
use clacks_model::{
    contact::OperatorInformation,
    group::DefinitionSet,
    ids::{EquipmentConfigurationId, ProfileId},
    profile::{EquipmentConfiguration, SavedLocationProfile},
    scalar::RecordedCallsign,
};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordState},
};
use std::collections::BTreeSet;

/// One transient operating edit with independent station and equipment selections.
#[derive(Clone, Debug)]
pub(crate) struct OperatingEditor {
    /// Applied read basis used by the transactional stale-save guard.
    pub expected: OperatingState,
    /// Editable station facts, not a saved profile.
    pub station: StationForm,
    /// Editable copied equipment facts.
    pub equipment: EquipmentForm,
    /// Independently optional recorded operator callsign text.
    pub operator_callsign: String,
    /// Independently optional recorded operator name.
    pub operator_name: String,
    /// Editable decimal watts with milliwatt resolution.
    pub power_watts: String,
    /// Exact selected station baseline; removed by a manual station change.
    pub selected_profile: Option<SavedLocationProfile>,
    /// Exact selected equipment baseline; removed by a manual equipment change.
    pub selected_equipment: Option<EquipmentConfiguration>,
}

impl OperatingEditor {
    /// Start editing applied values without saving or selecting anything implicitly.
    pub fn new(state: &OperatingState) -> Self {
        let current = state.current();
        let operator = current.and_then(|v| v.operator.as_ref());
        Self {
            expected: state.clone(),
            station: current
                .map(|v| StationForm::from_snapshot(&v.station))
                .unwrap_or_else(StationForm::empty),
            equipment: EquipmentForm::from_values(current.and_then(|v| v.equipment.as_ref())),
            operator_callsign: operator
                .and_then(|o| o.callsign().as_ref())
                .map(|call| call.as_str().to_owned())
                .unwrap_or_default(),
            operator_name: operator
                .and_then(|o| o.recorded_name().clone())
                .unwrap_or_default(),
            power_watts: power::format_watts(current.and_then(|v| v.transmit_power)),
            selected_profile: state.selected_profile().cloned(),
            selected_equipment: state.selected_equipment().cloned(),
        }
    }

    /// Copy a complete selected profile, preserving operator, power and absent-default equipment.
    pub fn stage_profile(
        &mut self,
        profile: &SavedLocationProfile,
        equipment: Option<&EquipmentConfiguration>,
    ) -> anyhow::Result<()> {
        let equipment = if let Some(id) = profile.default_equipment() {
            let equipment =
                equipment.ok_or(clacks_model::contact::RecordError::MissingEquipment { id })?;
            anyhow::ensure!(
                equipment.id() == id,
                clacks_model::contact::RecordError::EquipmentIdMismatch {
                    expected: id,
                    supplied: equipment.id()
                }
            );

            Some(equipment)
        } else {
            None
        };
        self.station = StationForm::from_snapshot(profile.station());
        self.selected_profile = Some(profile.clone());
        if let Some(equipment) = equipment {
            self.stage_equipment(equipment);
        }
        Ok(())
    }

    /// Explicitly select copied equipment while retaining every station/operator/power edit.
    pub fn stage_equipment(&mut self, equipment: &EquipmentConfiguration) {
        self.equipment = EquipmentForm::from_values(Some(equipment.values()));
        self.selected_equipment = Some(equipment.clone());
    }

    /// Detach after a user-originated station value change, including incomplete invalid input.
    pub fn station_changed(&mut self, previous: &StationForm, definitions: &DefinitionSet) {
        if self.station == *previous {
            return;
        }
        let equivalent = match (previous.build(definitions), self.station.build(definitions)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        };
        if !equivalent {
            self.selected_profile = None;
        }
    }

    /// Detach after a user-originated equipment change, without touching station selection.
    pub fn equipment_changed(&mut self, previous: &EquipmentForm) {
        if self.equipment != *previous && self.equipment.build() != previous.build() {
            self.selected_equipment = None;
        }
    }

    /// Construct one complete native operating context, returning field-local validation failures.
    pub fn build(&self, definitions: &DefinitionSet) -> Result<OperatingState, Vec<FieldError>> {
        let station = self.station.build(definitions)?;
        let mut equipment = self.equipment.build()?;
        if equipment.is_none()
            && let Some(selected) = &self.selected_equipment
        {
            equipment = Some(selected.values().clone());
        }
        let callsign = if self.operator_callsign.trim().is_empty() {
            None
        } else {
            Some(
                RecordedCallsign::new(&self.operator_callsign)
                    .map_err(|e| vec![issue("Operator callsign", e)])?,
            )
        };
        let recorded_name = optional(&self.operator_name);
        let operator = if callsign.is_some() || recorded_name.is_some() {
            Some(clacks_model::contact::OperatorInformationFields {
                callsign,
                recorded_name,
            })
        } else {
            None
        };
        let transmit_power =
            power::parse_watts(&self.power_watts).map_err(|e| vec![issue("Transmit power", e)])?;
        let value = clacks_model::profile::CurrentOperatingValues {
            station,
            equipment,
            operator: operator
                .map(OperatorInformation::try_from)
                .transpose()
                .map_err(|e| vec![issue("Operator", e)])?,
            transmit_power,
        };

        OperatingState::new(
            Some(value),
            self.selected_profile.clone(),
            self.selected_equipment.clone(),
        )
        .map_err(|error| vec![issue("Operating sources", error)])
    }

    /// Clear a known deleted source association without changing copied facts or edit basis.
    pub fn forget_source(&mut self, id: RecordId) {
        if self
            .selected_profile
            .as_ref()
            .is_some_and(|p| id == RecordId::Profile(p.id()))
        {
            self.selected_profile = None;
        }
        if self
            .selected_equipment
            .as_ref()
            .is_some_and(|e| id == RecordId::Equipment(e.id()))
        {
            self.selected_equipment = None;
        }
    }

    /// Forget proven deletions while preserving pending text and the ordinary stale-save guard.
    pub fn refresh_sources(&mut self, snapshot: &SettingsSnapshot) {
        let deleted: BTreeSet<_> = snapshot
            .profiles
            .iter()
            .chain(&snapshot.equipment)
            .filter(|r| matches!(r.working_state(), Some(RecordState::Deleted)))
            .map(|r| r.key().id)
            .collect();
        for id in &deleted {
            self.forget_source(*id);
        }
        let mut cleaned = self.expected.clone();
        for id in &deleted {
            cleaned.forget_source(*id);
        }
        if cleaned == snapshot.operating {
            self.expected = snapshot.operating.clone();
        }
    }
}

/// A separate station-profile save, with explicit equipment association and edit basis.
#[derive(Clone, Debug)]
pub(crate) struct ProfileEditor {
    /// Stable target identity; new profiles receive a fresh ID once on opening.
    pub id: ProfileId,
    /// Saved-record basis, absent for creation.
    pub expected: Option<ClientRecord>,
    /// Editable reusable display name.
    pub name: String,
    /// Station values owned by the profile.
    pub station: StationForm,
    /// Explicit saved equipment default, independent of operating equipment.
    pub default_equipment: Option<EquipmentConfigurationId>,
}

impl ProfileEditor {
    /// Open a new or existing profile with station text supplied by its caller.
    pub fn new(expected: Option<ClientRecord>, station: StationForm) -> Self {
        let source = expected.as_ref().and_then(|r| match r.working_state() {
            Some(RecordState::Live(clacks_sync::record::RecordValue::Profile(p))) => {
                Some(p.as_ref())
            }
            _ => None,
        });
        let id = source.map(|p| p.id()).unwrap_or_default();
        let name = source.map(|p| p.name().to_owned()).unwrap_or_default();
        let default_equipment = source.and_then(|p| p.default_equipment());
        Self {
            id,
            expected,
            name,
            station,
            default_equipment,
        }
    }

    /// Validate only profile-owned fields, without applying the caller's operating edits.
    pub fn build(
        &self,
        definitions: &DefinitionSet,
    ) -> Result<SavedLocationProfile, Vec<FieldError>> {
        if self.name.trim().is_empty() {
            return Err(vec![issue("Profile name", "Enter a profile name")]);
        }
        (clacks_model::profile::SavedLocationProfileFields {
            id: self.id,
            name: self.name.clone(),
            station: (self.station.build(definitions)?),
            default_equipment: self.default_equipment,
        })
        .try_into()
        .map_err(|e| vec![issue("Profile", e)])
    }
}

/// A separate equipment save with its own target and copied editable descriptions.
#[derive(Clone, Debug)]
pub(crate) struct EquipmentEditor {
    /// Stable reusable target identity.
    pub id: EquipmentConfigurationId,
    /// Saved-record basis, absent for creation.
    pub expected: Option<ClientRecord>,
    /// Editable configuration display name.
    pub name: String,
    /// Equipment facts owned by the configuration.
    pub equipment: EquipmentForm,
}

impl EquipmentEditor {
    /// Open a reusable equipment editor without selecting its result for the caller.
    pub fn new(expected: Option<ClientRecord>, equipment: EquipmentForm) -> Self {
        let source = expected.as_ref().and_then(|r| match r.working_state() {
            Some(RecordState::Live(clacks_sync::record::RecordValue::Equipment(e))) => {
                Some(e.as_ref())
            }
            _ => None,
        });
        let id = source.map(|e| e.id()).unwrap_or_default();
        let name = source.map(|e| e.name().to_owned()).unwrap_or_default();
        Self {
            id,
            expected,
            name,
            equipment,
        }
    }

    /// Validate configuration-owned values independently of any station or operating power.
    pub fn build(&self) -> Result<EquipmentConfiguration, Vec<FieldError>> {
        if self.name.trim().is_empty() {
            return Err(vec![issue(
                "Configuration name",
                "Enter a configuration name",
            )]);
        }
        (clacks_model::profile::EquipmentConfigurationFields {
            id: self.id,
            name: self.name.clone(),
            values: (self.equipment.build()?.unwrap_or_default()).into_fields(),
        })
        .try_into()
        .map_err(|e| vec![issue("Equipment configuration", e)])
    }
}
