use super::*;
use clacks_client_store::store::StoreSettings;
use clacks_model::{group::DefinitionSet, scalar::UtcSecond};

fn contact() -> anyhow::Result<Qso> {
    let mut form = crate::logging::form::ContactForm::empty();
    form.own.callsign = "W7HEX".into();
    form.dx.callsign = "OK2CQR".into();
    form.primary.band = "20m".into();
    form.mode = "SSB".into();
    form.start =
        crate::logging::time::ContactTimestamp::recorded(Some(UtcSecond::from_unix_seconds(1)));
    form.build(QsoId::new(), &DefinitionSet::new(vec![])?)
        .map_err(|e| anyhow::anyhow!("{e:?}"))
}

#[tokio::test]
async fn repeated_create_acknowledges_only_the_same_submitted_contact() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("store.sqlite"),
        Default::default(),
        StoreSettings::default(),
    )
    .await?;
    let qso = contact()?;
    let created = execute(
        &store,
        ContactAction::Save {
            expected: None,
            next: Box::new(qso.clone()),
        },
    )
    .await?;
    assert!(matches!(
        created,
        ContactResponse::Saved(SaveOutcome::Saved(_))
    ));
    let state = QueryState {
        edited: "mode:SSB OR".into(),
        applied: "mode:SSB".into(),
    };
    assert!(matches!(
        execute(&store, ContactAction::SaveQueryState { state: state.clone() }).await?,
        ContactResponse::QueryStateSaved(saved) if saved == state
    ));
    assert!(matches!(
        execute(&store, ContactAction::LoadQueryState).await?,
        ContactResponse::QueryState(saved) if saved == state
    ));
    let page = execute(
        &store,
        ContactAction::Page {
            query: ContactPageQuery {
                filter: clacks_model::query::parse(
                    &state.applied,
                    clacks_model::recognition::RecognitionCatalog::adif_318()?,
                )?,
                ..Default::default()
            },
        },
    )
    .await?;
    let ContactResponse::Page(page) = page else {
        anyhow::bail!("missing query page")
    };
    assert_eq!(page.matching_total, 1);
    assert_eq!(page.rows.first().unwrap().working.qso.id(), qso.id());
    let repeated = execute(
        &store,
        ContactAction::Save {
            expected: None,
            next: Box::new(qso.clone()),
        },
    )
    .await?;
    let ContactResponse::Saved(SaveOutcome::Unchanged(current)) = repeated else {
        anyhow::bail!("retry did not acknowledge saved facts")
    };
    let deleted = execute(
        &store,
        ContactAction::Delete {
            expected: Box::new(current),
        },
    )
    .await?;
    assert!(matches!(
        deleted,
        ContactResponse::Deleted(SaveOutcome::Saved(_))
    ));
    let retry = execute(
        &store,
        ContactAction::Save {
            expected: None,
            next: Box::new(qso),
        },
    )
    .await?;
    assert!(matches!(
        retry,
        ContactResponse::Saved(SaveOutcome::Stale { .. })
    ));
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn membership_preparation_captures_the_selected_basis() -> anyhow::Result<()> {
    use super::membership::{self, MembershipSubmission};
    use clacks_model::ids::TagId;
    use tokio::sync::watch;
    use tokio_util::sync::CancellationToken;
    // Match the existing capture setup: keep tracing out of its single-dispatch
    // cache while unconfigured parallel workers register callsites.
    let _cache_guard = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let output = tempfile::NamedTempFile::new()?;
    let subscriber = tracing_subscriber::fmt()
        .json()
        .without_time()
        .with_writer(output.reopen()?)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("store.sqlite"),
        Default::default(),
        StoreSettings::default(),
    )
    .await?;
    let a = contact()?;
    let b = contact()?;
    for qso in [&a, &b] {
        execute(
            &store,
            ContactAction::Save {
                expected: None,
                next: Box::new(qso.clone()),
            },
        )
        .await?;
    }
    let ids = [a.id(), b.id()].into();
    let basis = membership::prepare(&store, &ids).await?;
    assert_eq!(basis.records.len(), 2);
    let first = basis.records.first().unwrap().clone();
    let Some(RecordState::Live(RecordValue::Qso(mut next))) = first.working_state().cloned() else {
        panic!("qso")
    };
    crate::test_support::edit(&mut next, |next| {
        next.comment = Some("newer fact".into());
        Ok(())
    });
    save::save(
        &store,
        first.key(),
        Some(&first),
        RecordState::Live(RecordValue::Qso(next)),
    )
    .await?;
    let (progress, receiver) = watch::channel(Default::default());
    let report = membership::apply(
        &store,
        MembershipSubmission {
            basis: basis.records,
            add: [TagId::new()].into(),
            remove: Default::default(),
            cancel: CancellationToken::new(),
            progress,
        },
    )
    .await?;
    assert_eq!(report.entries.len(), 2);
    assert!(matches!(
        report.entries.first().unwrap().outcome,
        SaveOutcome::Stale { .. }
    ));
    assert_eq!(receiver.borrow().changed, 1);
    assert_eq!(receiver.borrow().stale, 1);
    let basis = membership::prepare(&store, &ids).await?;
    let cancel = CancellationToken::new();
    cancel.cancel();
    let (progress, receiver) = watch::channel(Default::default());
    let report = membership::apply(
        &store,
        MembershipSubmission {
            basis: basis.records,
            add: [TagId::new()].into(),
            remove: Default::default(),
            cancel,
            progress,
        },
    )
    .await?;
    assert!(report.entries.is_empty());
    assert_eq!(report.unprocessed.len(), 2);
    assert_eq!(receiver.borrow().changed, 0);
    assert!(
        membership::prepare(&store, &[QsoId::new()].into())
            .await
            .is_err()
    );
    let b = save::load(&store, key(&store, b.id())).await?.unwrap();
    save::save(&store, b.key(), Some(&b), RecordState::Deleted).await?;
    assert!(membership::prepare(&store, &ids).await.is_err());
    store.close().await?;
    let captured = std::fs::read_to_string(output.path())?;
    let event = captured
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["fields"]["event"] == "membership.change.completed")
        .unwrap();
    assert_eq!(event["fields"]["changed"], 1);
    assert_eq!(event["fields"]["stale"], 1);
    assert_eq!(event["fields"]["unprocessed"], 0);
    assert!(!captured.contains("newer fact"));

    Ok(())
}
