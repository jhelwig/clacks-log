//! Ordinary ADIF semantic projection for QRZ publication and comparison.

use super::{
    adif::{self, AdifFields, ProjectionContext},
    destination::{Destination, Service},
    error::{
        ProjectionDecision, ProjectionResult, ProjectionWarning, ReadinessIssue, ServiceError,
    },
};
use crate::{
    contact::{BandFrequency, Qso},
    effective::{EligibleStationFacts, select_location, select_name},
    export::{OutputPurpose, evaluate::plan_reference_export},
    ids::DestinationId,
    location::RecordedLocation,
    position::Position,
};
use serde::{Deserialize, Serialize};

/// Canonical values submitted to one remote QRZ logbook.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QrzSubmission {
    /// Collection used for this submission.
    pub destination: DestinationId,
    /// All emitted fields, excluding service status/credential metadata.
    pub fields: AdifFields,
}

/// Read-only current accepted inputs for ordinary QRZ output.
pub struct QrzProjectionInput<'a> {
    /// Current accepted contact.
    pub qso: &'a Qso,
    /// Selected remote QRZ logbook.
    pub destination: &'a Destination,
    /// Already source/key/applicability-eligible fallback information.
    pub eligible: EligibleStationFacts<'a>,
    /// Ordinary reference and recognition configuration.
    pub context: &'a ProjectionContext<'a>,
}

/// Add separately recorded pair values without borrowing a missing override frequency.
fn pair(fields: &mut AdifFields, pair: &BandFrequency, rx: bool) {
    if let Some(b) = pair.band() {
        fields.insert(
            if rx { "BAND_RX" } else { "BAND" }.into(),
            b.label().to_ascii_uppercase(),
        );
    }
    if let Some(f) = pair.frequency() {
        fields.insert(
            if rx { "FREQ_RX" } else { "FREQ" }.into(),
            adif::frequency(f),
        );
    }
}

/// Add a complete selected location, preserving area versus measured-point meaning.
fn location(fields: &mut AdifFields, l: &RecordedLocation, own: bool) {
    let key = |base: &str| {
        if own {
            format!("MY_{base}")
        } else {
            base.to_owned()
        }
    };
    if let Some(p) = &l.position {
        let grid = p.display_locator().to_string();
        let base: String = grid.chars().take(8).collect();
        let ext: String = grid.chars().skip(8).collect();
        fields.insert(key("GRIDSQUARE"), base);
        if !ext.is_empty() {
            fields.insert(key("GRIDSQUARE_EXT"), ext);
        }
        if let Position::ExplicitPoint { .. } = p {
            let (lat, lon) = adif::coordinates(p.display_coordinates());
            fields.insert(key("LAT"), lat);
            fields.insert(key("LON"), lon);
        }
    }
    for (field, value) in [
        ("DXCC", &l.dxcc),
        ("STATE", &l.primary_subdivision),
        ("CNTY", &l.secondary_subdivision),
    ] {
        if let Some(v) = value {
            fields.insert(key(field), v.clone());
        }
    }
    if let Some(z) = l.cq_zone {
        fields.insert(
            if own { "MY_CQ_ZONE" } else { "CQZ" }.into(),
            z.get().to_string(),
        );
    }
    if let Some(z) = l.itu_zone {
        fields.insert(
            if own { "MY_ITU_ZONE" } else { "ITUZ" }.into(),
            z.get().to_string(),
        );
    }
}

/// Prepare QRZ values without file expansion or changing recorded facts.
pub fn project(
    input: &QrzProjectionInput<'_>,
) -> anyhow::Result<ProjectionDecision<QrzSubmission>> {
    input.qso.validate()?;
    let result = prepare(input);
    Ok(match result {
        Ok(v) => ProjectionDecision::Ready(v),
        Err(e) => ProjectionDecision::NotReady(vec![ReadinessIssue {
            field: format!(
                "QRZ QSO {} destination {}",
                input.qso.id, input.destination.id
            ),
            reason: e.to_string(),
        }]),
    })
}

/// Prepare provider output after checking the destination namespace.
fn prepare(input: &QrzProjectionInput<'_>) -> anyhow::Result<ProjectionResult<QrzSubmission>> {
    if input.destination.service() != Service::Qrz {
        return Err(ServiceError::Invalid {
            detail: "QRZ destination required".into(),
        }
        .into());
    }
    let result = prepare_fields(input.qso, input.context, &input.eligible, true)?;
    Ok(ProjectionResult {
        value: QrzSubmission {
            destination: input.destination.id,
            fields: result.value,
        },
        warnings: result.warnings,
    })
}

/// Project only supplied historical QSO values without directory or provider fallback.
pub(crate) fn source_fields(
    qso: &Qso,
    context: &ProjectionContext<'_>,
) -> anyhow::Result<AdifFields> {
    Ok(prepare_fields(qso, context, &EligibleStationFacts::default(), false)?.value)
}

