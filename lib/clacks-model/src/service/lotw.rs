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

/// Pinned TrustedQSL input flag requiring uppercase station text.
const UPPERCASE_INPUT: u32 = 1;

/// One allowed ITU/CQ zone pairing from the pinned entity/subdivision data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZonePair {
    /// International Telecommunication Union zone number.
    pub itu: u8,
    /// CQ zone number paired with this ITU zone.
    pub cq: u8,
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
    /// Optional narrower zone restrictions for this subdivision.
    pub zones: Vec<ZonePair>,
}

/// Configuration-defined station validation metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationField {
    /// Upstream input flags, including mask 1 for uppercasing.
    pub flags: u32,
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
    #[serde(deserialize_with = "unique_map")]
    pub mapping: BTreeMap<String, String>,
    /// Concrete modes and their award groups.
    pub modes: BTreeMap<String, String>,
    /// Ordered service bands from the upstream definitions.
    pub bands: Vec<LotwBand>,
    /// Station field validation definitions.
    pub fields: BTreeMap<String, StationField>,
    /// Source-defined primary administrative fields.
    pub primary_fields: Vec<String>,
    /// Supported secondary administrative field bindings.
    pub secondary_fields: Vec<String>,
    /// Entity-level allowed ITU/CQ pairs.
    pub entity_zones: BTreeMap<String, Vec<ZonePair>>,
    /// Applicable fields indexed by entity identifier.
    pub entity_fields: BTreeMap<String, Vec<String>>,
    /// Fields emitted by the pinned signing specification.
    pub station_fields: Vec<String>,
}

impl LotwConfiguration {
    /// Load a supplied configuration and reject unsupported mappings/ranges.
    #[tracing::instrument(skip_all, name = "model.lotw_configuration.load", level = "debug")]
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        let c: Self = serde_json::from_str(json)?;
        if c.release.is_empty()
            || c.bands.is_empty()
            || c.mapping.is_empty()
            || c.mapping
                .keys()
                .map(|k| k.to_uppercase())
                .collect::<BTreeSet<_>>()
                .len()
                != c.mapping.len()
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
            tracing::debug!(
                event = "projection.conversion_unavailable",
                reason = "unmapped_band"
            );
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

/// Public source identity for the definitions used to normalize a historical submission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LotwMappingProvenance {
    /// TrustedQSL configuration release used for the mapping.
    pub release: String,
    /// Upstream source revision from which compatibility data was derived.
    pub upstream_commit: String,
}

/// Prepared signing values and their interpretation source, without signature bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LotwSubmission {
    /// Frozen mapping source; retained for interpretation rather than publication equality.
    pub mapping_provenance: LotwMappingProvenance,
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
        tracing::debug!(
            event = "projection.conversion_unavailable",
            reason = "unmapped_mode"
        );
        return Err(ServiceError::Invalid {
            detail: format!("unsupported LoTW mode {result}"),
        }
        .into());
    }
    Ok(result)
}

/// Project a complete QSO, returning service readiness instead of altering native data.
#[tracing::instrument(
    skip_all,
    name = "model.lotw.project",
    level = "debug",
    fields(
        qso_id = %input.qso.id,
        destination_id = %input.destination.id,
    ),
)]
pub fn project(
    input: &LotwProjectionInput<'_>,
) -> anyhow::Result<ProjectionDecision<LotwSubmission>> {
    input.qso.validate()?;
    let result = prepare(input);
    let decision = match result {
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
    };
    let (readiness, warning_count, readiness_issue_count) = match &decision {
        ProjectionDecision::Ready(result) => ("ready", result.warnings.len(), 0),
        ProjectionDecision::NotReady(issues) => ("not_ready", 0, issues.len()),
    };
    metrics::counter!("clacks_service_projection_readiness_total", "service" => "lotw", "readiness" => readiness).increment(1);
    tracing::debug!(
        event = "projection.prepared",
        qso_id = %input.qso.id,
        destination_id = %input.destination.id,
        readiness,
        warning_count,
        readiness_issue_count,
    );
    Ok(decision)
}

