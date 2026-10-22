mod support;
use clacks_model::{
    contact::BandFrequency,
    ids::{DestinationId, QsoId},
    recognition::RecognitionCatalog,
    recognition::band::Band,
    recognition::mode::{Mode, ModeIdentity},
    service::{
        destination::{Destination, RemoteCollection},
        error::ProjectionDecision,
        lotw::{self, LotwConfiguration, LotwProjectionInput, PublicSigningIdentity},
    },
};

#[test]
fn full_mapping_chain_and_unsupported_modes() -> anyhow::Result<()> {
    let c = LotwConfiguration::bundled()?;
    for (parent, sub, want) in [
        ("MFSK", Some("FT4"), "FT4"),
        ("FT4", None, "FT4"),
        ("FT8", None, "FT8"),
        ("SSB", Some("USB"), "SSB"),
        ("PSK", Some("QPSK31"), "PSK31"),
        ("MFSK", Some("JS8"), "DATA"),
        ("MFSK", None, "DATA"),
    ] {
        let m = Mode::from_recorded_identity(match sub {
            Some(s) => ModeIdentity::Specialization {
                label: s.into(),
                parent: Some(parent.into()),
            },
            None => ModeIdentity::Parent {
                label: parent.into(),
            },
        })?;
        assert_eq!(lotw::normalize_mode(&m, &c)?, want);
    }
    assert!(
        lotw::normalize_mode(
            &Mode::from_recorded_identity(ModeIdentity::Parent {
                label: "NOT-A-MODE".into()
            })?,
            &c
        )
        .is_err()
    );
    assert!(LotwConfiguration::from_json("{}").is_err());
    Ok(())
}

#[test]
fn projection_freezes_actual_values_and_whole_receive_override() -> anyhow::Result<()> {
    let c = LotwConfiguration::bundled()?;
    let mut q = support::qso(support::id::<QsoId>(1));
    let d = Destination {
        id: DestinationId::new(),
        collection: RemoteCollection::Lotw {
            account: "w7hex".into(),
        },
        description: "main".into(),
    };
    let signing = PublicSigningIdentity {
        callsign: q.own_station.callsign.clone(),
        dxcc: "291".into(),
    };
    let context = support::definitions();
    let get = |q: &clacks_model::contact::Qso| -> anyhow::Result<lotw::LotwSubmission> {
        match lotw::project(&LotwProjectionInput {
            qso: q,
            destination: &d,
            signing: &signing,
            configuration: &c,
            definitions: &context,
        })? {
            ProjectionDecision::Ready(r) => Ok(r.value),
            x => anyhow::bail!("{x:?}"),
        }
    };
    let original = get(&q)?;
    q.comment = Some("not sent".into());
    assert_eq!(original, get(&q)?);
    q.receive_override = Some(BandFrequency::new(
        None,
        Some(Band::from_recorded_label("40m")?),
        RecognitionCatalog::adif_318()?,
    )?);
    let rx = get(&q)?;
    assert_ne!(original.external_key(), rx.external_key());
    assert!(!rx.contact_fields.contains_key("FREQ_RX"));
    assert_eq!(rx.signing_identity.dxcc, "291");
    Ok(())
}

