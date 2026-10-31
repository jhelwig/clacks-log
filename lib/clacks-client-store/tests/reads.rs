use crate::support;
use clacks_client_store::{
    read::{
        self,
        contacts::{ContactColumn, ContactOrder, ContactPageQuery, SortDirection},
        review::ReviewQuery,
    },
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
    support::edit(&mut qso, |qso| {
        qso.tags = [support::id(20), support::id(21)].into();
        Ok(())
    });
    let SaveOutcome::Saved(row) = save::save(
        &store,
        support::qso_key(qso.id()),
        None,
        support::contact(qso),
    )
    .await?
    else {
        panic!("save")
    };
    let mut old = read::snapshot(&store).await?;
    let query = ContactPageQuery {
        view: clacks_client_store::logbooks::selection::SelectedView::Logbook(support::id(20)),
        ..Default::default()
    };
    assert_eq!(old.contact_page(&query).await?.rows.len(), 1);
    save::save(&store, row.key(), Some(&row), RecordState::Deleted).await?;
    assert_eq!(old.contact_page(&query).await?.rows.len(), 1);
    assert!(old.record(row.key()).await?.unwrap().pending().is_some());
    old.close().await?;
    let mut new = read::snapshot(&store).await?;
    assert!(
        new.contact_page(&ContactPageQuery::default())
            .await?
            .rows
            .is_empty()
    );
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
async fn expression_pages_preserve_native_labels_and_review_identity() -> anyhow::Result<()> {
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
        support::edit(&mut qso, |qso| {
            qso.contacted_station.callsign = RecordedCallsign::new(call)?;
            Ok(())
        });
        support::edit(&mut qso, |qso| {
            qso.start = UtcSecond::from_unix_seconds(-1);
            Ok(())
        });
        support::edit(&mut qso, |qso| {
            qso.primary = BandFrequency::from_recorded_pair(
                None,
                Some(Band::from_recorded_label("future-band")?),
            )?;
            Ok(())
        });
        support::edit(&mut qso, |qso| {
            qso.mode = Mode::from_recorded_identity(ModeIdentity::Parent {
                label: "future-mode".into(),
            })?;
            Ok(())
        });
        save::save(
            &store,
            support::qso_key(qso.id()),
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
    let query = ContactPageQuery {
        filter: clacks_model::query::parse(
            "k1 band:future-band mode:future-mode qso_start:1969-12-31T23:59:59Z",
            clacks_model::recognition::RecognitionCatalog::adif_318()?,
        )?,
        order: ContactOrder {
            column: ContactColumn::Start,
            direction: SortDirection::Ascending,
        },
        limit: 1,
        ..Default::default()
    };
    let first = snapshot.contact_page(&query).await?;
    assert_eq!(first.rows.len(), 1);
    let row = first.rows.first().unwrap();
    assert_eq!(row.working.qso.id(), support::id(10));
    assert_eq!(
        row.working.qso.primary().band().unwrap().label(),
        "future-band"
    );
    let second = snapshot
        .contact_page(&ContactPageQuery { offset: 1, ..query })
        .await?;
    assert_eq!(
        second.rows.first().unwrap().working.qso.id(),
        support::id(11)
    );
    for prefix in ["K%", "K_", "K!"] {
        assert!(
            snapshot
                .contact_page(&ContactPageQuery {
                    filter: clacks_model::query::parse(
                        prefix,
                        clacks_model::recognition::RecognitionCatalog::adif_318()?
                    )?,
                    ..Default::default()
                })
                .await?
                .rows
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
        keys.insert(row.record.key().id);
        cursor = Some(row.record.key().id);
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

// A held row is not ready-to-send work, and deleting a contact must not hide
// its pending operation from account-wide review.
#[tokio::test]
async fn review_sections_include_hidden_and_held_work() -> anyhow::Result<()> {
    use clacks_client_store::{
        enrollment::{self, RecoveryDisposition},
        identity::{self, EnrollmentBinding},
        read::review::{RecordKind, ReviewSection},
    };
    use clacks_model::tag::LogbookTag;
    use clacks_sync::record::{RecordId, RecordValue};

    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("review.sqlite"),
        support::id(1),
        StoreSettings::default(),
    )
    .await?;
    let old = EnrollmentBinding {
        account: support::id(1),
        actor: support::id(9),
        server: "https://clacks.example".into(),
        credential_reference: "old".into(),
    };
    identity::bind(&store, None, old.clone()).await?;
    let held = support::qso_key(support::id(10));
    save::save(
        &store,
        held,
        None,
        support::contact(support::qso(support::id(10))),
    )
    .await?;
    enrollment::install(
        &store,
        Some(&old),
        EnrollmentBinding {
            actor: support::id(12),
            credential_reference: "new".into(),
            ..old.clone()
        },
        RecoveryDisposition::FreshActor {
            former_actor: old.actor,
        },
    )
    .await?;
    for value in [10, 12] {
        let tag = RecordValue::Tag(Box::new(LogbookTag::new(support::id(value), "Main".into())));
        save::save(&store, support::key(&tag), None, RecordState::Live(tag)).await?;
    }
    let deleted = support::qso_key(support::id(11));
    let SaveOutcome::Saved(row) = save::save(
        &store,
        deleted,
        None,
        support::contact(support::qso(support::id(11))),
    )
    .await?
    else {
        panic!("saved")
    };
    save::save(&store, deleted, Some(&row), RecordState::Deleted).await?;

    let mut snapshot = read::snapshot(&store).await?;
    let page = snapshot
        .review_page(&ReviewQuery {
            section: Some(ReviewSection::Conflicts),
            limit: 1,
            ..Default::default()
        })
        .await?;
    assert_eq!(page.counts.conflicts, 1);
    assert_eq!(page.counts.pending_sync, 3);
    assert_eq!(page.rows.first().unwrap().record.key(), held);
    assert_eq!(
        page.rows.first().unwrap().authorization_hold,
        Some(old.actor)
    );
    assert!(page.rows.first().unwrap().review.is_none());
    assert!(page.next.is_none());
    let first = snapshot
        .review_page(&ReviewQuery {
            section: Some(ReviewSection::PendingSync),
            limit: 1,
            ..Default::default()
        })
        .await?;
    assert_eq!(first.rows.first().unwrap().record.key(), deleted);
    assert_eq!(
        first.rows.first().unwrap().record.working_state(),
        Some(&RecordState::Deleted)
    );
    let second = snapshot
        .review_page(&ReviewQuery {
            section: Some(ReviewSection::PendingSync),
            after: first.next,
            limit: 1,
            ..Default::default()
        })
        .await?;
    assert_eq!(
        second.rows.first().unwrap().record.key().id,
        RecordId::Tag(support::id(10))
    );
    let tags = snapshot
        .review_page(&ReviewQuery {
            kind: Some(RecordKind::Tag),
            ..Default::default()
        })
        .await?;
    assert_eq!(tags.counts.pending_sync, 2);
    assert_eq!(tags.rows.len(), 2);
    assert_eq!(
        snapshot.review_record(held).await?.unwrap().record.key(),
        held
    );
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}
