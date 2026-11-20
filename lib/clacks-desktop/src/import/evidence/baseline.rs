//! Record which source values imported service claims cover.

use super::{Edit, LotwConfiguration, ProjectionContext, RowProjection, Service};
use crate::{
    import::mapping::{IOTA_GROUP_ID, POTA_GROUP_ID, SOTA_GROUP_ID},
    review::fields::QsoField,
};
use clacks_model::{
    contact::{BandFrequency, Qso},
    ids::{GroupId, QsoId},
    location::{RecordedLocation, RecordedLocationFields},
    reference::StationSide,
    service::{
        adif::{self, AdifFields},
        evidence::{
            ImportedBaseline, ImportedBaselineInput, imported_baseline, observation::ModeReport,
            submission::ConfirmationBasis,
        },
        lotw::normalize_mode,
        qrz::write_location_fields,
    },
};
use std::collections::{BTreeMap, BTreeSet};

/// Record the selected source values in the form the service compares.
/// Use the available fields if the source cannot form a complete QSO.
pub(super) fn capture(
    row: &RowProjection,
    qso: QsoId,
    service: Service,
    coverage: BTreeSet<String>,
    lotw: &LotwConfiguration,
    context: &ProjectionContext<'_>,
) -> anyhow::Result<ImportedBaseline> {
    let projected = row
        .create(qso, None)
        .and_then(Qso::try_from)
        .and_then(|qso| {
            imported_baseline(&ImportedBaselineInput {
                qso: &qso,
                service,
                coverage: coverage.clone(),
                lotw,
                context,
            })
        });
    let mut baseline = projected.or_else(|_| sparse(row, service, coverage, lotw, context))?;
    if service == Service::Lotw {
        let iota = IOTA_GROUP_ID.parse::<GroupId>()?;
        if let Some(value) = row.edits.iter().find_map(|edit| match edit {
            Edit::Reference(StationSide::Own, reference) if reference.group_id == iota => {
                reference.identifier().as_ref()
            }
            _ => None,
        }) {
            baseline.fields.insert("MY_IOTA".into(), value.clone());
        }
    }
    Ok(baseline)
}

/// Build the comparison values that are available when the source cannot form a QSO.
fn sparse(
    row: &RowProjection,
    service: Service,
    coverage: BTreeSet<String>,
    lotw: &LotwConfiguration,
    context: &ProjectionContext<'_>,
) -> anyhow::Result<ImportedBaseline> {
    let mut fields = row
        .mode
        .as_ref()
        .map(|mode| adif::mode(mode, context.recognition))
        .unwrap_or_default();
    let mut confirmation = ConfirmationBasis {
        own_call: row.own_call.clone(),
        worked_call: row.worked_call.clone(),
        start: row.start,
        ..ConfirmationBasis::default()
    };
    if let Some(call) = &row.own_call {
        fields.insert("STATION_CALLSIGN".into(), call.as_str().into());
    }
    if let Some(call) = &row.worked_call {
        fields.insert("CALL".into(), call.as_str().into());
    }
    if let Some(start) = row.start
        && let Ok((date, time)) = adif::contact_time(start)
    {
        fields.insert("QSO_DATE".into(), date);
        fields.insert("TIME_ON".into(), time);
    }
    if let Some(pair) = &row.primary {
        add_pair(&mut fields, pair, "BAND", "FREQ");
        confirmation.band = band(pair, service, lotw);
        confirmation.frequency = pair.frequency();
        if confirmation.band.is_none() {
            confirmation.uninterpreted_band = pair.band().map(|band| band.label().to_owned());
        }
    }

    let mut end = None;
    let mut receive = None;
    let mut own_location = RecordedLocationFields::default();
    let mut contacted_location = RecordedLocationFields::default();
    for edit in &row.edits {
        if let Some(QsoField::Station { side, .. }) = edit.field() {
            let location = match side {
                StationSide::Own => &mut own_location,
                StationSide::Contacted => &mut contacted_location,
            };
            edit.apply_location(location);
        }
        match edit {
            Edit::End(value, _) => end = *value,
            Edit::Receive(value, _) => receive = Some(value),
            _ => {}
        }
    }
    let own_location: Option<RecordedLocation> = own_location
        .normalized()
        .map(TryInto::try_into)
        .transpose()?;
    if let Some(pair) = receive.filter(|value| Some(*value) != row.primary.as_ref()) {
        add_pair(&mut fields, pair, "BAND_RX", "FREQ_RX");
    }
    if let Some(pair) = receive.or(row.primary.as_ref()) {
        confirmation.receive_band = band(pair, service, lotw);
        confirmation.receive_frequency = pair.frequency();
        if confirmation.receive_band.is_none() {
            confirmation.uninterpreted_receive_band =
                pair.band().map(|band| band.label().to_owned());
        }
    }

    if service == Service::Lotw {
        let normalized = row
            .mode
            .as_ref()
            .and_then(|mode| normalize_mode(mode, lotw).ok())
            .or_else(|| {
                fields
                    .get("SUBMODE")
                    .or_else(|| fields.get("MODE"))
                    .cloned()
            });
        confirmation.mode = normalized.as_ref().map(|value| ModeReport {
            concrete: value.clone(),
            parent: fields.get("MODE").cloned(),
            group: lotw.modes.get(value).cloned(),
        });
        fields.remove("SUBMODE");
        if let Some(value) = normalized {
            fields.insert("MODE".into(), value);
        }
        if let Some(value) = &confirmation.band {
            fields.insert("BAND".into(), value.clone());
        }
    } else {
        qrz_source_fields(row, &mut fields)?;
        if let Some(location) = &own_location {
            write_location_fields(&mut fields, location, true);
        }
        if let Some(location) = contacted_location
            .normalized()
            .map(RecordedLocation::try_from)
            .transpose()?
        {
            write_location_fields(&mut fields, &location, false);
        }
        if let Some(end) = end
            && let Ok((date, time)) = adif::contact_time(end)
        {
            fields.insert("QSO_DATE_OFF".into(), date);
            fields.insert("TIME_OFF".into(), time);
        }
        fields.retain(|field, value| {
            let Some(text) = adif::text(field, value).value else {
                return false;
            };
            *value = text;
            true
        });
        confirmation.mode = fields.get("MODE").map(|value| ModeReport {
            concrete: value.clone(),
            parent: Some(value.clone()),
            group: None,
        });
    }
    Ok(ImportedBaseline {
        native_end: if service == Service::Qrz { end } else { None },
        service,
        fields,
        own_location: if service == Service::Lotw {
            own_location
        } else {
            None
        },
        confirmation,
        coverage,
    })
}

