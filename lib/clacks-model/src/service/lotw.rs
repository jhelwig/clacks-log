//! Pinned, data-driven LoTW normalization shared by comparison and signing.

use super::{
    adif::{self, AdifFields},
    destination::{Destination, ExternalRecordKey, LotwRecordKey, RemoteRecordId, Service},
    error::{ProjectionDecision, ProjectionResult, ReadinessIssue, ServiceError},
};
use crate::{
    contact::{BandFrequency, Qso},
    group::DefinitionSet,
    recognition::mode::{Mode, ModeIdentity},
    scalar::RecordedCallsign,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Service-compatible band range in whole hertz.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LotwBand {
    /// Concrete service band label.
    pub label: String,
    /// Inclusive lower frequency.
    pub lower_hz: u64,
    /// Inclusive upper frequency.
    pub upper_hz: u64,
}

/// One station-field enumeration, optionally dependent on another field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationValue {
    /// Required parent value, such as a state for a county.
    pub dependency: Option<String>,
    /// Serialized signing value.
    pub value: String,
    /// Alternate human-readable supplied label.
    pub label: String,
}

/// Configuration-defined station validation metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationField {
    /// Maximum supported characters; zero means unspecified.
    pub length: usize,
    /// Optional parent field identity.
    pub depends_on: Option<String>,
    /// Allowed supplied values, empty for non-enumerated fields.
    pub values: Vec<StationValue>,
}

/// Preserved public normalization data; updating it does not reinterpret old attempts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LotwConfiguration {
    /// Upstream configuration release.
    pub release: String,
    /// Source Git identity.
    pub upstream_commit: String,
    /// Uppercase combined/bare input labels to concrete modes.
    pub mapping: BTreeMap<String, String>,
    /// Concrete modes and their award groups.
    pub modes: BTreeMap<String, String>,
    /// Ordered service bands from the upstream definitions.
    pub bands: Vec<LotwBand>,
    /// Station field validation definitions.
    pub fields: BTreeMap<String, StationField>,
    /// Applicable fields indexed by entity identifier.
    pub entity_fields: BTreeMap<String, Vec<String>>,
    /// Fields emitted by the pinned signing specification.
    pub station_fields: Vec<String>,
}

impl LotwConfiguration {
    /// Load a supplied configuration and reject unsupported mappings/ranges.
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        let c: Self = serde_json::from_str(json)?;
        if c.release.is_empty()
            || c.bands.is_empty()
            || c.mapping.is_empty()
            || c.mapping
                .iter()
                .any(|(k, v)| k.trim().is_empty() || !c.modes.contains_key(v))
            || c.bands
                .iter()
                .any(|b| b.label.is_empty() || b.lower_hz == 0 || b.upper_hz < b.lower_hz)
        {
            return Err(ServiceError::Invalid {
                detail: "invalid LoTW normalization configuration".into(),
            }
            .into());
        }
        Ok(c)
    }

    /// Load the offline pinned compatibility configuration, returning malformed data errors.
    pub fn bundled() -> anyhow::Result<Self> {
        Self::from_json(include_str!("../../data/lotw-11.34.json"))
    }

    /// Classify a supplied pair using service definitions, keeping band-only pairs frequency-free.
    pub fn band(&self, pair: &BandFrequency) -> anyhow::Result<String> {
        let found = if let Some(hz) = pair.frequency() {
            self.bands
                .iter()
                .find(|b| (b.lower_hz..=b.upper_hz).contains(&hz.get()))
        } else {
            pair.band().and_then(|b| {
                self.bands
                    .iter()
                    .find(|v| v.label.eq_ignore_ascii_case(b.label()))
            })
        };
        found.map(|b| b.label.clone()).ok_or_else(|| {
            ServiceError::Invalid {
                detail: format!("LoTW band unavailable for {pair:?}"),
            }
            .into()
        })
    }
}

/// Public certificate identity used to prepare a submission without cryptography dependencies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicSigningIdentity {
    /// Certificate-authorized station callsign.
    pub callsign: RecordedCallsign,
    /// Entity actually used for signing.
    pub dxcc: String,
}

/// One normalized semantic signing input; signatures and mapping provenance are separate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LotwSubmission {
    /// Remote collection namespace.
    pub destination: crate::ids::DestinationId,
    /// Actual own identity selected for signing.
    pub signing_identity: PublicSigningIdentity,
    /// Contact fields at their emitted precision.
    pub contact_fields: AdifFields,
    /// All emitted station fields.
    pub station_fields: AdifFields,
    /// Exact remote identity frozen with these values.
    pub key: LotwRecordKey,
}

