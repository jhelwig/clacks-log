use super::{
    ClientStore, ImportedClaim, ProjectionNeeds, RecordState, RecordValue, Service, ServicePayload,
    ServiceRecord, StoreSettings, account_definitions, imported, projection, read, save, support,
};
use clacks_model::{
    scalar::{Milliwatts, UtcTimestamp},
    service::{
        destination::{Destination, RemoteCollection},
        evidence::submission::{
            DispatchPhase, ResultScope, SubmissionAttempt, SubmissionOutcome, SubmissionProjection,
            SubmissionResult,
        },
        qrz::QrzSubmission,
    },
};
use clacks_sync::service::ServiceSupport;

#[tokio::test]
async fn historical_qrz_submission_coverage_uses_its_retained_book() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create_with_definitions(
        &directory.path().join("history.sqlite"),
        support::id(1),
        &account_definitions()?,
        StoreSettings::default(),
    )
    .await?;
    let mut qso = support::qso(support::id(10));
    let saved = save::save(
        &store,
        support::qso_key(qso.id()),
        None,
        support::contact(qso.clone()),
    )
    .await?;
    let state = imported(&qso, 99, Service::Qrz, ImportedClaim::Uploaded)?;
    let RecordState::Live(RecordValue::Service(s)) = state else {
        panic!("fixture")
    };
    let ServicePayload::Imported(a) = s.payload else {
        panic!("baseline")
    };
    let destination = Destination {
        id: support::id(100),
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse()?),
        },
        description: "Historical book".into(),
    };
    let at = UtcTimestamp::new(1_000, 0)?;
    let attempt = SubmissionAttempt {
        id: support::id(40),
        qso: qso.id(),
        projection: SubmissionProjection::Qrz(QrzSubmission {
            destination: destination.id,
            fields: a.baseline.fields,
        }),
        phase: DispatchPhase::Dispatched,
        at,
        batch: None,
    };
    let result = SubmissionResult {
        id: support::id(41),
        destination: destination.id,
        scope: ResultScope::IndividualAttempt(attempt.id),
        outcome: SubmissionOutcome::Success,
        phase: DispatchPhase::Dispatched,
        at,
        diagnostic: None,
        external: None,
        external_attempt: None,
    };
    for payload in [
        ServicePayload::Attempt(attempt),
        ServicePayload::Result(result),
    ] {
        let value = RecordValue::Service(Box::new(ServiceRecord {
            payload,
            support: ServiceSupport {
                destination: Some(destination.clone()),
                ..Default::default()
            },
        }));
        save::save(&store, support::key(&value), None, RecordState::Live(value)).await?;
    }
    let current_book = RecordValue::Service(Box::new(ServiceRecord {
        payload: ServicePayload::Destination(Destination {
            id: support::id(200),
            collection: RemoteCollection::Qrz {
                book_id: Some("8".parse()?),
            },
            description: "New book".into(),
        }),
        support: Default::default(),
    }));
    save::save(
        &store,
        support::key(&current_book),
        None,
        RecordState::Live(current_book),
    )
    .await?;
    let mut snapshot = read::snapshot(&store).await?;
    let facts = projection::load(
        &mut snapshot.transaction,
        snapshot.account,
        &[qso.id()],
        ProjectionNeeds {
            station: true,
            services: true,
        },
    )
    .await?;
    assert_eq!(facts.first().unwrap().id, qso.id());
    assert!(facts.first().unwrap().facts.qrz.uploaded);
    assert!(!facts.first().unwrap().facts.qrz.confirmed);
    snapshot.close().await?;
    support::edit(&mut qso, |qso| {
        qso.own_context.transmit_power = Some(Milliwatts::new(100_000)?);
        Ok(())
    });
    let save::SaveOutcome::Saved(saved) = saved else {
        panic!("save")
    };
    save::save(
        &store,
        saved.key,
        Some(&saved),
        support::contact(qso.clone()),
    )
    .await?;
    let mut snapshot = read::snapshot(&store).await?;
    let facts = projection::load(
        &mut snapshot.transaction,
        snapshot.account,
        &[qso.id()],
        ProjectionNeeds {
            station: true,
            services: true,
        },
    )
    .await?;
    assert!(!facts.first().unwrap().facts.qrz.uploaded);
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn lotw_coverage_retains_its_public_signing_and_account_context() -> anyhow::Result<()> {
    use clacks_model::{
        recognition::mode::{Mode, ModeIdentity},
        service::{
            error::ProjectionDecision,
            lotw::{self, LotwConfiguration, LotwProjectionInput, PublicSigningIdentity},
        },
    };
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create_with_definitions(
        &directory.path().join("history.sqlite"),
        support::id(1),
        &account_definitions()?,
        StoreSettings::default(),
    )
    .await?;
    let mut qso = support::qso(support::id(10));
    support::edit(&mut qso, |qso| {
        qso.own_station.location = Some(
            (clacks_model::location::RecordedLocationFields {
                dxcc: Some("291".into()),
                ..Default::default()
            })
            .try_into()
            .expect("valid native fixture"),
        );
        Ok(())
    });
    let saved = save::save(
        &store,
        support::qso_key(qso.id()),
        None,
        support::contact(qso.clone()),
    )
    .await?;
    let seed = account_definitions()?;
    let definitions = clacks_model::group::DefinitionSet::new(seed.groups)?;
    let destination = Destination {
        id: support::id(100),
        collection: RemoteCollection::Lotw {
            account: "personal".into(),
        },
        description: "Personal account".into(),
    };
    let signing = PublicSigningIdentity {
        callsign: qso.own_station().callsign.clone(),
        dxcc: "291".into(),
    };
    let ProjectionDecision::Ready(submission) = lotw::project(&LotwProjectionInput {
        qso: &qso,
        destination: &destination,
        signing: &signing,
        configuration: &LotwConfiguration::bundled()?,
        definitions: &definitions,
    })?
    else {
        panic!("independent prepared submission fixture")
    };
    let at = UtcTimestamp::new(1_000, 0)?;
    let attempt = SubmissionAttempt {
        id: support::id(40),
        qso: qso.id(),
        projection: SubmissionProjection::Lotw(Box::new(submission.value)),
        phase: DispatchPhase::Dispatched,
        at,
        batch: None,
    };
    let result = SubmissionResult {
        id: support::id(41),
        destination: destination.id,
        scope: ResultScope::IndividualAttempt(attempt.id),
        outcome: SubmissionOutcome::Success,
        phase: DispatchPhase::Dispatched,
        at,
        diagnostic: None,
        external: None,
        external_attempt: None,
    };
    for payload in [
        ServicePayload::Attempt(attempt),
        ServicePayload::Result(result),
    ] {
        let value = RecordValue::Service(Box::new(ServiceRecord {
            payload,
            support: ServiceSupport {
                destination: Some(destination.clone()),
                ..Default::default()
            },
        }));
        save::save(&store, support::key(&value), None, RecordState::Live(value)).await?;
    }
    let club = RecordValue::Service(Box::new(ServiceRecord {
        payload: ServicePayload::Destination(Destination {
            id: support::id(200),
            collection: RemoteCollection::Lotw {
                account: "club".into(),
            },
            description: "Separate club account".into(),
        }),
        support: Default::default(),
    }));
    save::save(&store, support::key(&club), None, RecordState::Live(club)).await?;
    let mut snapshot = read::snapshot(&store).await?;
    let facts = projection::load(
        &mut snapshot.transaction,
        snapshot.account,
        &[qso.id()],
        ProjectionNeeds {
            station: true,
            services: true,
        },
    )
    .await?;
    assert!(facts.first().unwrap().facts.lotw.uploaded);
    snapshot.close().await?;
    support::edit(&mut qso, |qso| {
        qso.mode = Mode::from_recorded_identity(ModeIdentity::Parent {
            label: "SSB".into(),
        })?;
        Ok(())
    });
    let save::SaveOutcome::Saved(saved) = saved else {
        panic!("save")
    };
    save::save(
        &store,
        saved.key,
        Some(&saved),
        support::contact(qso.clone()),
    )
    .await?;
    let mut snapshot = read::snapshot(&store).await?;
    let facts = projection::load(
        &mut snapshot.transaction,
        snapshot.account,
        &[qso.id()],
        ProjectionNeeds {
            station: true,
            services: true,
        },
    )
    .await?;
    assert!(!facts.first().unwrap().facts.lotw.uploaded);
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}
