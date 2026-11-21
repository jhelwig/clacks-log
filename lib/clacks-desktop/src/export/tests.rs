use super::{
    ExportDecision, ExportFormat, ReadinessField,
    prepare::{PrepareError, prepare},
};
use clacks_client_store::read::{
    export::{ConfirmationDate, ExportContact, ExportInput},
    projection::{ContactFacts, ServiceStatus},
};
use clacks_model::{
    contact::{
        BandFrequency, OperatorInformationFields, OwnOperatingContext, Qso, StationSnapshot,
        input::QsoFields,
    },
    defaults::account_definitions,
    effective::{InformationSource, LocationFacts, Sourced},
    export::{
        rules::{BindingCondition, FieldBinding, GroupExportMappings, ValueExpression},
        select::ContextResolution,
    },
    group::{DefinitionSet, GroupDefinitionFields, IdentifierDefinition},
    ids::{GroupId, QsoId},
    location::RecordedLocationFields,
    position::{Coordinates, GridLength, Locator, Position},
    recognition::{
        band::Band,
        mode::{Mode, ModeIdentity},
    },
    reference::ReferenceAssociation,
    scalar::{Hertz, Milliwatts, RecordedCallsign, UtcSecond},
    service::{destination::Service, evidence::ImportProvenance},
};
use std::collections::{BTreeMap, BTreeSet};

/// Build an export snapshot with the installed definitions.
pub(super) fn input(contacts: Vec<ExportContact>) -> anyhow::Result<ExportInput> {
    let initial = account_definitions()?;
    Ok(ExportInput {
        contacts,
        definitions: DefinitionSet::new(initial.groups)?,
        profiles: initial
            .output_profiles
            .into_iter()
            .map(|profile| (profile.id, profile))
            .collect(),
    })
}

fn references(
    group: GroupId,
    identifiers: &[&str],
) -> anyhow::Result<BTreeSet<ReferenceAssociation>> {
    identifiers
        .iter()
        .map(|identifier| ReferenceAssociation::new(group, Some((*identifier).into())))
        .collect()
}

/// Create a saved contact with valid export inputs and the specified park associations.
pub(super) fn contact(own: &[&str], contacted: &[&str]) -> anyhow::Result<ExportContact> {
    let group = "01M5QZFPZZ8TTM3MCPAT3MA7MA".parse()?;
    let contacted_references = references(group, contacted)?;
    let qso = Qso::try_from(QsoFields {
        id: QsoId::new(),
        start: UtcSecond::from_unix_seconds(1_609_459_198),
        end: None,
        own_station: StationSnapshot {
            callsign: RecordedCallsign::new("W7X/P")?,
            location: None,
            references: references(group, own)?,
        },
        contacted_station: StationSnapshot {
            callsign: RecordedCallsign::new("VE7/K1ABC/P")?,
            location: None,
            references: contacted_references.clone(),
        },
        own_context: OwnOperatingContext::default(),
        contacted_operator: None,
        primary: BandFrequency::from_recorded_pair(None, Some(Band::from_recorded_label("20m")?))?,
        receive_override: None,
        mode: Mode::from_recorded_identity(ModeIdentity::Parent {
            label: "SSB".into(),
        })?,
        sent_report: None,
        received_report: None,
        comment: None,
        tags: BTreeSet::new(),
    })?;
    Ok(ExportContact {
        qso,
        contacted_references,
        confirmation_dates: Vec::new(),
        facts: ContactFacts::default(),
    })
}

