use crate::support;
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
        callsign: q.own_station().callsign.clone(),
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
    let stored = serde_json::to_value(&original)?;
    assert_eq!(
        stored
            .pointer("/mapping_provenance/release")
            .and_then(serde_json::Value::as_str),
        Some(c.release.as_str())
    );
    assert_eq!(
        stored
            .pointer("/mapping_provenance/upstream_commit")
            .and_then(serde_json::Value::as_str),
        Some(c.upstream_commit.as_str())
    );
    let mut inconsistent = stored.clone();
    inconsistent["key"]["mode"] = serde_json::json!("CW");
    assert!(serde_json::from_value::<lotw::LotwSubmission>(inconsistent).is_err());
    support::edit(&mut q, |q| {
        q.comment = Some("not sent".into());

        Ok(())
    });
    assert_eq!(original, get(&q)?);
    support::edit(&mut q, |q| {
        q.receive_override = Some(BandFrequency::new(
            None,
            Some(Band::from_recorded_label("40m")?),
            RecognitionCatalog::adif_318()?,
        )?);

        Ok(())
    });
    let rx = get(&q)?;
    assert_ne!(original.external_key(), rx.external_key());
    assert!(!rx.contact_fields().contains_key("FREQ_RX"));
    assert_eq!(rx.signing_identity().dxcc, "291");
    Ok(())
}

#[test]
fn entity_dependent_subdivisions_and_county_parent_are_resolved() -> anyhow::Result<()> {
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
                callsign: q.own_station().callsign.clone(),
                dxcc: entity.into(),
            },
            configuration: &c,
            definitions: &definitions,
        })
    };
    support::edit(&mut q, |q| {
        q.own_station.location = Some(
            (clacks_model::location::RecordedLocationFields {
                primary_subdivision: Some("WA".into()),
                secondary_subdivision: Some("WA,King".into()),
                ..Default::default()
            })
            .try_into()
            .expect("valid native fixture"),
        );

        Ok(())
    });
    let ProjectionDecision::Ready(r) = project(&q, "291")? else {
        panic!("normal US state/county must be ready")
    };
    assert_eq!(
        r.value.station_fields().get("US_STATE").map(String::as_str),
        Some("WA")
    );
    assert_eq!(
        r.value
            .station_fields()
            .get("US_COUNTY")
            .map(String::as_str),
        Some("King")
    );
    support::edit(&mut q, |q| {
        let mut location = q.own_station.location.take().unwrap().into_fields();
        location.secondary_subdivision = Some("AZ,King".into());
        q.own_station.location = Some(location.try_into()?);

        Ok(())
    });
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
        support::edit(&mut q, |q| {
            q.own_station.location = Some(
                (clacks_model::location::RecordedLocationFields {
                    primary_subdivision: Some(value.value.clone()),
                    ..Default::default()
                })
                .try_into()
                .expect("valid native fixture"),
            );

            Ok(())
        });
        let ProjectionDecision::Ready(r) = project(&q, entity)? else {
            panic!("entity dependency must be resolved")
        };
        assert_eq!(r.value.station_fields().get(field), Some(&value.value));
    }
    Ok(())
}

