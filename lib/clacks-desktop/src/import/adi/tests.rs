//! ADI mapping cases for source conflicts, field clears, and service history.

use super::prepare_source;
use crate::review::fields::{FieldId, QsoField};
use clacks_model::group::DefinitionSet;
use std::collections::BTreeMap;

#[test]
fn only_an_explicit_empty_value_offers_a_clear() {
    let input = b"<CALL:5>K1ABC <COMMENT:0> <TX_PWR:3>bad <EOR>\n\
                  <CALL:5>K1DEF <QSO_DATE_OFF:8>20260230 <TIME_OFF:0> <EOR>\n\
                  <CALL:5>K1DEF <QSO_DATE_OFF:0> <TIME_OFF:0> <EOR>";
    let source = prepare_source(input, &DefinitionSet::default());
    let first = source
        .rows
        .first()
        .expect("the first row should be readable because its fields are framed through EOR");
    let first_edits = first.resolve(&BTreeMap::new()).present();
    assert!(first_edits.contains(&FieldId::Qso(QsoField::Comment)));
    assert!(!first_edits.contains(&FieldId::Qso(QsoField::Power)));
    let warning = first
        .warnings
        .first()
        .expect("the row should warn because TX_PWR contains the nonnumeric value bad");
    assert!(warning.message.contains("TX_PWR"));
    assert!(warning.message.contains("\"bad\""));
    assert_eq!(
        input.get(warning.span.clone()),
        Some(b"<TX_PWR:3>bad".as_slice())
    );

    let second = source
        .rows
        .get(1)
        .expect("the second row should be readable because February 30 is a value error inside a complete record");
    let second_edits = second.resolve(&BTreeMap::new()).present();
    assert!(!second_edits.contains(&FieldId::Qso(QsoField::Comment)));
    assert!(!second_edits.contains(&FieldId::Qso(QsoField::End)));
    assert!(
        second
            .warnings
            .iter()
            .any(|warning| warning.message.contains("20260230"))
    );

    let explicit = source.rows.get(2).expect(
        "the third row should be readable because its empty end fields are framed through EOR",
    );
    assert!(
        explicit
            .resolve(&BTreeMap::new())
            .present()
            .contains(&FieldId::Qso(QsoField::End))
    );
}

#[test]
fn matched_overlay_keeps_saved_precision_for_compatible_source_values() -> anyhow::Result<()> {
    use clacks_model::ids::QsoId;

    let input = b"<CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <QSO_DATE:8>20261119 <TIME_ON:6>080021 <TIME_OFF:6>080307 \
                  <FREQ:6>14.074 <FREQ_RX:5>7.074 <MODE:3>FT8 <EOR>\n\
                  <CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <QSO_DATE:8>20261119 <TIME_ON:4>0800 <TIME_OFF:4>0803 \
                  <BAND:3>20m <BAND_RX:3>40m <MODE:3>FT8 <EOR>\n\
                  <CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <QSO_DATE:8>20261119 <TIME_ON:4>0800 \
                  <FREQ:0><BAND:3>20m <FREQ_RX:0><BAND_RX:3>40m <MODE:3>FT8 <EOR>\n\
                  <FREQ:0><FREQ_RX:0><EOR>\n\
                  <BAND:0><BAND_RX:0><EOR>\n\
                  <FREQ:0><BAND:0><FREQ_RX:0><BAND_RX:0><EOR>\n\
                  <FREQ:0><FREQ_RX:3>bad<BAND_RX:0><EOR>";
    let source = prepare_source(input, &DefinitionSet::default());
    let saved = source
        .rows
        .first()
        .expect("the row with second-precision times should be readable because it ends at EOR")
        .resolve(&BTreeMap::new())
        .create(QsoId::new(), None)?;
    let incoming = source
        .rows
        .get(1)
        .expect("the row with minute-only times should be readable because it ends at EOR")
        .resolve(&BTreeMap::new());
    let mut proposed = saved.clone();
    incoming.apply(&mut proposed)?;
    assert_eq!(proposed, saved);

    let raw = incoming.create(QsoId::new(), None)?;
    assert!(raw.primary.frequency().is_none());
    assert!(
        raw.receive_override
            .and_then(|pair| pair.frequency())
            .is_none()
    );
    assert_eq!(raw.start.unix_seconds() % 60, 0);
    assert_eq!(raw.end.map(|time| time.unix_seconds() % 60), Some(0));
    let clearing = source.rows.get(2).expect(
        "the third row should be readable because its zero-length frequency fields end in EOR",
    );
    let mut explicit = saved.clone();
    clearing.resolve(&BTreeMap::new()).apply(&mut explicit)?;
    assert!(explicit.primary.frequency().is_none());
    assert!(
        explicit
            .receive_override
            .and_then(|pair| pair.frequency())
            .is_none()
    );

    let mut radio_clears = source.rows.iter().skip(3);
    let mut frequency_only = saved.clone();
    radio_clears
        .next()
        .expect("the frequency-only clear should have a row because both empty frequency fields precede EOR")
        .resolve(&BTreeMap::new())
        .apply(&mut frequency_only)?;
    assert!(frequency_only.primary.frequency().is_none());
    assert_eq!(frequency_only.primary.band(), saved.primary.band());
    let receive = frequency_only
        .receive_override
        .as_ref()
        .expect("the receive pair should remain because the source clears FREQ_RX and leaves BAND_RX unspecified");
    assert!(receive.frequency().is_none());
    assert_eq!(receive.band().map(|band| band.label()), Some("40m"));

    let mut band_only = saved.clone();
    radio_clears
        .next()
        .expect("the band-only clear should have a row because both empty band fields precede EOR")
        .resolve(&BTreeMap::new())
        .apply(&mut band_only)?;
    assert_eq!(band_only.primary.frequency(), saved.primary.frequency());
    assert!(band_only.primary.band().is_none());
    let receive = band_only
        .receive_override
        .as_ref()
        .expect("the receive pair should remain because the source clears BAND_RX and leaves FREQ_RX unspecified");
    assert_eq!(
        receive.frequency(),
        saved
            .receive_override
            .as_ref()
            .and_then(|pair| pair.frequency())
    );
    assert!(receive.band().is_none());

    let mut both = saved.clone();
    radio_clears
        .next()
        .expect(
            "the combined radio clear should have a row because all four empty fields precede EOR",
        )
        .resolve(&BTreeMap::new())
        .apply(&mut both)?;
    assert_eq!(both.primary, saved.primary);
    assert!(both.receive_override.is_none());

    let mut partial = saved.clone();
    radio_clears
        .next()
        .expect("the final row should be readable because bad FREQ_RX is a value error inside a complete record")
        .resolve(&BTreeMap::new())
        .apply(&mut partial)?;
    assert!(partial.primary.frequency().is_none());
    assert_eq!(partial.primary.band(), saved.primary.band());
    assert_eq!(partial.receive_override, band_only.receive_override);
    Ok(())
}