#[test]
fn ordinary_preserves_a_single_contact_with_lists_exact_utc_and_applicable_facts()
-> anyhow::Result<()> {
    let mut contact = contact(&["US-0011@US-WA", "US-0012"], &["US-0021", "US-0022"])?;
    let mut fields = contact.qso.into_fields();
    fields.end = Some(UtcSecond::from_unix_seconds(1_609_459_203));
    fields.own_station.location = Some(
        RecordedLocationFields {
            primary_subdivision: Some("WA".into()),
            secondary_subdivision: Some("WA,King".into()),
            dxcc: Some("291".into()),
            position: Some(Position::ExplicitPoint {
                coordinates: Coordinates::from_decimal_degrees("47.5", "-122.25")?,
                grid_length: GridLength::new(6)?,
            }),
            ..Default::default()
        }
        .try_into()?,
    );
    fields.own_context = OwnOperatingContext {
        transmit_power: Some(Milliwatts::new(2250)?),
        operator: Some(
            OperatorInformationFields {
                callsign: Some(RecordedCallsign::new("W7X")?),
                recorded_name: Some("Zoë".into()),
            }
            .try_into()?,
        ),
        equipment: None,
    };
    fields.primary = BandFrequency::from_recorded_pair(
        Some(Hertz::new(14_074_001)?),
        Some(Band::from_recorded_label("20m")?),
    )?;
    fields.receive_override = Some(BandFrequency::from_recorded_pair(
        Some(Hertz::new(7_074_000)?),
        Some(Band::from_recorded_label("40m")?),
    )?);
    fields.mode = Mode::from_recorded_identity(ModeIdentity::Specialization {
        label: "FT4".into(),
        parent: Some("MFSK".into()),
    })?;
    fields.sent_report = Some("-07".into());
    fields.received_report = Some("+01".into());
    fields.comment = Some("Sławomir\nThanks\rQSO\r\n73".into());
    contact.qso = fields.try_into()?;
    contact.facts.contact_location = Some(Sourced {
        value: LocationFacts {
            location: Some(
                RecordedLocationFields {
                    position: Some(Position::GridArea(Locator::parse("CN87AA00AA")?)),
                    ..Default::default()
                }
                .try_into()?,
            ),
            references: BTreeSet::new(),
        },
        source: InformationSource::Directory,
    });
    contact.facts.lotw.confirmed = true;
    contact.facts.qrz = ServiceStatus {
        uploaded: true,
        confirmed: false,
    };
    contact.facts.name = Some(Sourced {
        value: "José".into(),
        source: InformationSource::Directory,
    });
    for service in [Service::Lotw, Service::Qrz] {
        contact.confirmation_dates.push(ConfirmationDate {
            service,
            date: UtcSecond::from_unix_seconds(1_609_545_600),
            provenance: ImportProvenance {
                source: "file".into(),
                record: "row".into(),
                mapping: "LOTW_QSLRDATE".into(),
            },
        });
    }
    let plan = prepare(
        &input(vec![contact])?,
        ExportFormat::Ordinary,
        &BTreeMap::new(),
    )?;

    let [file] = plan.files.as_slice() else {
        anyhow::bail!("ordinary output must contain one file");
    };
    let [record] = file.records.as_slice() else {
        anyhow::bail!("ordinary output must contain one record");
    };
    for (field, expected) in [
        ("BAND", "20m"),
        ("BAND_RX", "40m"),
        ("CALL", "VE7/K1ABC/P"),
        ("FREQ", "14.074001"),
        ("FREQ_RX", "7.074000"),
        ("GRIDSQUARE", "CN87AA00"),
        ("GRIDSQUARE_EXT", "AA"),
        ("LAT", "N047 00.005"),
        ("LON", "W123 59.990"),
        ("LOTW_QSL_RCVD", "Y"),
        ("LOTW_QSLRDATE", "20210102"),
        ("MODE", "MFSK"),
        ("MY_CNTY", "WA,King"),
        ("MY_DXCC", "291"),
        ("MY_LAT", "N047 30.000"),
        ("MY_LON", "W122 15.000"),
        ("MY_NAME", "Zoe"),
        ("MY_POTA_REF", "US-0011@US-WA,US-0012"),
        ("MY_STATE", "WA"),
        ("NAME", "Jose"),
        ("NOTES", "Slawomir\r\nThanks\r\nQSO\r\n73"),
        ("OPERATOR", "W7X"),
        ("POTA_REF", "US-0021,US-0022"),
        ("QSO_DATE", "20201231"),
        ("QSO_DATE_OFF", "20210101"),
        ("RST_RCVD", "+01"),
        ("RST_SENT", "-07"),
        ("STATION_CALLSIGN", "W7X/P"),
        ("SUBMODE", "FT4"),
        ("TIME_OFF", "000003"),
        ("TIME_ON", "235958"),
        ("TX_PWR", "2.250"),
    ] {
        assert_eq!(
            record.fields.get(field).map(String::as_str),
            Some(expected),
            "{field}"
        );
    }
    assert!(!record.fields.contains_key("APP_QRZLOG_STATUS"));
    assert!(!record.fields.contains_key("APP_QRZLOG_QSLDATE"));
    assert!(!record.fields.contains_key("QRZCOM_QSO_UPLOAD_STATUS"));
    for field in ["MY_NAME", "NAME", "NOTES"] {
        assert!(plan.warnings.iter().any(|warning| warning.field == field));
    }

    Ok(())
}