impl LotwSubmission {
    /// Identify this prepared remote record without recomputing mode under newer mappings.
    pub fn external_key(&self) -> ExternalRecordKey {
        ExternalRecordKey {
            destination: self.destination,
            record: RemoteRecordId::Lotw(self.key.clone()),
        }
    }
}

/// Read-only inputs to prepare one real LoTW signing value.
pub struct LotwProjectionInput<'a> {
    /// Accepted contact or prepared historical import.
    pub qso: &'a Qso,
    /// Established LoTW collection.
    pub destination: &'a Destination,
    /// Eligible public signing identity.
    pub signing: &'a PublicSigningIdentity,
    /// Explicit service normalization definitions.
    pub configuration: &'a LotwConfiguration,
    /// Account group definitions for explicit own reference mappings.
    pub definitions: &'a DefinitionSet,
}

/// Normalize native mode labels using upstream combined/submode/parent fallback.
pub fn normalize_mode(mode: &Mode, config: &LotwConfiguration) -> anyhow::Result<String> {
    let (parent, sub) = match mode.identity() {
        ModeIdentity::Parent { label } => (label.to_uppercase(), None),
        ModeIdentity::Specialization { label, parent } => (
            parent.unwrap_or_else(|| label.clone()).to_uppercase(),
            Some(label.to_uppercase()),
        ),
    };
    let mut keys = Vec::new();
    if let Some(sub) = sub {
        keys.push(format!("{parent}%{sub}"));
        keys.push(sub);
    }
    keys.push(parent.clone());
    let result = keys
        .iter()
        .find_map(|k| config.mapping.get(k))
        .cloned()
        .unwrap_or(parent);
    if !config.modes.contains_key(&result) {
        return Err(ServiceError::Invalid {
            detail: format!("unsupported LoTW mode {result}"),
        }
        .into());
    }
    Ok(result)
}

/// Project a complete QSO, returning service readiness instead of altering native data.
pub fn project(
    input: &LotwProjectionInput<'_>,
) -> anyhow::Result<ProjectionDecision<LotwSubmission>> {
    input.qso.validate()?;
    let result = prepare(input);
    Ok(match result {
        Ok(value) => ProjectionDecision::Ready(ProjectionResult {
            value,
            warnings: Vec::new(),
        }),
        Err(e) => ProjectionDecision::NotReady(vec![ReadinessIssue {
            field: format!(
                "LoTW QSO {} destination {}",
                input.qso.id, input.destination.id
            ),
            reason: e.to_string(),
        }]),
    })
}