#[test]
fn same_minute_end_keeps_saved_seconds_during_start_review() -> anyhow::Result<()> {
    use clacks_model::{contact::Qso, ids::QsoId};

    let input = b"<CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <QSO_DATE:8>20261119 <TIME_ON:6>120000 <TIME_OFF:6>120015 \
                  <BAND:3>20m <MODE:3>FT8 <EOR>\n\
                  <CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <QSO_DATE:8>20261119 <TIME_ON:6>120030 <TIME_OFF:4>1200 \
                  <BAND:3>20m <MODE:3>FT8 <EOR>";
    let source = prepare_source(input, &DefinitionSet::default());
    let saved = source
        .rows
        .first()
        .expect("the saved contact fixture should have a row because its start and end precede EOR")
        .resolve(&BTreeMap::new())
        .create(QsoId::new(), None)?;
    let incoming = source
        .rows
        .get(1)
        .expect("the incoming contact should have a row because its later start and minute-only end precede EOR")
        .resolve(&BTreeMap::new());

    let mut comparison = saved.clone();
    incoming.apply(&mut comparison)?;
    assert_eq!(comparison.end, saved.end);
    assert!(Qso::try_from(comparison.clone()).is_err());
    comparison.start = saved.start;
    assert_eq!(Qso::try_from(comparison)?, Qso::try_from(saved)?);
    Ok(())
}

#[test]
fn explicit_position_clears_preserve_the_omitted_position_kind() -> anyhow::Result<()> {
    use clacks_model::{contact::StationSnapshot, ids::QsoId, position::Position};

    let contact = "<CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <QSO_DATE:8>20261119 <TIME_ON:4>0800 <BAND:3>20m <MODE:3>FT8";
    let input = format!(
        "{contact} <MY_LAT:11>N040 00.000 <MY_LON:11>W075 00.000 <GRIDSQUARE:4>FN20 <EOR>\n\
         {contact} <MY_GRIDSQUARE:0><GRIDSQUARE:0><EOR>\n\
         {contact} <MY_LAT:0><MY_LON:0><LAT:0><LON:0><EOR>\n\
         {contact} <MY_LAT:0><EOR>\n\
         {contact} <MY_GRIDSQUARE:0><MY_LAT:0><MY_LON:0><GRIDSQUARE:0><LAT:0><LON:0><EOR>"
    );
    let source = prepare_source(input.as_bytes(), &DefinitionSet::default());
    let mut rows = source.rows.iter();
    let saved = rows
        .next()
        .expect("the saved-location fixture should have a row because both station positions precede EOR")
        .resolve(&BTreeMap::new())
        .create(QsoId::new(), None)?;
    let position = |station: &StationSnapshot| {
        station
            .location
            .as_ref()
            .and_then(|location| location.position().clone())
    };
    let point = position(&saved.own_station);
    let grid = position(&saved.contacted_station);
    assert!(matches!(point, Some(Position::ExplicitPoint { .. })));
    assert!(matches!(grid, Some(Position::GridArea(_))));

    for (own, worked) in [
        (point.clone(), None),
        (None, grid.clone()),
        (point, grid),
        (None, None),
    ] {
        let row = rows.next().expect(
            "each position-clear case should have a row because its empty fields precede EOR",
        );
        let mut proposed = saved.clone();
        row.resolve(&BTreeMap::new()).apply(&mut proposed)?;
        assert_eq!(position(&proposed.own_station), own);
        assert_eq!(position(&proposed.contacted_station), worked);
    }
    Ok(())
}

#[test]
fn empty_submode_clears_only_the_saved_specialization() -> anyhow::Result<()> {
    use clacks_model::{ids::QsoId, recognition::mode::ModeIdentity};

    let input = b"<CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <QSO_DATE:8>20261119 <TIME_ON:4>0800 \
                  <BAND:3>20m <MODE:3>PSK <SUBMODE:5>PSK31 <EOR>\n\
                  <CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <QSO_DATE:8>20261119 <TIME_ON:4>0800 \
                  <BAND:3>20m <SUBMODE:0><EOR>\n\
                  <CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <QSO_DATE:8>20261119 <TIME_ON:4>0800 \
                  <BAND:3>20m <EOR>";
    let source = prepare_source(input, &DefinitionSet::default());
    let saved = source
        .rows
        .first()
        .expect(
            "the mode fixture should have a row because MODE and SUBMODE are framed through EOR",
        )
        .resolve(&BTreeMap::new())
        .create(QsoId::new(), None)?;
    let mode = FieldId::Qso(QsoField::Mode);
    let empty = source
        .rows
        .get(1)
        .expect("the submode clear should have a row because the empty SUBMODE field precedes EOR")
        .resolve(&BTreeMap::new());
    assert!(empty.present().contains(&mode));
    let mut proposed = saved.clone();
    empty.apply(&mut proposed)?;
    assert_eq!(
        proposed.mode.identity(),
        ModeIdentity::Parent {
            label: "PSK".into()
        }
    );

    let omitted = source
        .rows
        .get(2)
        .expect("the third row should be readable because EOR closes it even with both mode fields omitted")
        .resolve(&BTreeMap::new());
    assert!(!omitted.present().contains(&mode));
    let mut unchanged = saved.clone();
    omitted.apply(&mut unchanged)?;
    assert_eq!(unchanged.mode, saved.mode);
    Ok(())
}