/// Emit the supported field set and retain every conversion warning.
fn prepare_fields(
    q: &Qso,
    c: &ProjectionContext<'_>,
    eligible: &EligibleStationFacts<'_>,
    require_submission_time: bool,
) -> anyhow::Result<ProjectionResult<AdifFields>> {
    if c.profiles
        .get(&c.export.output_profile_id)
        .is_none_or(|p| p.purpose != OutputPurpose::OrdinaryInterchange)
    {
        return Err(ServiceError::Invalid {
            detail: "QRZ requires a QRZ destination and ordinary ADIF profile".into(),
        }
        .into());
    }
    let converted_time = adif::contact_time(q.start);
    let converted_time = if require_submission_time {
        Some(converted_time?)
    } else {
        converted_time.ok()
    };
    let mut fields = adif::mode(&q.mode, c.recognition);
    fields.extend([
        (
            "STATION_CALLSIGN".into(),
            q.own_station.callsign.as_str().into(),
        ),
        ("CALL".into(), q.contacted_station.callsign.as_str().into()),
    ]);
    if let Some((date, time)) = converted_time {
        fields.insert("QSO_DATE".into(), date);
        fields.insert("TIME_ON".into(), time);
    }
    pair(&mut fields, &q.primary, false);
    if let Some(rx) = &q.receive_override
        && rx != &q.primary
    {
        pair(&mut fields, rx, true);
    }
    if let Some(end) = q.end {
        let converted = adif::contact_time(end);
        let converted = if require_submission_time {
            Some(converted?)
        } else {
            converted.ok()
        };
        if let Some((date, time)) = converted {
            fields.insert("QSO_DATE_OFF".into(), date);
            fields.insert("TIME_OFF".into(), time);
        }
    }
    for (name, value) in [
        ("RST_SENT", &q.sent_report),
        ("RST_RCVD", &q.received_report),
        ("COMMENT", &q.comment),
    ] {
        if let Some(v) = value {
            fields.insert(name.into(), v.clone());
        }
    }
    if let Some(op) = &q.own_context.operator {
        if let Some(call) = &op.callsign {
            fields.insert("OPERATOR".into(), call.as_str().into());
        }
        if let Some(name) = &op.recorded_name {
            fields.insert("MY_NAME".into(), name.clone());
        }
    }
    if let Some(op) = &q.contacted_operator
        && let Some(call) = &op.callsign
    {
        fields.insert("CONTACTED_OP".into(), call.as_str().into());
    }
    if let Some(name) = select_name(
        q.contacted_operator
            .as_ref()
            .and_then(|o| o.recorded_name.as_deref()),
        eligible,
    ) {
        fields.insert("NAME".into(), name.value);
    }
    if let Some(l) = &q.own_station.location {
        location(&mut fields, l, true);
    }
    if let Some(l) =
        select_location(&q.contacted_station, eligible, c.definitions).and_then(|l| l.value)
    {
        location(&mut fields, &l, false);
    }
    if let Some(power) = q.own_context.transmit_power {
        fields.insert("TX_PWR".into(), adif::power(power));
    }
    if let Some(e) = &q.own_context.equipment {
        if let Some(rig) = &e.equipment_description {
            fields.insert("MY_RIG".into(), rig.clone());
        }
        if let Some(antenna) = &e.antenna_description {
            fields.insert("MY_ANTENNA".into(), antenna.clone());
        }
    }
    let refs = plan_reference_export(
        std::slice::from_ref(q),
        c.export,
        c.definitions,
        c.profiles,
        c.resolutions,
    )?;
    if !refs.decisions.is_empty() {
        return Err(ServiceError::Invalid {
            detail: format!("unresolved QRZ reference choices: {:?}", refs.decisions),
        }
        .into());
    }
    let mut warnings = refs
        .warnings
        .into_iter()
        .map(|w| ProjectionWarning {
            field: "reference".into(),
            detail: format!("{w:?}"),
        })
        .collect::<Vec<_>>();
    for file in refs.files {
        for entry in file.entries {
            for (field, value) in entry.reference_fields {
                let permitted = matches!(
                    field.as_str(),
                    "IOTA"
                        | "MY_IOTA"
                        | "POTA_REF"
                        | "MY_POTA_REF"
                        | "SOTA_REF"
                        | "MY_SOTA_REF"
                        | "WWFF_REF"
                        | "MY_WWFF_REF"
                        | "SIG"
                        | "SIG_INFO"
                        | "MY_SIG"
                        | "MY_SIG_INFO"
                        | "CONTEST_ID"
                );
                if permitted {
                    if fields
                        .insert(field.clone(), value.clone())
                        .is_some_and(|old| old != value)
                    {
                        return Err(ServiceError::Invalid {
                            detail: format!("QRZ conflicting mapped field {field}"),
                        }
                        .into());
                    }
                } else {
                    warnings.push(ProjectionWarning {
                        field,
                        detail: "unsupported service reference field omitted".into(),
                    });
                }
            }
        }
    }
    let mut emitted = AdifFields::new();
    for (field, value) in fields {
        let text = adif::text(&field, &value);
        warnings.extend(text.warnings);
        if let Some(value) = text.value {
            emitted.insert(field, value);
        }
    }
    for name in ["STATION_CALLSIGN", "CALL", "QSO_DATE", "TIME_ON"] {
        if !emitted.contains_key(name)
            && (require_submission_time || !matches!(name, "QSO_DATE" | "TIME_ON"))
        {
            return Err(ServiceError::Invalid {
                detail: format!("QRZ required field {name} cannot be represented"),
            }
            .into());
        }
    }
    Ok(ProjectionResult {
        value: emitted,
        warnings,
    })
}
