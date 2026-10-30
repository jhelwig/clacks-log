use crate::support;
use clacks_model::{
    effective::EligibleStationFacts,
    export::{ExportContext, OutputPurpose},
    ids::{DestinationId, QsoId},
    recognition::RecognitionCatalog,
    service::{
        adif::ProjectionContext,
        destination::{Destination, RemoteCollection},
        error::ProjectionDecision,
        qrz::{self, QrzProjectionInput},
    },
};

#[test]
fn ordinary_projection_maps_specialization_and_omits_unsupported_fields() -> anyhow::Result<()> {
    let q = support::qso(support::id::<QsoId>(1));
    let d = Destination {
        id: DestinationId::new(),
        collection: RemoteCollection::Qrz {
            book_id: Some("42".parse().unwrap()),
        },
        description: "main".into(),
    };
    let defs = support::definitions();
    let profiles = support::profiles();
    let resolutions = Default::default();
    let profile = profiles
        .values()
        .find(|p| p.purpose == OutputPurpose::OrdinaryInterchange)
        .unwrap();
    let export = ExportContext {
        output_profile_id: profile.id,
        target_group_id: None,
    };
    let context = ProjectionContext {
        recognition: RecognitionCatalog::adif_318()?,
        definitions: &defs,
        profiles: &profiles,
        export: &export,
        resolutions: &resolutions,
    };
    let result = qrz::project(&QrzProjectionInput {
        qso: &q,
        destination: &d,
        eligible: EligibleStationFacts::default(),
        context: &context,
    })?;
    let ProjectionDecision::Ready(r) = result else {
        panic!("not ready")
    };
    assert_eq!(r.value.fields.get("MODE").map(String::as_str), Some("MFSK"));
    assert_eq!(
        r.value.fields.get("SUBMODE").map(String::as_str),
        Some("FT4")
    );
    assert_eq!(
        r.value.fields.get("FREQ").map(String::as_str),
        Some("14.074000")
    );
    assert!(!r.value.fields.contains_key("APP_QRZLOG_LOGID"));
    assert!(!r.value.fields.contains_key("BAND_RX"));
    Ok(())
}

#[test]
fn station_location_equipment_reports_and_text_are_projected() -> anyhow::Result<()> {
    use clacks_model::{
        location::{CqZone, ItuZone},
        position::{Locator, Position},
        scalar::{Milliwatts, UtcSecond},
    };
    let mut q = support::qso(support::id::<QsoId>(2));
    support::edit(&mut q, |q| {
        q.own_station.location = Some(
            (clacks_model::location::RecordedLocationFields {
                position: Some(Position::GridArea(Locator::parse("FN31PR12AB34")?)),
                dxcc: Some("291".into()),
                primary_subdivision: Some("CT".into()),
                secondary_subdivision: Some("Hartford".into()),
                cq_zone: Some(CqZone::new(5)?),
                itu_zone: Some(ItuZone::new(8)?),
            })
            .try_into()
            .expect("valid native fixture"),
        );

        Ok(())
    });
    support::edit(&mut q, |q| {
        q.own_context.operator = Some(
            (clacks_model::contact::OperatorInformationFields {
                callsign: None,
                recorded_name: Some("étude".into()),
            })
            .try_into()
            .expect("valid native fixture"),
        );

        Ok(())
    });
    support::edit(&mut q, |q| {
        q.own_context.transmit_power = Some(Milliwatts::new(1001)?);

        Ok(())
    });
    support::edit(&mut q, |q| {
        q.own_context.equipment = Some(
            (clacks_model::equipment::EquipmentValuesFields {
                equipment_description: Some("rig".into()),
                antenna_description: Some("wire".into()),
                notes: Some("not sent".into()),
            })
            .try_into()
            .expect("valid native fixture"),
        );

        Ok(())
    });
    support::edit(&mut q, |q| {
        q.end = Some(UtcSecond::from_unix_seconds(86401));

        Ok(())
    });
    support::edit(&mut q, |q| {
        q.sent_report = Some("-12".into());

        Ok(())
    });
    support::edit(&mut q, |q| {
        q.received_report = Some("-10".into());

        Ok(())
    });
    support::edit(&mut q, |q| {
        q.comment = Some("hello".into());

        Ok(())
    });
    let d = Destination {
        id: DestinationId::new(),
        collection: RemoteCollection::Qrz {
            book_id: Some("42".parse().unwrap()),
        },
        description: "main".into(),
    };
    let defs = support::definitions();
    let profiles = support::profiles();
    let resolutions = Default::default();
    let profile = profiles
        .values()
        .find(|p| p.purpose == OutputPurpose::OrdinaryInterchange)
        .unwrap();
    let export = ExportContext {
        output_profile_id: profile.id,
        target_group_id: None,
    };
    let context = ProjectionContext {
        recognition: RecognitionCatalog::adif_318()?,
        definitions: &defs,
        profiles: &profiles,
        export: &export,
        resolutions: &resolutions,
    };
    let ProjectionDecision::Ready(r) = qrz::project(&QrzProjectionInput {
        qso: &q,
        destination: &d,
        eligible: EligibleStationFacts::default(),
        context: &context,
    })?
    else {
        panic!("not ready")
    };
    for (key, want) in [
        ("MY_GRIDSQUARE", "FN31PR12"),
        ("MY_GRIDSQUARE_EXT", "AB34"),
        ("MY_DXCC", "291"),
        ("MY_STATE", "CT"),
        ("MY_CNTY", "Hartford"),
        ("MY_CQ_ZONE", "5"),
        ("MY_ITU_ZONE", "8"),
        ("TX_PWR", "1.001"),
        ("MY_NAME", "etude"),
        ("MY_RIG", "rig"),
        ("MY_ANTENNA", "wire"),
        ("RST_SENT", "-12"),
        ("RST_RCVD", "-10"),
        ("COMMENT", "hello"),
        ("QSO_DATE_OFF", "19700102"),
    ] {
        assert_eq!(
            r.value.fields.get(key).map(String::as_str),
            Some(want),
            "{key}"
        );
    }
    assert!(!r.value.fields.contains_key("MY_LAT"));
    assert!(!r.warnings.is_empty());
    Ok(())
}