#[test]
fn unfamiliar_submode_is_retained_and_invalid_extension_keeps_the_base_grid() -> anyhow::Result<()>
{
    use clacks_model::{ids::QsoId, recognition::mode::ModeIdentity};

    let input = b"<CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <QSO_DATE:8>20261119 <TIME_ON:4>0800 \
                  <BAND:3>20m <SUBMODE:7>Example <GRIDSQUARE:8>FN01MH42 <GRIDSQUARE_EXT:5>BQ98A <EOR>";
    let source = prepare_source(input, &DefinitionSet::default());
    let row = source
        .rows
        .first()
        .expect("the row should be readable because the unknown submode and invalid extension are value issues inside a complete record");
    assert!(
        row.warnings
            .iter()
            .any(|warning| warning.message.contains("SUBMODE")
                && warning.message.contains("Example"))
    );
    assert!(
        row.warnings
            .iter()
            .any(|warning| warning.message.contains("GRIDSQUARE_EXT")
                && warning.message.contains("BQ98A"))
    );
    let fields = row.resolve(&BTreeMap::new()).create(QsoId::new(), None)?;
    assert_eq!(
        fields.mode.identity(),
        ModeIdentity::Specialization {
            label: "Example".into(),
            parent: None
        }
    );
    let location = fields
        .contacted_station
        .location
        .expect("the worked location should exist because the row supplies a valid base locator");
    let position = location
        .position()
        .as_ref()
        .expect("the position should use the base locator because only its extension is invalid");
    assert_eq!(position.display_locator().length().get(), 8);
    Ok(())
}

#[test]
fn recognized_log4om_signature_interprets_scalar_lengths_with_framed_header() -> anyhow::Result<()>
{
    let input = "# Export\n<ADIF_VER:5>3.1.0 <PROGRAMID:7>LOG4OM2 <PROGRAMVERSION:8>2.42.1.0 <EOH>\n\
                 <CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <QSO_DATE:8>20261119 <TIME_ON:4>0800 \n                 <BAND:3>20m <MODE:3>FT8 <QTH:4>Léon <NAME:4>Zoë! <EOR>";
    let source = prepare_source(input.as_bytes(), &DefinitionSet::default());
    assert!(source.terminal.is_none());
    let row = source
        .rows
        .first()
        .expect("the row should be readable because the recognized LOG4OM header selects scalar lengths for its non-ASCII field");
    assert!(row.warnings.is_empty());
    let fields = row
        .resolve(&BTreeMap::new())
        .create(clacks_model::ids::QsoId::new(), None)?;
    assert_eq!(
        fields
            .contacted_operator
            .as_ref()
            .and_then(|operator| operator.recorded_name().as_deref()),
        Some("Zoë!")
    );
    Ok(())
}

#[test]
fn repeated_pota_submission_fields_offer_one_park_per_station() -> anyhow::Result<()> {
    use crate::import::mapping::Choice;
    use clacks_model::{defaults::account_definitions, ids::QsoId};

    let definitions = DefinitionSet::new(account_definitions()?.groups)?;
    let input = b"<CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <QSO_DATE:8>20261119 <TIME_ON:4>0800 <MODE:3>FT8 <BAND:3>20m \
                  <MY_POTA_REF:7>US-0003 <MY_SIG:4>POTA <MY_SIG_INFO:7>US-0001 <MY_SIG_INFO:7>US-0002 \
                  <POTA_REF:21>US-0004@US-OR,US-0006 <SIG:4>POTA <SIG_INFO:13>US-0004@US-CA <SIG_INFO:13>us-0004@us-ca <SIG_INFO:7>US-0005 <EOR>";
    let source = prepare_source(input, &definitions);
    let row = source.rows.first().expect(
        "the source row should be readable because the supplied fields are framed through EOR",
    );
    assert_eq!(row.disputes.len(), 2);
    let identifiers = |station: &clacks_model::contact::StationSnapshot| {
        station
            .references
            .iter()
            .filter_map(|entry| entry.identifier().clone())
            .collect::<Vec<_>>()
    };
    for (own, other, expected_own, expected_other) in [
        (
            Choice::Use(0),
            Choice::Use(2),
            vec!["US-0001", "US-0003"],
            vec!["US-0004@US-OR", "US-0005", "US-0006"],
        ),
        (
            Choice::Use(1),
            Choice::Use(0),
            vec!["US-0002", "US-0003"],
            vec!["US-0004@US-CA", "US-0006"],
        ),
        (
            Choice::Omit,
            Choice::Omit,
            vec!["US-0003"],
            vec!["US-0004@US-OR", "US-0006"],
        ),
    ] {
        let fields = row
            .resolve(&BTreeMap::from([(0, own), (1, other)]))
            .create(QsoId::new(), None)?;
        assert_eq!(identifiers(&fields.own_station), expected_own);
        assert_eq!(identifiers(&fields.contacted_station), expected_other);
    }
    assert_eq!(
        row.disputes
            .iter()
            .map(|dispute| (dispute.fact.as_str(), dispute.alternatives.len()))
            .collect::<Vec<_>>(),
        [("MY_SIG_INFO", 2), ("SIG_INFO", 3)]
    );
    Ok(())
}