#[test]
fn entity_dependent_subdivisions_and_county_parent_are_resolved() -> anyhow::Result<()> {
    use clacks_model::location::RecordedLocation;
    let c = LotwConfiguration::bundled()?;
    let d = Destination {
        id: DestinationId::new(),
        collection: RemoteCollection::Lotw {
            account: "w7hex".into(),
        },
        description: "main".into(),
    };
    let definitions = support::definitions();
    let mut q = support::qso(support::id(1));
    let project = |q: &clacks_model::contact::Qso, entity: &str| {
        lotw::project(&LotwProjectionInput {
            qso: q,
            destination: &d,
            signing: &PublicSigningIdentity {
                callsign: q.own_station.callsign.clone(),
                dxcc: entity.into(),
            },
            configuration: &c,
            definitions: &definitions,
        })
    };
    q.own_station.location = Some(RecordedLocation {
        primary_subdivision: Some("WA".into()),
        secondary_subdivision: Some("WA,King".into()),
        ..Default::default()
    });
    let ProjectionDecision::Ready(r) = project(&q, "291")? else {
        panic!("normal US state/county must be ready")
    };
    assert_eq!(
        r.value.station_fields.get("US_STATE").map(String::as_str),
        Some("WA")
    );
    assert_eq!(
        r.value.station_fields.get("US_COUNTY").map(String::as_str),
        Some("King")
    );
    q.own_station
        .location
        .as_mut()
        .unwrap()
        .secondary_subdivision = Some("AZ,King".into());
    assert!(matches!(
        project(&q, "291")?,
        ProjectionDecision::NotReady(_)
    ));
    for (entity, field) in [("15", "RU_OBLAST"), ("224", "FI_KUNTA")] {
        let value = c
            .fields
            .get(field)
            .unwrap()
            .values
            .iter()
            .find(|v| v.dependency.as_deref() == Some(entity))
            .unwrap();
        q.own_station.location = Some(RecordedLocation {
            primary_subdivision: Some(value.value.clone()),
            ..Default::default()
        });
        let ProjectionDecision::Ready(r) = project(&q, entity)? else {
            panic!("entity dependency must be resolved")
        };
        assert_eq!(r.value.station_fields.get(field), Some(&value.value));
    }
    Ok(())
}

#[test]
fn pinned_entity_and_subdivision_zone_constraints_are_enforced() -> anyhow::Result<()> {
    use clacks_model::location::{CqZone, ItuZone, RecordedLocation};
    let c = LotwConfiguration::bundled()?;
    let d = Destination {
        id: DestinationId::new(),
        collection: RemoteCollection::Lotw {
            account: "w7hex".into(),
        },
        description: "main".into(),
    };
    let definitions = support::definitions();
    let mut q = support::qso(support::id(1));
    let signing = PublicSigningIdentity {
        callsign: q.own_station.callsign.clone(),
        dxcc: "291".into(),
    };
    let project = |q: &_| {
        lotw::project(&LotwProjectionInput {
            qso: q,
            destination: &d,
            signing: &signing,
            configuration: &c,
            definitions: &definitions,
        })
    };
    q.own_station.location = Some(RecordedLocation {
        cq_zone: Some(CqZone::new(40)?),
        ..Default::default()
    });
    assert!(matches!(project(&q)?, ProjectionDecision::NotReady(_)));
    q.own_station.location = Some(RecordedLocation {
        primary_subdivision: Some("WA".into()),
        cq_zone: Some(CqZone::new(5)?),
        itu_zone: Some(ItuZone::new(8)?),
        ..Default::default()
    });
    assert!(matches!(project(&q)?, ProjectionDecision::NotReady(_)));
    q.own_station.location.as_mut().unwrap().cq_zone = Some(CqZone::new(3)?);
    q.own_station.location.as_mut().unwrap().itu_zone = Some(ItuZone::new(6)?);
    assert!(matches!(project(&q)?, ProjectionDecision::Ready(_)));
    Ok(())
}

#[test]
fn duplicate_mapping_rows_are_configuration_errors() -> anyhow::Result<()> {
    let config = LotwConfiguration::bundled()?;
    let json = serde_json::to_string(&config)?;
    let duplicate = json.replacen(
        "\"mapping\":{",
        "\"mapping\":{\"DUP\":\"DATA\",\"DUP\":\"FT4\",",
        1,
    );
    assert!(LotwConfiguration::from_json(&duplicate).is_err());
    let colliding = json.replacen("\"mapping\":{", "\"mapping\":{\"ft4\":\"DATA\",", 1);
    assert!(LotwConfiguration::from_json(&colliding).is_err());
    Ok(())
}