fn qrz_source_fields(row: &RowProjection, fields: &mut AdifFields) -> anyhow::Result<()> {
    let iota: GroupId = IOTA_GROUP_ID.parse()?;
    let pota: GroupId = POTA_GROUP_ID.parse()?;
    let sota: GroupId = SOTA_GROUP_ID.parse()?;
    let mut references = BTreeMap::<&str, BTreeSet<&str>>::new();
    for edit in &row.edits {
        let (field, value) = match edit {
            Edit::StationOperator(call) | Edit::OperatorCall(StationSide::Own, Some(call)) => {
                ("OPERATOR", call.as_str().to_owned())
            }
            Edit::OperatorCall(StationSide::Contacted, Some(call)) => {
                ("CONTACTED_OP", call.as_str().to_owned())
            }
            Edit::OperatorName(StationSide::Own, Some(value)) => ("MY_NAME", value.clone()),
            Edit::OperatorName(StationSide::Contacted, Some(value)) => ("NAME", value.clone()),
            Edit::Power(Some(value)) => ("TX_PWR", adif::power(*value)),
            Edit::Sent(Some(value)) => ("RST_SENT", value.clone()),
            Edit::Received(Some(value)) => ("RST_RCVD", value.clone()),
            Edit::Comment(Some(value)) => ("COMMENT", value.clone()),
            Edit::Equipment(Some(value)) => ("MY_RIG", value.clone()),
            Edit::Antenna(Some(value)) => ("MY_ANTENNA", value.clone()),
            Edit::Contest(_, code) => ("CONTEST_ID", code.clone()),
            Edit::Reference(side, reference) => {
                let (own, contacted) = match reference.group_id {
                    group if group == iota => ("MY_IOTA", "IOTA"),
                    group if group == pota => ("MY_POTA_REF", "POTA_REF"),
                    group if group == sota => ("MY_SOTA_REF", "SOTA_REF"),
                    _ => continue,
                };
                let field = match side {
                    StationSide::Own => own,
                    StationSide::Contacted => contacted,
                };
                if let Some(value) = reference.identifier().as_deref() {
                    references.entry(field).or_default().insert(value);
                }
                continue;
            }
            _ => continue,
        };
        fields.insert(field.into(), value);
    }
    for (field, values) in references {
        fields.insert(
            field.into(),
            values.into_iter().collect::<Vec<_>>().join(","),
        );
    }
    Ok(())
}

fn add_pair(fields: &mut AdifFields, pair: &BandFrequency, band: &str, frequency: &str) {
    if let Some(value) = pair.band() {
        fields.insert(band.into(), value.label().to_ascii_uppercase());
    }
    if let Some(value) = pair.frequency() {
        fields.insert(frequency.into(), adif::frequency(value));
    }
}

fn band(pair: &BandFrequency, service: Service, lotw: &LotwConfiguration) -> Option<String> {
    match service {
        Service::Lotw => lotw.band(pair).ok(),
        Service::Qrz => pair.band().map(|band| band.label().to_ascii_uppercase()),
    }
}