#[test]
fn overlapping_pota_submission_and_list_share_the_park_resolution() -> anyhow::Result<()> {
    use crate::import::mapping::Choice;
    use clacks_model::{defaults::account_definitions, ids::QsoId};

    let definitions = DefinitionSet::new(account_definitions()?.groups)?;
    let input = b"<CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <QSO_DATE:8>20261119 <TIME_ON:4>0800 <MODE:3>FT8 <BAND:3>20m \
                  <POTA_REF:35>US-0004@US-OR,US-0004@US-WA,US-0006 <SIG:4>POTA <SIG_INFO:13>US-0004@US-CA <SIG_INFO:7>US-0005 <EOR>";
    let source = prepare_source(input, &definitions);
    let row = source.rows.first().expect(
        "the source row should be readable because the supplied fields are framed through EOR",
    );
    let option = |fact: &str, identifier: &str| {
        let (index, dispute) = row
            .disputes
            .iter()
            .enumerate()
            .find(|(_, dispute)| dispute.fact == fact)?;
        let alternative = dispute
            .alternatives
            .iter()
            .position(|value| value.value == identifier)?;
        Some((index, Choice::Use(alternative)))
    };
    let submission_park = option("SIG_INFO", "US-0004@US-CA").expect(
        "California should be offered because one SIG_INFO occurrence supplies US-0004@US-CA",
    );
    let other_park = option("SIG_INFO", "US-0005")
        .expect("US-0005 should be offered because it is a separate SIG_INFO occurrence");
    let list_park = option("POTA_REF US-0004", "US-0004@US-OR")
        .expect("Oregon should be offered because POTA_REF supplies US-0004@US-OR");

    assert!(row.source_resolved(&BTreeMap::from([submission_park])));
    assert!(!row.source_resolved(&BTreeMap::from([other_park])));
    for (choices, expected) in [
        (
            BTreeMap::from([submission_park, list_park]),
            vec!["US-0004@US-CA", "US-0006"],
        ),
        (
            BTreeMap::from([other_park, list_park]),
            vec!["US-0004@US-OR", "US-0005", "US-0006"],
        ),
        (
            BTreeMap::from([(submission_park.0, Choice::Omit), list_park]),
            vec!["US-0004@US-OR", "US-0006"],
        ),
    ] {
        assert!(row.source_resolved(&choices));
        let fields = row.resolve(&choices).create(QsoId::new(), None)?;
        let references = fields
            .contacted_station
            .references
            .iter()
            .filter_map(|reference| reference.identifier().as_deref())
            .collect::<Vec<_>>();
        assert_eq!(references, expected);
    }
    Ok(())
}

#[test]
fn disputed_park_qualifiers_resolve_independently_of_other_references() -> anyhow::Result<()> {
    use crate::import::mapping::Choice;
    use clacks_model::{defaults::account_definitions, ids::QsoId};

    let definitions = DefinitionSet::new(account_definitions()?.groups)?;
    let input = b"<CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <QSO_DATE:8>20261119 <TIME_ON:4>0800 <MODE:3>FT8 <BAND:3>20m \
                  <POTA_REF:21>US-0001@US-CA,US-0002 <SIG:4>POTA <SIG_INFO:13>US-0001@US-OR <EOR>\n\
                  <CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <QSO_DATE:8>20261119 <TIME_ON:4>0800 <MODE:3>FT8 <BAND:3>20m \
                  <POTA_REF:15>US-0001,US-0003 <EOR>";
    let source = prepare_source(input, &definitions);
    let row = source.rows.first().expect(
        "the row should be readable because the competing park fields are framed through EOR",
    );
    assert_eq!(row.disputes.len(), 1);
    for (choice, expected) in [
        (Choice::Use(0), vec!["US-0001@US-CA", "US-0002"]),
        (Choice::Use(1), vec!["US-0001@US-OR", "US-0002"]),
        (Choice::Omit, vec!["US-0002"]),
    ] {
        let fields = row
            .resolve(&BTreeMap::from([(0, choice)]))
            .create(QsoId::new(), None)?;
        let values = fields
            .contacted_station
            .references
            .iter()
            .filter_map(|entry| entry.identifier().as_deref())
            .collect::<Vec<_>>();
        assert_eq!(values, expected);
    }
    let mut saved = row
        .resolve(&BTreeMap::from([(0, Choice::Use(0))]))
        .create(QsoId::new(), None)?;
    let incoming = source
        .rows
        .get(1)
        .expect("the incoming parks should have a row because the bare and new references precede EOR in the second record");
    incoming.resolve(&BTreeMap::new()).apply(&mut saved)?;

    let references = saved
        .contacted_station
        .references
        .iter()
        .filter_map(|entry| entry.identifier().as_deref())
        .collect::<Vec<_>>();
    assert_eq!(references, ["US-0001@US-CA", "US-0002", "US-0003"]);
    Ok(())
}