#[test]
fn ordinary_preserves_valid_members_of_pota_lists_with_unsupported_identifiers()
-> anyhow::Result<()> {
    let contact = contact(&["ABCDE-1234"], &["ABCDE-1234", "US-0021"])?;
    let id = contact.qso.id();
    let snapshot = input(vec![contact])?;

    let plan = prepare(&snapshot, ExportFormat::Ordinary, &BTreeMap::new())?;

    let [file] = plan.files.as_slice() else {
        anyhow::bail!("the saved contact still produces one ordinary ADI file");
    };
    let [entry] = file.records.as_slice() else {
        anyhow::bail!("invalid optional park identifiers leave one entry for the saved contact");
    };
    assert_eq!(entry.qso_id, id);
    assert_eq!(
        entry.fields.get("POTA_REF").map(String::as_str),
        Some("US-0021")
    );
    assert!(!entry.fields.contains_key("MY_POTA_REF"));
    for field in ["POTA_REF", "MY_POTA_REF"] {
        assert!(
            plan.warnings
                .iter()
                .any(|warning| warning.qso_id == id && warning.field == field)
        );
    }

    Ok(())
}

#[test]
fn qso_start_outside_adi_date_range_prevents_export_of_that_contact() -> anyhow::Result<()> {
    let mut contact = contact(&["US-0011"], &[])?;
    let mut fields = contact.qso.into_fields();
    fields.start = UtcSecond::from_unix_seconds(-1_262_304_002); // 1929-12-31 23:59:58 UTC
    contact.qso = fields.try_into()?;
    let snapshot = input(vec![contact])?;

    let result = prepare(&snapshot, ExportFormat::Ordinary, &BTreeMap::new());

    assert!(matches!(
        result,
        Err(PrepareError::InvalidContact {
            field: ReadinessField::StartTime,
            ..
        })
    ));
    let pota = prepare(&snapshot, ExportFormat::Pota, &BTreeMap::new())?;
    let [skipped] = pota.skipped.as_slice() else {
        anyhow::bail!("the unrepresentable start prevents including the contact in POTA output");
    };
    assert_eq!(skipped.missing, [ReadinessField::StartTime]);
    assert!(pota.files.is_empty());

    Ok(())
}

#[test]
fn ordinary_requires_a_choice_to_omit_or_select_the_contest_field() -> anyhow::Result<()> {
    let mut contact = contact(&[], &[])?;
    let first = "01M5QZFPZZX5EW5Y7JPCDYD3H1".parse()?;
    let second = "01M5QZFPZZC215Z61VW00MX34Y".parse()?;
    let mut fields = contact.qso.into_fields();
    let id = fields.id;
    fields.own_station.references.extend([
        ReferenceAssociation::new(first, None)?,
        ReferenceAssociation::new(second, None)?,
    ]);
    contact.qso = fields.try_into()?;
    let snapshot = input(vec![contact])?;
    let review = prepare(&snapshot, ExportFormat::Ordinary, &BTreeMap::new())?;
    let [ExportDecision::AmbiguousContextField { candidates, .. }] = review.decisions.as_slice()
    else {
        anyhow::bail!("distinct contests must call for one choice");
    };
    assert_eq!(
        candidates,
        &BTreeMap::from([
            (first, "070-160M-SPRINT".into()),
            (second, "070-3-DAY".into())
        ])
    );

    for (choice, expected) in [
        (
            ContextResolution::ChooseTarget(first),
            Some("070-160M-SPRINT"),
        ),
        (ContextResolution::OmitReported, None),
    ] {
        let plan = prepare(
            &snapshot,
            ExportFormat::Ordinary,
            &BTreeMap::from([(id, choice)]),
        )?;
        assert!(plan.decisions.is_empty());
        let [file] = plan.files.as_slice() else {
            anyhow::bail!("a contest choice permits one ordinary ADI file");
        };
        let [entry] = file.records.as_slice() else {
            anyhow::bail!("a contest choice must produce one entry for the original contact");
        };
        assert_eq!(entry.fields.get("CONTEST_ID").map(String::as_str), expected);
        assert_eq!(entry.qso_id, id);
        assert_eq!(
            plan.warnings
                .iter()
                .any(|warning| warning.field == "CONTEST_ID"),
            expected.is_none()
        );
    }

    Ok(())
}