/// Build fields after native validation; conversion errors become contextual readiness.
fn prepare(input: &LotwProjectionInput<'_>) -> anyhow::Result<LotwSubmission> {
    let q = input.qso;
    let c = input.configuration;
    if input.destination.service() != Service::Lotw
        || input.signing.callsign != q.own_station.callsign
        || input.signing.dxcc.trim().is_empty()
    {
        return Err(ServiceError::Invalid {
            detail: "LoTW destination or public signing identity mismatch".into(),
        }
        .into());
    }
    if let Some(entity) = q
        .own_station
        .location
        .as_ref()
        .and_then(|l| l.dxcc.as_ref())
        && entity != &input.signing.dxcc
    {
        return Err(ServiceError::Invalid {
            detail: format!(
                "recorded entity {entity} differs from certificate {}",
                input.signing.dxcc
            ),
        }
        .into());
    }
    let band = c.band(&q.primary)?;
    let rx = q.primary.effective_receive(q.receive_override.as_ref());
    let receive_band = c.band(rx)?;
    let mode = normalize_mode(&q.mode, c)?;
    let key = LotwRecordKey {
        own_call: q.own_station.callsign.clone(),
        worked_call: q.contacted_station.callsign.clone(),
        start: q.start,
        band: band.clone(),
        receive_band: receive_band.clone(),
        mode: mode.clone(),
    };
    let (date, time) = adif::contact_time(q.start)?;
    let mut fields = AdifFields::from([
        ("CALL".into(), q.contacted_station.callsign.as_str().into()),
        ("BAND".into(), band.clone()),
        ("MODE".into(), mode),
        ("QSO_DATE".into(), date),
        ("QSO_TIME".into(), time),
    ]);
    if let Some(f) = q.primary.frequency() {
        fields.insert("FREQ".into(), adif::frequency(f));
    }
    if rx != &q.primary {
        if receive_band != band {
            fields.insert("BAND_RX".into(), receive_band);
        }
        if let Some(f) = rx.frequency()
            && Some(f) != q.primary.frequency()
        {
            fields.insert("FREQ_RX".into(), adif::frequency(f));
        }
    }
    let mut station = AdifFields::new();
    if let Some(l) = &q.own_station.location {
        if let Some(p) = &l.position {
            station.insert("GRIDSQUARE".into(), p.display_locator().to_string());
        }
        if let Some(z) = l.cq_zone {
            station.insert("CQZ".into(), z.get().to_string());
        }
        if let Some(z) = l.itu_zone {
            station.insert("ITUZ".into(), z.get().to_string());
        }
        if let Some(applicable) = c.entity_fields.get(&input.signing.dxcc) {
            for (value, secondary) in [
                (&l.primary_subdivision, false),
                (&l.secondary_subdivision, true),
            ] {
                if let Some(value) = value {
                    let found = applicable
                        .iter()
                        .filter_map(|name| c.fields.get(name).map(|f| (name, f)))
                        .filter(|(_, f)| f.depends_on.is_some() == secondary)
                        .find_map(|(name, f)| {
                            let parent = f.depends_on.as_ref().and_then(|n| station.get(n));
                            f.values
                                .iter()
                                .find(|v| {
                                    v.dependency.as_ref() == parent
                                        && (v.value.eq_ignore_ascii_case(value)
                                            || v.label.eq_ignore_ascii_case(value)
                                            || value.split_once(',').is_some_and(|(_, tail)| {
                                                v.value.eq_ignore_ascii_case(tail)
                                            }))
                                })
                                .map(|v| (name.clone(), v.value.clone()))
                        });
                    if let Some((name, value)) = found {
                        station.insert(name, value);
                    } else {
                        return Err(ServiceError::Invalid {
                            detail: format!(
                                "unsupported LoTW subdivision {value} for entity {}",
                                input.signing.dxcc
                            ),
                        }
                        .into());
                    }
                }
            }
        } else if l.primary_subdivision.is_some() || l.secondary_subdivision.is_some() {
            return Err(ServiceError::Invalid {
                detail: format!(
                    "no station subdivision mapping for entity {}",
                    input.signing.dxcc
                ),
            }
            .into());
        }
    }
    if let Some(value) = own_iota(q, input.definitions)? {
        station.insert("IOTA".into(), value);
    }
    for (field, value) in &station {
        if let Some(meta) = c.fields.get(field)
            && meta.length > 0
            && value.chars().count() > meta.length
        {
            return Err(ServiceError::Invalid {
                detail: format!(
                    "LoTW station field {field} exceeds {} characters",
                    meta.length
                ),
            }
            .into());
        }
    }
    Ok(LotwSubmission {
        destination: input.destination.id,
        signing_identity: input.signing.clone(),
        contact_fields: fields,
        station_fields: station,
        key,
    })
}

/// Evaluate explicitly mapped own IOTA for both imported baselines and real signing inputs.
pub(crate) fn own_iota(q: &Qso, definitions: &DefinitionSet) -> anyhow::Result<Option<String>> {
    let mut station = AdifFields::new();
    let groups = q
        .own_station
        .references
        .iter()
        .map(|r| r.group_id)
        .collect::<BTreeSet<_>>();
    for id in groups {
        if let Some(def) = definitions.get(id) {
            let compiled = crate::export::rules::compile_definition(def)?;
            let bindings = BTreeMap::new();
            let context = crate::export::rules::ExpressionContext {
                qso: Some(q),
                current_identifier: None,
                bindings: &bindings,
            };
            for (name, value) in crate::export::select::evaluate_fields(
                &compiled,
                &def.export_mappings.ordinary_interchange,
                &context,
            )? {
                if name == "MY_IOTA"
                    && station
                        .insert("IOTA".into(), value.clone())
                        .is_some_and(|old| old != value)
                {
                    return Err(ServiceError::Invalid {
                        detail: "multiple own IOTA signing values".into(),
                    }
                    .into());
                }
            }
        }
    }
    Ok(station.remove("IOTA"))
}