#[test]
fn service_status_choices_keep_claims_and_only_exact_representable_timestamps() -> anyhow::Result<()>
{
    use crate::import::mapping::Choice;
    use clacks_model::{
        defaults::account_definitions,
        export::{ExportContext, OutputPurpose, ProfileSet},
        ids::QsoId,
        recognition::RecognitionCatalog,
        service::{
            adif::ProjectionContext, destination::Service, evidence::ImportedClaim,
            lotw::LotwConfiguration,
        },
    };
    let seed = account_definitions()?;
    let definitions = DefinitionSet::new(seed.groups)?;
    let profile = seed
        .output_profiles
        .iter()
        .find(|profile| profile.purpose == OutputPurpose::OrdinaryInterchange)
        .expect("the ordinary ADIF profile should exist because account_definitions supplies the initial export defaults");
    let export = ExportContext {
        output_profile_id: profile.id,
        target_group_id: None,
    };
    let profiles: ProfileSet = seed
        .output_profiles
        .into_iter()
        .map(|profile| (profile.id, profile))
        .collect();
    let resolutions = BTreeMap::new();
    let context = ProjectionContext {
        recognition: RecognitionCatalog::adif_318()?,
        definitions: &definitions,
        profiles: &profiles,
        export: &export,
        resolutions: &resolutions,
    };
    let lotw = LotwConfiguration::bundled()?;
    let observed = r#"[{"CT":"QRZCOM","S":"Yes","SD":"2026-11-19T10:15:00.5981436Z"}]"#;
    let exact_receipt = r#"[{"CT":"QRZCOM","S":"Yes","SD":"2026-11-19T10:15:00Z"}]"#;
    let timestamp_reports = r#"[{"CT":"QRZCOM","S":"Yes","SD":"2026-11-19T10:15:00Z"},{"CT":"QRZCOM","S":"Yes","SD":"2026-11-19T10:15:00.500Z"},{"CT":"QRZCOM","S":"Yes","SD":"2026-11-19T13:15:00Z"},{"CT":"QRZCOM","S":"Yes","SD":"2026-11-19T11:15:00+01:00"}]"#;
    let malformed_entries = r#"[null,{"CT":"QRZCOM","S":"Yes","SD":{}},{"CT":"QRZCOM","R":"Yes","RD":"2026-11-19T13:15:00Z"}]"#;
    let input = format!(
        "Example export\n<ADIF_VER:5>3.1.8 <PROGRAMID:7>Example <PROGRAMVERSION:3>1.2 <EOH>\n\
                         <CALL:5>K1ABC <BAND:3>20m <MODE:3>FT8 <QSO_DATE:8>20261119 <TIME_ON:6>080010 \
                         <QRZCOM_QSO_UPLOAD_STATUS:1>N <APP_L4ONG_QSO_CONFIRMATIONS:{}>{observed} <EOR>\n\
                         <CALL:5>K1ABC <BAND:3>20m <MODE:3>FT8 <QSO_DATE:8>20261119 <TIME_ON:6>080010 \
                         <QRZCOM_QSO_UPLOAD_STATUS:1>Y <QRZCOM_QSO_UPLOAD_DATE:8>20261119 \
                         <APP_L4ONG_QSO_CONFIRMATIONS:{}>{observed} <EOR>\n\
                         <CALL:5>K1ABC <BAND:3>20m <MODE:3>FT8 <QSO_DATE:8>20261119 <TIME_ON:6>080010 \
                         <QRZCOM_QSO_UPLOAD_STATUS:1>Y <QRZCOM_QSO_UPLOAD_DATE:8>20261119 \
                         <APP_L4ONG_QSO_CONFIRMATIONS:{}>{timestamp_reports} <EOR>\n\
                         <CALL:5>K1ABC <BAND:3>20m <MODE:3>FT8 <QSO_DATE:8>20261119 <TIME_ON:6>080010 \
                         <QRZCOM_QSO_UPLOAD_STATUS:1>Y \
                         <QRZCOM_QSO_UPLOAD_DATE:8>20261119 <LOTW_QSL_SENT:1>Y \
                         <RST_SENT:2>59 <RST_RCVD:2>57 <COMMENT:8>Portable <IOTA:6>EU-002 <MY_IOTA:6>na-001 \
                         <NAME:5>Alice <MY_NAME:3>Bob <CONTACTED_OP:5>K1XYZ <TX_PWR:3>5.5 \
                         <MY_RIG:4>KX3A <MY_ANTENNA:6>Dipole <GRIDSQUARE:4>FN20 <MY_STATE:2>PA \
                         <POTA_REF:15>US-0001,US-0002 <MY_SOTA_REF:10>W7W/KG-001 <CONTEST_ID:15>070-160M-SPRINT <EOR>\n\
                         <CALL:5>K1ABC <QRZCOM_QSO_UPLOAD_STATUS:1>Y \
                         <APP_L4ONG_QSO_CONFIRMATIONS:{}>{malformed_entries} <EOR>\n\
                         <CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <BAND:3>20m <MODE:3>FT8 \
                         <QSO_DATE:8>20261119 <TIME_ON:6>080010 <LOTW_QSL_SENT:1>Y \
                         <IOTA:6>EU-002 <MY_IOTA:6>na-001 <EOR>\n\
                         <CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <BAND:3>20m <MODE:3>FT8 \
                         <QSO_DATE:8>20261119 <TIME_ON:6>080010 <QRZCOM_QSO_UPLOAD_STATUS:1>Y \
                         <QRZCOM_QSO_UPLOAD_DATE:8>20261119 \
                         <APP_L4ONG_QSO_CONFIRMATIONS:{}>{exact_receipt} <EOR>",
        observed.len(),
        observed.len(),
        timestamp_reports.len(),
        malformed_entries.len(),
        exact_receipt.len(),
    );
    let source = prepare_source(input.as_bytes(), &definitions);
    let row = source.rows.first().expect(
        "the contradictory upload statuses should have a row because they are framed through EOR",
    );
    assert_eq!(row.disputes.len(), 1);
    for (choice, count) in [(Choice::Use(0), 1), (Choice::Use(1), 0), (Choice::Omit, 0)] {
        let assertions = row.resolve(&BTreeMap::from([(0, choice)])).assertions(
            QsoId::new(),
            "file.adi",
            &lotw,
            &context,
        )?;
        assert_eq!(assertions.len(), count);
    }

    let row = source.rows.get(1).expect(
        "the agreeing upload statuses should have a row because they are framed through EOR",
    );
    assert!(row.disputes.is_empty());
    let assertions =
        row.resolve(&BTreeMap::new())
            .assertions(QsoId::new(), "file.adi", &lotw, &context)?;
    assert_eq!(assertions.len(), 1);
    let claim = assertions
        .first()
        .expect("a QRZ upload assertion should exist because the standard status is Y and LOG4OM reports S=Yes");
    assert_eq!(claim.date, None);
    assert!(
        row.warnings
            .iter()
            .any(|warning| warning.message.contains(".5981436")
                && warning.message.contains("fractional seconds"))
    );
    assert!(claim.provenance.mapping.contains("QRZCOM_QSO_UPLOAD_DATE"));
    assert!(claim.provenance.mapping.contains("QRZCOM.SD"));
    assert!(claim.provenance.mapping.contains("PROGRAMID Example"));
    assert!(claim.baseline.confirmation.own_call.is_none());
    assert!(claim.baseline.confirmation.start.is_some());

    let row = source.rows.get(2).expect(
        "the repeated timestamps should have a row because the LOG4OM array is framed through EOR",
    );
    let dispute = row
        .disputes
        .first()
        .expect("date review should be required because three LOG4OM timestamps name distinct instants on the same day");
    assert_eq!(dispute.alternatives.len(), 3);
    for (choice, expected_hour) in [
        (Choice::Use(0), None),
        (Choice::Use(1), Some(10)),
        (Choice::Use(2), Some(13)),
        (Choice::Omit, None),
    ] {
        let assertions = row.resolve(&BTreeMap::from([(0, choice)])).assertions(
            QsoId::new(),
            "file.adi",
            &lotw,
            &context,
        )?;
        let claim = assertions
            .first()
            .expect("an upload assertion should remain because the selected date does not alter the agreeing positive statuses");
        let actual_hour = claim
            .date
            .map(|date| jiff::Timestamp::from_second(date.unix_seconds()))
            .transpose()?
            .map(|date| jiff::tz::Offset::UTC.to_datetime(date).hour());
        assert_eq!(actual_hour, expected_hour);
        assert_eq!(
            claim.provenance.mapping.contains("QRZCOM_QSO_UPLOAD_DATE"),
            choice != Choice::Omit
        );
    }

    let row = source.rows.get(3).expect(
        "the calendar-only upload date should have a row because its fields are framed through EOR",
    );
    assert!(
        row.warnings
            .iter()
            .any(|warning| warning.message.contains("20261119")
                && warning.message.contains("calendar date"))
    );
    let assertions =
        row.resolve(&BTreeMap::new())
            .assertions(QsoId::new(), "file.adi", &lotw, &context)?;
    let claim = assertions
        .iter()
        .find(|claim| claim.service == Service::Qrz)
        .expect("the QRZ assertion should remain because QRZCOM_QSO_UPLOAD_STATUS is Y even though its date has no time");
    assert!(claim.date.is_none());
    assert!(claim.provenance.mapping.contains("QRZCOM_QSO_UPLOAD_DATE"));
    assert!(claim.baseline.confirmation.own_call.is_none());
    for (field, supplied) in [
        ("RST_SENT", "59"),
        ("RST_RCVD", "57"),
        ("COMMENT", "Portable"),
        ("NAME", "Alice"),
        ("MY_NAME", "Bob"),
        ("CONTACTED_OP", "K1XYZ"),
        ("TX_PWR", "5.500"),
        ("MY_RIG", "KX3A"),
        ("MY_ANTENNA", "Dipole"),
        ("GRIDSQUARE", "FN20"),
        ("MY_STATE", "PA"),
        ("IOTA", "EU-002"),
        ("MY_IOTA", "NA-001"),
        ("POTA_REF", "US-0001,US-0002"),
        ("MY_SOTA_REF", "W7W/KG-001"),
        ("CONTEST_ID", "070-160M-SPRINT"),
    ] {
        assert_eq!(
            claim.baseline.fields.get(field).map(String::as_str),
            Some(supplied)
        );
    }
    for absent in ["STATION_CALLSIGN", "OPERATOR", "STATE", "MY_GRIDSQUARE"] {
        assert!(!claim.baseline.fields.contains_key(absent));
    }

    let lotw_upload = assertions
        .iter()
        .find(|claim| claim.service == Service::Lotw)
        .expect("the LoTW upload assertion should exist because LOTW_QSL_SENT is Y");
    assert!(lotw_upload.baseline.confirmation.own_call.is_none());
    assert_eq!(
        lotw_upload
            .baseline
            .fields
            .get("MY_IOTA")
            .map(String::as_str),
        Some("NA-001")
    );

    let row = source
        .rows
        .get(4)
        .expect("the row should be readable because the malformed LOG4OM entries are inside an ADI field framed through EOR");
    assert!(
        row.warnings
            .iter()
            .any(|warning| warning.message.contains("entry 1") && warning.message.contains("null"))
    );
    assert!(
        row.warnings
            .iter()
            .any(|warning| warning.message.contains("QRZCOM.SD") && warning.message.contains("{}"))
    );

    let assertions =
        row.resolve(&BTreeMap::new())
            .assertions(QsoId::new(), "file.adi", &lotw, &context)?;
    assert_eq!(assertions.len(), 2);
    let upload = assertions
        .iter()
        .find(|claim| claim.claim == ImportedClaim::Uploaded)
        .expect("an upload claim should remain because S=Yes is valid independently of the malformed SD value");
    assert!(upload.date.is_none());
    assert!(upload.provenance.mapping.contains("(QRZCOM.S)"));
    let confirmation = assertions
        .iter()
        .find(|claim| claim.claim == ImportedClaim::Confirmed)
        .expect("a confirmation should exist because the next LOG4OM entry reports R=Yes");
    assert!(confirmation.date.is_some());
    assert!(confirmation.provenance.mapping.contains("(QRZCOM.R)"));
    assert!(confirmation.provenance.mapping.contains("(QRZCOM.RD)"));

    let complete = source
        .rows
        .get(5)
        .expect("the complete LoTW fixture should have a row because its QSO and service fields precede EOR")
        .resolve(&BTreeMap::new());
    let id = QsoId::new();
    clacks_model::contact::Qso::try_from(complete.create(id, None)?)?;
    let assertions = complete.assertions(id, "file.adi", &lotw, &context)?;
    let claim = assertions.first().expect(
        "the LoTW assertion should exist because the complete record supplies LOTW_QSL_SENT=Y",
    );
    assert!(claim.baseline.confirmation.own_call.is_some());
    assert_eq!(
        claim.baseline.fields.get("MY_IOTA").map(String::as_str),
        Some("NA-001")
    );

    let row = source
        .rows
        .get(6)
        .expect("the last row should be readable because both upload date fields precede EOR");
    assert!(row.disputes.is_empty());
    let warnings: Vec<_> = row
        .warnings
        .iter()
        .filter(|warning| {
            warning.message.contains("QRZCOM_QSO_UPLOAD_DATE")
                && warning.message.contains("20261119")
                && warning.message.contains("calendar date")
        })
        .collect();
    assert_eq!(warnings.len(), 1);
    let assertions =
        row.resolve(&BTreeMap::new())
            .assertions(QsoId::new(), "file.adi", &lotw, &context)?;
    let claim = assertions
        .first()
        .expect("a QRZ upload assertion should exist because its standard status is Y and LOG4OM reports S=Yes");
    let instant = "2026-11-19T10:15:00Z".parse::<jiff::Timestamp>()?;
    assert_eq!(
        claim.date.map(|date| date.unix_seconds()),
        Some(instant.as_second())
    );
    assert!(claim.provenance.mapping.contains("QRZCOM_QSO_UPLOAD_DATE"));
    assert!(claim.provenance.mapping.contains("QRZCOM.SD"));
    Ok(())
}

