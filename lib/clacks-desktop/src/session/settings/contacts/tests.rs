use super::*;
async fn execute(store: &ClientStore, action: ContactAction) -> anyhow::Result<ContactResponse> {
    super::execute(store, action, None, tokio::sync::watch::channel(()).0).await
}

use clacks_client_store::store::StoreSettings;
use clacks_model::{group::DefinitionSet, scalar::UtcSecond};

#[tokio::test]
async fn select_all_reads_every_applied_match() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("selection.sqlite"),
        Default::default(),
        Default::default(),
    )
    .await?;
    let mut expected = std::collections::BTreeSet::new();
    for _ in 0..205 {
        let qso = contact()?;
        expected.insert(qso.id());
        save::contacts::save(&store, None, qso, None).await?;
    }
    for _ in 0..2 {
        let mut qso = contact()?;
        crate::test_support::edit(&mut qso, |fields| {
            fields.contacted_station.callsign =
                clacks_model::scalar::RecordedCallsign::new("W7OTHER")?;
            Ok(())
        });
        save::contacts::save(&store, None, qso, None).await?;
    }
    let deleted = contact()?;
    let SaveOutcome::Saved(basis) =
        save::contacts::save(&store, None, deleted.clone(), None).await?
    else {
        panic!("save");
    };
    save::save(
        &store,
        key(&store, deleted.id()),
        Some(&basis),
        RecordState::Deleted,
    )
    .await?;
    let filter = clacks_model::query::parse(
        "contact.call:OK2CQR",
        clacks_model::recognition::RecognitionCatalog::adif_318()?,
    )?;
    let result = execute(
        &store,
        ContactAction::SelectAll {
            query: ContactPageQuery {
                filter,
                show_selected: true,
                offset: 100,
                limit: 1,
                ..Default::default()
            },
        },
    )
    .await?;
    let ContactResponse::SelectedAll(ids) = result else {
        panic!("selection response");
    };
    assert_eq!(ids, expected);
    assert_eq!(ids.len(), 205);
    store.close().await?;
    Ok(())
}

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
            directory: None,
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
            directory: None,
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
            directory: None,
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