#[test]
fn pota_partitions_and_expands_only_eligible_contacts() -> anyhow::Result<()> {
    let mut expanded = contact(
        &["US-0011@US-WA", "US-0012@US-WA"],
        &["US-0021", "US-0021@US-WA", "US-0022", "US-0023", "bad"],
    )?;
    let mut fields = expanded.qso.into_fields();
    let unrelated_contest = "01M5QZFPZZX5EW5Y7JPCDYD3H1".parse()?;
    fields
        .own_station
        .references
        .insert(ReferenceAssociation::new(unrelated_contest, None)?);
    expanded.qso = fields.try_into()?;
    let solo = contact(&["US-0011@US-WA"], &[])?;
    let mut missing = contact(&["US-0011@US-WA"], &[])?;
    let mut fields = missing.qso.into_fields();
    fields.primary = BandFrequency::from_recorded_pair(Some(Hertz::new(14_074_001)?), None)?;
    missing.qso = fields.try_into()?;
    let [expanded_id, solo_id, missing_id] = [expanded.qso.id(), solo.qso.id(), missing.qso.id()];
    let snapshot = input(vec![expanded, solo, missing])?;
    let plan = prepare(&snapshot, ExportFormat::Pota, &BTreeMap::new())?;

    assert_eq!(plan.requested_count, 3);
    assert_eq!(plan.eligible_count(), 2);
    assert!(
        plan.warnings
            .iter()
            .any(|warning| warning.qso_id == expanded_id && warning.field == "SIG_INFO")
    );
    let [skipped] = plan.skipped.as_slice() else {
        anyhow::bail!("only the frequency-only contact lacks POTA requirements");
    };
    assert_eq!(skipped.qso_id, missing_id);
    assert_eq!(skipped.missing, [ReadinessField::TransmitBand]);
    assert_eq!(plan.files.len(), 2);
    for (park, includes_solo) in [("US-0011", true), ("US-0012", false)] {
        let file = plan
            .files
            .iter()
            .find(|file| file.partition == [park, "US-WA"])
            .ok_or_else(|| anyhow::anyhow!("missing output for {park} in Washington"))?;
        let entries = file
            .records
            .iter()
            .map(|record| {
                assert_eq!(
                    record.fields.get("MY_SIG").map(String::as_str),
                    Some("POTA")
                );
                assert_eq!(
                    record.fields.get("MY_SIG_INFO").map(String::as_str),
                    Some(park)
                );
                assert_eq!(
                    record.fields.get("MY_STATE").map(String::as_str),
                    Some("WA")
                );
                assert!(!record.fields.contains_key("CONTEST_ID"));
                assert!(!record.fields.contains_key("MY_POTA_REF"));
                (
                    record.qso_id,
                    record.fields.get("SIG").map(String::as_str),
                    record.fields.get("SIG_INFO").map(String::as_str),
                )
            })
            .collect::<BTreeSet<_>>();
        let mut expected = BTreeSet::from([
            (expanded_id, Some("POTA"), Some("US-0021")),
            (expanded_id, Some("POTA"), Some("US-0022")),
            (expanded_id, Some("POTA"), Some("US-0023")),
        ]);
        if includes_solo {
            expected.insert((solo_id, None, None));
        }
        assert_eq!(entries, expected);
        assert_eq!(file.records.len(), expected.len());
    }

    Ok(())
}