#[test]
fn unusable_standard_upload_is_not_covered_by_prior_exporter_sent_status() {
    let historical = r#"[{"CT":"QRZCOM","S":"Yes"}]"#;
    let input = format!(
        "<CALL:5>K1ABC <QRZCOM_QSO_UPLOAD_STATUS:1>M <APP_L4ONG_QSO_CONFIRMATIONS:{}>{historical}<EOR>",
        historical.len()
    );
    let source = prepare_source(input.as_bytes(), &DefinitionSet::default());
    let row = source
        .rows
        .first()
        .expect("the row should be readable because QRZ's modified status is framed through EOR");
    assert!(row.disputes.is_empty());
    assert!(!row.resolve(&BTreeMap::new()).has_assertions());
    assert!(!row.warnings.is_empty());
}

#[test]
fn direct_recording_uses_adif_time_radio_and_geography_authority() -> anyhow::Result<()> {
    use clacks_model::{ids::QsoId, position::Position, recognition::mode::ModeIdentity};

    let input = b"<CALL:7>K1ABC/P <OPERATOR:7>K1DEF/P <QSO_DATE:8>20261119 <TIME_ON:4>2359 <TIME_OFF:6>000015 \
                  <BAND:3>40m <FREQ:10>14.0740000 <BAND_RX:3>80m <MODE:2>CW <SUBMODE:3>FT4 \
                  <GRIDSQUARE:8>FN01MH42 <GRIDSQUARE_EXT:2>BQ <LAT:11>N045 00.000 <LON:11>W123 00.000 \
                  <TX_PWR:7>2.50000 <EOR>";
    let source = prepare_source(input, &DefinitionSet::default());
    let row = source
        .rows
        .first()
        .expect("the row should be readable because its recorded fields precede EOR");
    assert!(
        row.warnings
            .iter()
            .any(|warning| warning.message.contains("Frequency supplies band"))
    );
    assert!(row.warnings.iter().any(|warning| {
        warning
            .message
            .contains("Recognized submode supplies parent")
    }));
    assert!(row.warnings.iter().any(|warning| {
        warning
            .message
            .contains("Explicit coordinates supply locator")
    }));
    let resolved = row.resolve(&BTreeMap::new());
    assert!(resolved.minute_precision);
    let fields = resolved.create(QsoId::new(), None)?;
    assert_eq!(fields.own_station.callsign.as_str(), "K1DEF/P");
    assert_eq!(
        fields
            .own_context
            .operator
            .as_ref()
            .and_then(|value| value.callsign().as_ref())
            .map(|value| value.as_str()),
        Some("K1DEF/P")
    );
    assert_eq!(fields.contacted_station.callsign.as_str(), "K1ABC/P");
    assert_eq!(fields.primary.band().map(|v| v.label()), Some("20m"));
    assert_eq!(
        fields
            .receive_override
            .as_ref()
            .and_then(|v| v.band())
            .map(|v| v.label()),
        Some("80m")
    );
    assert_eq!(
        fields.receive_override.as_ref().and_then(|v| v.frequency()),
        None
    );
    assert_eq!(
        fields.own_context.transmit_power.map(|v| v.get()),
        Some(2500)
    );
    assert_eq!(
        fields
            .end
            .map(|end| end.unix_seconds() - fields.start.unix_seconds()),
        Some(75)
    );
    assert_eq!(
        fields.mode.identity(),
        ModeIdentity::Specialization {
            label: "FT4".into(),
            parent: Some("MFSK".into())
        }
    );
    let location = fields.contacted_station.location.expect(
        "the worked location should exist because the row supplies valid LAT and LON values",
    );
    let Some(Position::ExplicitPoint { grid_length, .. }) = location.position() else {
        anyhow::bail!("the coordinate pair must take precedence over the locator area");
    };
    assert_eq!(grid_length.get(), 10);
    Ok(())
}

