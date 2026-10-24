mod support;
use clacks_client_store::{
    read::{self, history::HistoryQuery, review::ReviewQuery},
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
};
use clacks_sync::record::RecordState;

#[tokio::test]
async fn working_history_is_unique_snapshot_consistent_and_deletions_are_reviewable()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("workspace.sqlite"),
        support::id(1),
        StoreSettings::default(),
    )
    .await?;
    let mut qso = support::qso(support::id(10));
    qso.tags = [support::id(20), support::id(21)].into();
    let SaveOutcome::Saved(row) = save::save(
        &store,
        support::qso_key(qso.id),
        None,
        support::contact(qso),
    )
    .await?
    else {
        panic!("save")
    };
    let mut old = read::snapshot(&store).await?;
    let query = HistoryQuery {
        tags: [support::id(20), support::id(21)].into(),
        ..Default::default()
    };
    assert_eq!(old.history(&query).await?.len(), 1);
    save::save(&store, row.key, Some(&row), RecordState::Deleted).await?;
    assert_eq!(old.history(&query).await?.len(), 1);
    assert!(old.record(row.key).await?.unwrap().pending.is_some());
    old.close().await?;
    let mut new = read::snapshot(&store).await?;
    assert!(new.history(&HistoryQuery::default()).await?.is_empty());
    let review = new.review(&ReviewQuery::default()).await?;
    assert_eq!(review.len(), 1);
    assert_eq!(
        review.first().unwrap().record.working_state(),
        Some(&RecordState::Deleted)
    );
    new.close().await?;
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn history_prefix_filters_and_keyset_pages_preserve_native_labels() -> anyhow::Result<()> {
    use clacks_client_store::read::review::RecordKind;
    use clacks_model::{
        contact::BandFrequency,
        recognition::{
            band::Band,
            mode::{Mode, ModeIdentity},
        },
        scalar::{RecordedCallsign, UtcSecond},
        tag::LogbookTag,
    };
    use clacks_sync::record::{RecordId, RecordValue};
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("workspace.sqlite"),
        support::id(1),
        StoreSettings::default(),
    )
    .await?;
    for (n, call) in [(10, "K1ABC"), (11, "K1XYZ"), (12, "K2ABC")] {
        let mut qso = support::qso(support::id(n));
        qso.contacted_station.callsign = RecordedCallsign::new(call)?;
        qso.start = UtcSecond::from_unix_seconds(-1);
        qso.primary = BandFrequency::from_recorded_pair(
            None,
            Some(Band::from_recorded_label("future-band")?),
        )?;
        qso.mode = Mode::from_recorded_identity(ModeIdentity::Parent {
            label: "future-mode".into(),
        })?;
        save::save(
            &store,
            support::qso_key(qso.id),
            None,
            support::contact(qso),
        )
        .await?;
    }
    // Distinct kinds with identical raw IDs must both survive review pagination.
    let tag = RecordValue::Tag(Box::new(LogbookTag::new(
        support::id(10),
        "same identifier".into(),
    )));
    save::save(&store, support::key(&tag), None, RecordState::Live(tag)).await?;
    let mut snapshot = read::snapshot(&store).await?;
    let query = HistoryQuery {
        contacted_call_prefix: Some(" k1 ".into()),
        band: Some("future-band".into()),
        mode: Some("future-mode".into()),
        start: Some(UtcSecond::from_unix_seconds(-1)),
        end: Some(UtcSecond::from_unix_seconds(-1)),
        limit: 1,
        ..Default::default()
    };
    let first = snapshot.history(&query).await?;
    assert_eq!(first.len(), 1);
    let row = first.first().unwrap();
    assert_eq!(row.qso.id, support::id(10));
    let second = snapshot
        .history(&HistoryQuery {
            after: Some((row.qso.start, row.qso.id)),
            ..query
        })
        .await?;
    assert_eq!(second.first().unwrap().qso.id, support::id(11));
    for prefix in ["K%", "K_", "K!"] {
        assert!(
            snapshot
                .history(&HistoryQuery {
                    contacted_call_prefix: Some(prefix.into()),
                    ..Default::default()
                })
                .await?
                .is_empty()
        );
    }
    let mut cursor = None;
    let mut keys = std::collections::BTreeSet::new();
    loop {
        let page = snapshot
            .review(&ReviewQuery {
                after: cursor,
                limit: 1,
                ..Default::default()
            })
            .await?;
        let Some(row) = page.first() else { break };
        keys.insert(row.record.key.id);
        cursor = Some(row.record.key.id);
    }
    assert_eq!(keys.len(), 4);
    assert!(
        keys.contains(&RecordId::Qso(support::id(10)))
            && keys.contains(&RecordId::Tag(support::id(10)))
    );
    assert_eq!(
        snapshot
            .review(&ReviewQuery {
                kind: Some(RecordKind::Tag),
                ..Default::default()
            })
            .await?
            .len(),
        1
    );
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}