/// Build fields after native validation; conversion errors become contextual readiness.
fn prepare(input: &LotwProjectionInput<'_>) -> anyhow::Result<LotwSubmission> {
    let q = input.qso;
    let c = input.configuration;
    if input.destination.service() != Service::Lotw
        || input.signing.callsign != q.own_station.callsign
        || input.signing.dxcc.trim().is_empty()
    {
        tracing::debug!(
            event = "projection.conversion_unavailable",
            reason = "destination_or_signing_identity_mismatch"
        );
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
        tracing::debug!(
            event = "projection.conversion_unavailable",
            reason = "signing_entity_mismatch"
        );
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
    let mut station = location_fields(q.own_station.location.as_ref(), &input.signing.dxcc, c)?;
    if let Some(value) = own_iota(q, input.definitions)? {
        station.insert("IOTA".into(), value);
    }
    for (field, value) in &mut station {
        *value = station_value(field, value, c);
    }
    for (field, value) in &station {
        if let Some(meta) = c.fields.get(field)
            && meta.length > 0
            && value.chars().count() > meta.length
        {
            tracing::debug!(
                event = "projection.conversion_unavailable",
                reason = "station_field_too_long"
            );
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
        mapping_provenance: LotwMappingProvenance {
            release: c.release.clone(),
            upstream_commit: c.upstream_commit.clone(),
        },
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
                    tracing::debug!(
                        event = "projection.conversion_unavailable",
                        reason = "ambiguous_own_iota"
                    );
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

/// Apply the pinned field's input transformation for shared source/signing comparisons.
pub(crate) fn station_value(field: &str, value: &str, configuration: &LotwConfiguration) -> String {
    if configuration
        .fields
        .get(field)
        .is_some_and(|meta| meta.flags & UPPERCASE_INPUT != 0)
    {
        value.to_ascii_uppercase()
    } else {
        value.to_owned()
    }
}

/// Deserialize map rows without silently replacing a duplicate normalization key.
fn unique_map<'de, D, V>(deserializer: D) -> Result<BTreeMap<String, V>, D::Error>
where
    D: serde::Deserializer<'de>,
    V: Deserialize<'de>,
{
    deserializer.deserialize_map(UniqueMap(std::marker::PhantomData))
}

/// Value-type marker for a normalization map with unique source keys.
struct UniqueMap<V>(
    /// Deserialized value type, without storing an extra runtime value.
    std::marker::PhantomData<V>,
);

impl<'de, V: Deserialize<'de>> serde::de::Visitor<'de> for UniqueMap<V> {
    type Value = BTreeMap<String, V>;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("normalization map with unique keys")
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(
        self,
        mut access: A,
    ) -> Result<Self::Value, A::Error> {
        let mut result = BTreeMap::new();
        while let Some((key, value)) = access.next_entry::<String, V>()? {
            if result.insert(key.clone(), value).is_some() {
                return Err(serde::de::Error::custom(format!(
                    "duplicate normalization key {key}"
                )));
            }
        }
        Ok(result)
    }
}

/// Prepare supported location fields without importing certificate defaults into recorded data.
fn location_fields(
    location: Option<&crate::location::RecordedLocation>,
    entity: &str,
    c: &LotwConfiguration,
) -> anyhow::Result<AdifFields> {
    let mut station = AdifFields::new();
    let mut subdivision_zones = Vec::new();
    if let Some(l) = location {
        if let Some(p) = &l.position {
            station.insert("GRIDSQUARE".into(), p.display_locator().to_string());
        }
        if let Some(z) = l.cq_zone {
            station.insert("CQZ".into(), z.get().to_string());
        }
        if let Some(z) = l.itu_zone {
            station.insert("ITUZ".into(), z.get().to_string());
        }
        if let Some(applicable) = c.entity_fields.get(entity) {
            for (value, secondary) in [
                (&l.primary_subdivision, false),
                (&l.secondary_subdivision, true),
            ] {
                if let Some(value) = value {
                    let found = applicable
                        .iter()
                        .filter_map(|name| c.fields.get(name).map(|f| (name, f)))
                        .filter(|(name, _)| {
                            if secondary {
                                c.secondary_fields.contains(name)
                            } else {
                                c.primary_fields.contains(name)
                            }
                        })
                        .find_map(|(name, f)| {
                            let parent = f.depends_on.as_ref().and_then(|n| {
                                if n == "DXCC" {
                                    Some(entity)
                                } else {
                                    station.get(n).map(String::as_str)
                                }
                            });
                            let supplied = if secondary {
                                if let Some((prefix, tail)) = value.split_once(',') {
                                    if parent.is_some_and(|p| p.eq_ignore_ascii_case(prefix)) {
                                        Some(tail)
                                    } else {
                                        None
                                    }
                                } else {
                                    Some(value.as_str())
                                }
                            } else {
                                Some(value.as_str())
                            };
                            f.values
                                .iter()
                                .find(|v| {
                                    v.dependency.as_deref() == parent
                                        && supplied.is_some_and(|value| {
                                            v.value.eq_ignore_ascii_case(value)
                                                || v.label.eq_ignore_ascii_case(value)
                                        })
                                })
                                .map(|v| (name.clone(), v.value.clone(), v.zones.clone()))
                        });
                    if let Some((name, value, zones)) = found {
                        if !secondary && !zones.is_empty() {
                            subdivision_zones = zones;
                        }
                        station.insert(name, value);
                    } else {
                        tracing::debug!(
                            event = "projection.conversion_unavailable",
                            reason = "unsupported_subdivision"
                        );
                        return Err(ServiceError::Invalid {
                            detail: format!(
                                "unsupported LoTW subdivision {value} for entity {}",
                                entity
                            ),
                        }
                        .into());
                    }
                }
            }
        } else if l.primary_subdivision.is_some() || l.secondary_subdivision.is_some() {
            tracing::debug!(
                event = "projection.conversion_unavailable",
                reason = "missing_subdivision_mapping"
            );
            return Err(ServiceError::Invalid {
                detail: format!("no station subdivision mapping for entity {}", entity),
            }
            .into());
        }
    }
    if let Some(l) = location {
        for (scope, zones) in [
            (
                "entity",
                c.entity_zones.get(entity).map(Vec::as_slice).unwrap_or(&[]),
            ),
            ("subdivision", subdivision_zones.as_slice()),
        ] {
            if !zones.is_empty()
                && !zones.iter().any(|p| {
                    l.cq_zone.is_none_or(|z| z.get() == p.cq)
                        && l.itu_zone.is_none_or(|z| z.get() == p.itu)
                })
            {
                tracing::debug!(
                    event = "projection.conversion_unavailable",
                    reason = "zone_pair_mismatch"
                );
                return Err(ServiceError::Invalid {
                    detail: format!("invalid CQ/ITU zone pair for LoTW {scope} {}", entity),
                }
                .into());
            }
        }
    }
    Ok(station)
}

/// Capture source-only location comparisons at the precision and vocabulary LoTW emits.
pub(crate) fn location_baseline(
    location: Option<&crate::location::RecordedLocation>,
    configuration: &LotwConfiguration,
) -> Option<crate::location::RecordedLocation> {
    let mut location = location?.clone();
    location.position = location
        .position
        .as_ref()
        .map(|p| crate::position::Position::GridArea(p.display_locator()));
    if let Some(entity) = &location.dxcc
        && let Ok(fields) = location_fields(Some(&location), entity, configuration)
    {
        location.primary_subdivision = configuration
            .primary_fields
            .iter()
            .find_map(|name| fields.get(name).cloned());
        location.secondary_subdivision = configuration
            .secondary_fields
            .iter()
            .find_map(|name| fields.get(name).cloned());
    }
    location.normalized()
}