#[test]
fn selected_start_resolves_end_rollover_without_disputing_agreeing_precision() -> anyhow::Result<()>
{
    use crate::import::mapping::Choice;
    use clacks_model::ids::QsoId;

    let input = b"<CALL:5>K1ABC <STATION_CALLSIGN:5>K1DEF <BAND:3>20m <MODE:3>FT8 \
                  <QSO_DATE:8>20261119 <TIME_ON:4>2359 <TIME_ON:6>235900 <TIME_ON:6>235901 \
                  <TIME_OFF:4>0000 <TIME_OFF:6>000015 <EOR>";
    let source = prepare_source(input, &DefinitionSet::default());
    let row = source.rows.first().expect(
        "the row should be readable because both start values and the end time precede EOR",
    );
    let dispute = row.disputes.first().expect(
        "start review should be required because the two TIME_ON fields name different seconds",
    );
    assert_eq!(row.disputes.len(), 1);
    assert_eq!(dispute.alternatives.len(), 2);

    for (choice, duration) in [(Choice::Use(0), 75), (Choice::Use(1), 74)] {
        let resolved = row.resolve(&BTreeMap::from([(0, choice)]));
        assert!(!resolved.minute_precision);
        let fields = resolved.create(QsoId::new(), None)?;
        assert_eq!(
            fields
                .end
                .map(|end| end.unix_seconds() - fields.start.unix_seconds()),
            Some(duration)
        );
    }
    Ok(())
}