#[test]
fn pinned_entity_and_subdivision_zone_constraints_are_enforced() -> anyhow::Result<()> {
    use clacks_model::location::{CqZone, ItuZone};
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
        callsign: q.own_station().callsign.clone(),
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
    support::edit(&mut q, |q| {
        q.own_station.location = Some(
            (clacks_model::location::RecordedLocationFields {
                cq_zone: Some(CqZone::new(40)?),
                ..Default::default()
            })
            .try_into()
            .expect("valid native fixture"),
        );

        Ok(())
    });
    assert!(matches!(project(&q)?, ProjectionDecision::NotReady(_)));
    support::edit(&mut q, |q| {
        q.own_station.location = Some(
            (clacks_model::location::RecordedLocationFields {
                primary_subdivision: Some("WA".into()),
                cq_zone: Some(CqZone::new(5)?),
                itu_zone: Some(ItuZone::new(8)?),
                ..Default::default()
            })
            .try_into()
            .expect("valid native fixture"),
        );

        Ok(())
    });
    assert!(matches!(project(&q)?, ProjectionDecision::NotReady(_)));
    support::edit(&mut q, |q| {
        let mut location = q.own_station.location.take().unwrap().into_fields();
        location.cq_zone = Some(CqZone::new(3)?);
        q.own_station.location = Some(location.try_into()?);

        Ok(())
    });
    support::edit(&mut q, |q| {
        let mut location = q.own_station.location.take().unwrap().into_fields();
        location.itu_zone = Some(ItuZone::new(6)?);
        q.own_station.location = Some(location.try_into()?);

        Ok(())
    });
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

#[test]
fn retained_mapping_provenance_survives_reload_without_requiring_a_new_upload() -> anyhow::Result<()>
{
    use clacks_model::{
        ids::{AttemptId, SubmissionResultId},
        scalar::UtcTimestamp,
        service::{
            applicability::{PublicationCoverage, PublicationCoverageInput, publication_coverage},
            destination::Service,
            evidence::{
                ImportedBaseline,
                submission::{
                    ConfirmationBasis, DispatchPhase, ResultScope, SubmissionAttempt,
                    SubmissionOutcome, SubmissionProjection, SubmissionResult,
                },
            },
        },
    };
    let original = LotwConfiguration::bundled()?;
    let mut updated = original.clone();
    updated.release = "later-identical-mappings".into();
    updated.upstream_commit = "later-source".into();
    let qso = support::qso(support::id(1));
    let destination = Destination {
        id: DestinationId::new(),
        collection: RemoteCollection::Lotw {
            account: "w7hex".into(),
        },
        description: "main".into(),
    };
    let signing = PublicSigningIdentity {
        callsign: qso.own_station().callsign.clone(),
        dxcc: "291".into(),
    };
    let definitions = support::definitions();
    let project = |configuration| -> anyhow::Result<SubmissionProjection> {
        match lotw::project(&LotwProjectionInput {
            qso: &qso,
            destination: &destination,
            signing: &signing,
            configuration,
            definitions: &definitions,
        })? {
            ProjectionDecision::Ready(p) => Ok(SubmissionProjection::Lotw(Box::new(p.value))),
            result => anyhow::bail!("{result:?}"),
        }
    };
    let attempt = SubmissionAttempt {
        id: AttemptId::new(),
        qso: qso.id(),
        projection: project(&original)?,
        phase: DispatchPhase::Dispatched,
        at: UtcTimestamp::new(0, 0)?,
        batch: None,
    };
    let reloaded: SubmissionAttempt = serde_json::from_str(&serde_json::to_string(&attempt)?)?;
    assert_eq!(attempt, reloaded);
    let current = project(&updated)?;
    assert_ne!(reloaded.projection, current);
    let result = SubmissionResult {
        external_attempt: None,
        id: SubmissionResultId::new(),
        destination: destination.id,
        scope: ResultScope::IndividualAttempt(attempt.id),
        outcome: SubmissionOutcome::Success,
        phase: DispatchPhase::Dispatched,
        at: UtcTimestamp::new(1, 0)?,
        diagnostic: None,
        external: None,
    };
    let source = ImportedBaseline {
        native_end: None,
        service: Service::Lotw,
        fields: Default::default(),
        own_location: None,
        confirmation: ConfirmationBasis::default(),
        coverage: Default::default(),
    };
    assert_eq!(
        publication_coverage(&PublicationCoverageInput {
            qso: qso.id(),
            service: Service::Lotw,
            destination: Some(destination.id),
            current: Some(&current),
            source_current: &source,
            assertions: &[],
            attempts: &[reloaded],
            results: &[result]
        })?,
        PublicationCoverage::Covered
    );
    Ok(())
}