#[test]
fn qrz_confirmation_keeps_first_confirmation_separate_from_receipt_and_upload() -> anyhow::Result<()>
{
    let mut contact = contact(&[], &[])?;
    contact.facts.qrz.confirmed = true;
    for (mapping, date) in [
        ("APP_QRZLOG_QSLDATE at bytes 42–50", 1_609_545_600),
        (
            "APP_L4ONG_QSO_CONFIRMATIONS (QRZCOM.RD) at bytes 123–158",
            1_609_632_000,
        ),
    ] {
        contact.confirmation_dates.push(ConfirmationDate {
            service: Service::Qrz,
            date: UtcSecond::from_unix_seconds(date),
            provenance: ImportProvenance {
                source: "backup.adi".into(),
                record: "Record 1".into(),
                mapping: mapping.into(),
            },
        });
    }
    let snapshot = input(vec![contact])?;
    let plan = prepare(&snapshot, ExportFormat::Ordinary, &BTreeMap::new())?;
    let entry = plan
        .files
        .iter()
        .flat_map(|file| &file.records)
        .next()
        .expect("ordinary export retains the confirmed contact");
    assert_eq!(
        entry.fields.get("APP_QRZLOG_STATUS").map(String::as_str),
        Some("C")
    );
    assert_eq!(
        entry.fields.get("APP_QRZLOG_QSLDATE").map(String::as_str),
        Some("20210102")
    );
    assert!(!entry.fields.contains_key("QRZCOM_QSO_DOWNLOAD_STATUS"));
    assert!(
        plan.warnings
            .iter()
            .any(|warning| warning.field == "APP_QRZLOG_QSLDATE")
    );

    Ok(())
}

#[test]
fn ordinary_omits_unrepresentable_confirmation_dates_and_retains_status() -> anyhow::Result<()> {
    let mut contact = contact(&[], &[])?;
    contact.facts.lotw.confirmed = true;
    contact.facts.qrz.confirmed = true;
    for (service, field) in [
        (Service::Lotw, "LOTW_QSLRDATE"),
        (Service::Qrz, "APP_QRZLOG_QSLDATE"),
    ] {
        contact.confirmation_dates.push(ConfirmationDate {
            service,
            date: UtcSecond::from_unix_seconds(-1_262_304_002),
            provenance: ImportProvenance {
                source: "backup.adi".into(),
                record: "Record 1".into(),
                mapping: format!("{field} at bytes 42–50"),
            },
        });
    }
    let id = contact.qso.id();
    let snapshot = input(vec![contact])?;

    let plan = prepare(&snapshot, ExportFormat::Ordinary, &BTreeMap::new())?;

    let entry = plan
        .files
        .iter()
        .flat_map(|file| &file.records)
        .next()
        .expect("confirmed contact remains exportable when its dates cannot be represented");
    assert_eq!(
        entry.fields.get("LOTW_QSL_RCVD").map(String::as_str),
        Some("Y")
    );
    assert_eq!(
        entry.fields.get("APP_QRZLOG_STATUS").map(String::as_str),
        Some("C")
    );
    for field in ["LOTW_QSLRDATE", "APP_QRZLOG_QSLDATE"] {
        assert!(!entry.fields.contains_key(field));
        assert!(
            plan.warnings
                .iter()
                .any(|warning| warning.qso_id == id && warning.field == field)
        );
    }

    Ok(())
}

#[test]
fn reference_fields_cannot_replace_recorded_station_identity_case_insensitively()
-> anyhow::Result<()> {
    let group_id = GroupId::new();
    let mut contact = contact(&[], &[])?;
    let mut fields = contact.qso.into_fields();
    fields
        .own_station
        .references
        .insert(ReferenceAssociation::new(group_id, None)?);
    contact.qso = fields.try_into()?;
    let mut snapshot = input(vec![contact])?;
    let mut definitions: Vec<_> = snapshot.definitions.iter().cloned().collect();
    definitions.push(
        GroupDefinitionFields {
            id: group_id,
            name: "Conflicting test mapping".into(),
            identifier: IdentifierDefinition {
                required: false,
                normalization: Vec::new(),
                syntax: None,
            },
            bears_location_context: false,
            catalog: None,
            export_mappings: GroupExportMappings {
                ordinary_interchange: vec![FieldBinding {
                    field: "call".into(),
                    value: ValueExpression::Literal("W1DIFFERENT".into()),
                    when: BindingCondition::Always,
                }],
                ..Default::default()
            },
        }
        .try_into()?,
    );
    snapshot.definitions = DefinitionSet::new(definitions)?;

    let Err(error) = prepare(&snapshot, ExportFormat::Ordinary, &BTreeMap::new()) else {
        anyhow::bail!("a case-insensitive reference binding conflict must prevent preparation");
    };
    assert!(error.to_string().contains("conflicting values for CALL"));

    Ok(())
}