#[test]
fn qrz_export_precedence_keeps_status_and_receipt_date_provenance_distinct() -> anyhow::Result<()> {
    use crate::import::mapping::Choice;
    use clacks_model::{
        defaults::account_definitions,
        export::{ExportContext, OutputPurpose, ProfileSet},
        ids::QsoId,
        recognition::RecognitionCatalog,
        service::{adif::ProjectionContext, lotw::LotwConfiguration},
    };
    let seed = account_definitions()?;
    let definitions = DefinitionSet::new(seed.groups)?;
    let export = ExportContext {
        output_profile_id: seed
            .output_profiles
            .iter()
            .find(|profile| profile.purpose == OutputPurpose::OrdinaryInterchange)
            .expect("the ordinary ADIF profile should exist because account_definitions supplies the initial export defaults")
            .id,
        target_group_id: None,
    };
    let profiles: ProfileSet = seed
        .output_profiles
        .into_iter()
        .map(|profile| (profile.id, profile))
        .collect();
    let context = ProjectionContext {
        recognition: RecognitionCatalog::adif_318()?,
        definitions: &definitions,
        profiles: &profiles,
        export: &export,
        resolutions: &BTreeMap::new(),
    };
    let lotw = LotwConfiguration::bundled()?;
    let prepare = |program: &str, status: &str, first_date: &str, received: &str| {
        let receipt = format!(r#"[{{"CT":"QRZCOM","R":"Yes","RD":"{received}"}}]"#);
        let input = format!(
            "Log export\n<PROGRAMID:{}>{program} <EOH>\n\
             <CALL:5>K1ABC <APP_QRZLOG_STATUS:1>{status} {first_date} \
             <APP_L4ONG_QSO_CONFIRMATIONS:{}>{receipt} <EOR>",
            program.len(),
            receipt.len(),
        );
        prepare_source(input.as_bytes(), &definitions)
    };
    let exact = "2026-11-19T10:15:00Z";
    let expected_timestamp = exact.parse::<jiff::Timestamp>()?.as_second();
    for program in ["qrzlog", "QRZLogbook", "Example"] {
        let source = prepare(program, "N", "", exact);
        let row = source.rows.first().expect(
            "the source row should be readable because the supplied fields are framed through EOR",
        );
        assert_eq!(row.disputes.is_empty(), program != "Example");
        assert!(!row.resolve(&BTreeMap::new()).has_assertions());
    }

    let source = prepare("qrzlog", "C", "<APP_QRZLOG_QSLDATE:8>20261119", exact);
    let row = source.rows.first().expect(
        "the source row should be readable because the supplied fields are framed through EOR",
    );
    let dispute = row
        .disputes
        .first()
        .expect("date review should be required because QRZ's first confirmation date and LOG4OM's received date describe different events");
    assert_eq!(row.disputes.len(), 1);
    assert_eq!(
        dispute
            .alternatives
            .iter()
            .map(|value| value.source.as_str())
            .collect::<Vec<_>>(),
        [
            "APP_QRZLOG_QSLDATE",
            "APP_L4ONG_QSO_CONFIRMATIONS (QRZCOM.RD)"
        ]
    );
    for (choice, timestamp, date_source) in [
        (Choice::Use(0), None, Some("APP_QRZLOG_QSLDATE")),
        (Choice::Use(1), Some(expected_timestamp), Some("QRZCOM.RD)")),
        (Choice::Omit, None, None),
    ] {
        let assertions = row.resolve(&BTreeMap::from([(0, choice)])).assertions(
            QsoId::new(),
            "file.adi",
            &lotw,
            &context,
        )?;
        let claim = assertions
            .first()
            .expect("a confirmation assertion should remain because both statuses confirm the QSO and the choice changes only the date");
        assert_eq!(claim.date.map(|value| value.unix_seconds()), timestamp);
        for field in ["APP_QRZLOG_QSLDATE", "QRZCOM.RD)"] {
            assert_eq!(
                claim.provenance.mapping.contains(field),
                date_source == Some(field)
            );
        }
    }

    for (received, fractional, expected) in [
        (exact, false, Some(expected_timestamp)),
        ("2026-11-19T10:15:00.5Z", true, None),
        ("1969-12-31T23:59:59Z", false, Some(-1)),
        ("9999-12-31T23:59:59Z", false, Some(253_402_300_799)),
        ("9999-12-31T23:00:00-01:00", false, Some(253_402_300_800)),
        ("2017-01-01T00:59:60+01:00", true, None),
    ] {
        let source = prepare("qrzlog", "C", "", received);
        let row = source.rows.first().expect(
            "the source row should be readable because the supplied fields are framed through EOR",
        );
        assert!(row.disputes.is_empty());
        assert_eq!(
            row.warnings
                .iter()
                .any(|warning| warning.message.contains("QRZCOM.RD")
                    && warning.message.contains("fractional seconds")),
            fractional
        );
        let assertions =
            row.resolve(&BTreeMap::new())
                .assertions(QsoId::new(), "file.adi", &lotw, &context)?;
        let claim = assertions.first().expect(
            "a confirmation assertion should exist because QRZ reports C and LOG4OM reports R=Yes",
        );
        assert_eq!(claim.date.map(|value| value.unix_seconds()), expected);
        for source in ["APP_QRZLOG_STATUS", "QRZCOM.R)", "QRZCOM.RD)"] {
            assert!(claim.provenance.mapping.contains(source));
        }
    }
    let source = prepare(
        "qrzlog",
        "C",
        "<APP_QRZLOG_QSLDATE:8>20261118",
        "2026-11-19T10:15:60Z",
    );
    let row = source
        .rows
        .first()
        .expect("the framed row supplies a valid QRZ status and calendar date");
    assert!(
        row.disputes.is_empty(),
        "an invalid leap second must not introduce a conflicting source date"
    );
    let assertions =
        row.resolve(&BTreeMap::new())
            .assertions(QsoId::new(), "file.adi", &lotw, &context)?;
    let claim = assertions
        .first()
        .expect("the valid QRZ confirmation status supplies an assertion");
    assert!(!claim.provenance.mapping.contains("QRZCOM.RD)"));
    Ok(())
}
