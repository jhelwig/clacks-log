//! Membership intent preserves complete contact bases and committed partial work.

use crate::support::{self, contact, id, qso, qso_key};
use clacks_client_store::{
    logbooks::selection::SelectedView,
    read::{self, contacts::ContactPageQuery},
    save::{
        self, SaveOutcome,
        tags::{TagChangeStop, change_tags},
    },
    store::{ClientStore, StoreSettings},
};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordState, RecordValue},
};
use std::{collections::BTreeSet, ops::ControlFlow};

async fn create(store: &ClientStore, n: u128, tags: &[u128]) -> anyhow::Result<ClientRecord> {
    let mut value = qso(id(n));
    support::edit(&mut value, |value| {
        value.tags = tags.iter().copied().map(id).collect();
        Ok(())
    });
    support::edit(&mut value, |value| {
        value.comment = Some(format!("contact {n}"));
        Ok(())
    });
    let SaveOutcome::Saved(row) = save::save(store, qso_key(id(n)), None, contact(value)).await?
    else {
        panic!("created contact")
    };
    Ok(row)
}

#[tokio::test]
async fn membership_preserves_per_contact_values() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, id(1), StoreSettings::default()).await?;
    let a = create(&store, 10, &[20, 21]).await?;
    let b = create(&store, 11, &[20]).await?;
    let untouched = create(&store, 12, &[20]).await?;
    let report = change_tags(
        &store,
        &[a.clone(), b.clone()],
        &[id(22)].into(),
        &[id(20)].into(),
        |_| ControlFlow::Continue(()),
    )
    .await?;
    assert_eq!(report.entries.len(), 2);
    assert!(report.stop.is_none());
    assert!(report.unprocessed.is_empty());
    for (old, tags) in [
        (a, BTreeSet::from([id(21), id(22)])),
        (b, BTreeSet::from([id(22)])),
    ] {
        let current = save::load(&store, old.key).await?.unwrap();
        let Some(RecordState::Live(RecordValue::Qso(mut expected))) = old.working_state().cloned()
        else {
            panic!("qso")
        };
        support::edit(&mut expected, |expected| {
            expected.tags = tags;
            Ok(())
        });
        assert_eq!(
            current.working_state(),
            Some(&RecordState::Live(RecordValue::Qso(expected)))
        );
        assert_eq!(current.latest, old.latest);
        assert_eq!(current.retained, old.retained);
        assert_eq!(
            current.pending.as_ref().unwrap().original,
            old.pending.as_ref().unwrap().original
        );
    }
    assert_eq!(save::load(&store, untouched.key).await?, Some(untouched));
    let b = save::load(&store, qso_key(id(11))).await?.unwrap();
    change_tags(&store, &[b], &BTreeSet::new(), &[id(22)].into(), |_| {
        ControlFlow::Continue(())
    })
    .await?;
    store.close().await?;
    let store = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    let mut snapshot = read::snapshot(&store).await?;
    for view in [SelectedView::AllContacts, SelectedView::Untagged] {
        let page = snapshot
            .contact_page(&ContactPageQuery {
                view,
                ..Default::default()
            })
            .await?;
        assert!(page.rows.iter().any(|row| row.working.qso.id() == id(11)));
    }
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn membership_retains_stale_and_cancelled_results() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("workspace.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    let a = create(&store, 10, &[20]).await?;
    let b = create(&store, 11, &[20]).await?;
    let c = create(&store, 12, &[20]).await?;
    let mut newer = qso(id(11));
    support::edit(&mut newer, |newer| {
        newer.comment = Some("intervening correction".into());
        Ok(())
    });
    save::save(&store, b.key, Some(&b), contact(newer)).await?;
    let report = change_tags(
        &store,
        &[a.clone(), b, c.clone()],
        &[id(21)].into(),
        &BTreeSet::new(),
        |_| ControlFlow::Continue(()),
    )
    .await?;
    assert!(matches!(
        report
            .entries
            .iter()
            .find(|entry| entry.id == id(11))
            .unwrap()
            .outcome,
        SaveOutcome::Stale { .. }
    ));
    assert_eq!(report.entries.len(), 3);
    let current = save::load(&store, qso_key(id(11))).await?.unwrap();
    let Some(RecordState::Live(RecordValue::Qso(value))) = current.working_state() else {
        panic!("qso")
    };
    assert_eq!(value.comment().as_deref(), Some("intervening correction"));
    let a = save::load(&store, a.key).await?.unwrap();
    let c = save::load(&store, c.key).await?.unwrap();
    let report = change_tags(
        &store,
        &[a.clone(), c.clone()],
        &[id(22)].into(),
        &BTreeSet::new(),
        |progress| {
            if progress.completed == 1 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        },
    )
    .await?;
    assert!(matches!(report.stop, Some(TagChangeStop::Cancelled)));
    assert_eq!(report.entries.len(), 1);
    assert_eq!(report.unprocessed, vec![id(12)]);
    assert_eq!(save::load(&store, c.key).await?, Some(c.clone()));
    let unchanged = change_tags(
        &store,
        std::slice::from_ref(&c),
        &[id(21)].into(),
        &BTreeSet::new(),
        |_| ControlFlow::Continue(()),
    )
    .await?;
    assert!(
        matches!(&unchanged.entries.first().unwrap().outcome, SaveOutcome::Unchanged(row) if row == &c)
    );
    save::save(&store, c.key, Some(&c), RecordState::Deleted).await?;
    let deleted = change_tags(&store, &[c], &[id(22)].into(), &BTreeSet::new(), |_| {
        ControlFlow::Continue(())
    })
    .await?;
    assert!(matches!(
        deleted.entries.first().unwrap().outcome,
        SaveOutcome::Stale { .. }
    ));
    assert_eq!(
        save::load(&store, qso_key(id(12)))
            .await?
            .unwrap()
            .working_state(),
        Some(&RecordState::Deleted)
    );
    store.close().await?;
    Ok(())
}