#[test]
fn ordinary_preserves_printable_callsigns_and_pota_requires_receiver_syntax() -> anyhow::Result<()>
{
    let mut contact = contact(&["US-0011"], &[])?;
    let mut fields = contact.qso.into_fields();
    fields.contacted_station.callsign = RecordedCallsign::new("W7X/P-7")?;
    contact.qso = fields.try_into()?;
    let snapshot = input(vec![contact])?;

    let ordinary = prepare(&snapshot, ExportFormat::Ordinary, &BTreeMap::new())?;
    let record = ordinary
        .files
        .iter()
        .flat_map(|file| &file.records)
        .next()
        .expect("printable native callsign is representable as ordinary ADI");
    assert_eq!(
        record.fields.get("CALL").map(String::as_str),
        Some("W7X/P-7")
    );

    let pota = prepare(&snapshot, ExportFormat::Pota, &BTreeMap::new())?;
    let [skipped] = pota.skipped.as_slice() else {
        anyhow::bail!("POTA rejects a dash in the contacted callsign");
    };
    assert_eq!(skipped.missing, [ReadinessField::ContactedCallsign]);
    assert!(pota.files.is_empty());

    Ok(())
}

#[test]
fn ordinary_omits_non_catalog_subdivision_text_with_field_warnings() -> anyhow::Result<()> {
    let mut contact = contact(&[], &[])?;
    let mut fields = contact.qso.into_fields();
    fields.own_station.location = Some(
        RecordedLocationFields {
            primary_subdivision: Some("free text state".into()),
            secondary_subdivision: Some("free text county".into()),
            ..Default::default()
        }
        .try_into()?,
    );
    contact.qso = fields.try_into()?;
    let snapshot = input(vec![contact])?;

    let plan = prepare(&snapshot, ExportFormat::Ordinary, &BTreeMap::new())?;
    let record = plan
        .files
        .iter()
        .flat_map(|file| &file.records)
        .next()
        .expect("unsupported optional location fields leave the QSO exportable");
    for field in ["MY_STATE", "MY_CNTY"] {
        assert!(!record.fields.contains_key(field));
        assert!(plan.warnings.iter().any(|warning| warning.field == field));
    }

    Ok(())
}

#[test]
fn pota_rejects_a_mapping_that_collapses_distinct_contacted_parks() -> anyhow::Result<()> {
    let contact = contact(&["US-0011"], &["US-0021", "US-0022"])?;
    let mut snapshot = input(vec![contact])?;
    let group = "01M5QZFPZZ8TTM3MCPAT3MA7MA".parse()?;
    let profile = "01M5QZFPZZM8HD710JY3Z4M36X".parse()?;
    let mut definition = snapshot
        .definitions
        .get(group)
        .expect("initial definitions contain the POTA group")
        .clone()
        .into_fields();
    let mapping = definition
        .export_mappings
        .targeted
        .get_mut(&profile)
        .expect("initial definitions contain the targeted POTA mapping");
    let field = mapping
        .fields
        .iter_mut()
        .find(|binding| binding.field == "SIG_INFO")
        .expect("the targeted POTA mapping carries contacted park identity");
    field.value = ValueExpression::Literal("US-0021".into());
    let replacement = definition.try_into()?;
    snapshot.definitions = DefinitionSet::new(
        snapshot
            .definitions
            .iter()
            .filter(|definition| definition.id() != group)
            .cloned()
            .chain([replacement])
            .collect(),
    )?;

    let Err(error) = prepare(&snapshot, ExportFormat::Pota, &BTreeMap::new()) else {
        anyhow::bail!("a mapping that loses a contacted park must fail preparation");
    };
    assert!(error.to_string().contains("contacted parks"));

    Ok(())
}
