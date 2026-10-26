mod support;
use clacks_client_store::{
    logbooks::deletion::{self, TagDeleteOutcome, TagDeletionPreview},
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
};
use clacks_model::tag::{LogbookTag, TagDeletionChoice};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};
use support::{contact, id, qso, qso_key};

async fn saved(
    store: &ClientStore,
    state: RecordState,
    key: RecordKey,
) -> anyhow::Result<ClientRecord> {
    let SaveOutcome::Saved(record) = save::save(store, key, None, state).await? else {
        panic!("new save expected")
    };
    Ok(record)
}

#[tokio::test]
async fn all_tag_deletion_choices_apply_to_the_complete_membership_basis() -> anyhow::Result<()> {
    for choice in [
        TagDeletionChoice::LeaveAll,
        TagDeletionChoice::DeleteAll,
        TagDeletionChoice::DeleteUnique,
    ] {
        let dir = tempfile::tempdir()?;
        let store = ClientStore::create(
            &dir.path().join("workspace.sqlite"),
            id(1),
            StoreSettings::default(),
        )
        .await?;
        let tag_key = RecordKey {
            account: id(1),
            id: RecordId::Tag(id(20)),
        };
        let _tag = saved(
            &store,
            RecordState::Live(RecordValue::Tag(Box::new(LogbookTag::new(
                id(20),
                "Same name".into(),
            )))),
            tag_key,
        )
        .await?;
        let mut unique = qso(id(10));
        unique.tags = [id(20)].into();
        let unique = saved(&store, contact(unique), qso_key(id(10))).await?;
        let mut shared = qso(id(11));
        shared.tags = [id(20), id(21)].into();
        let shared = saved(&store, contact(shared), qso_key(id(11))).await?;
        let untagged = saved(&store, contact(qso(id(12))), qso_key(id(12))).await?;
        let preview = deletion::preview(&store, id(20)).await?;
        assert_eq!(preview.members, vec![unique, shared]);
        let TagDeleteOutcome::Applied { plan, .. } =
            deletion::delete_tag(&store, &preview, choice).await?
        else {
            panic!("tag deletion expected")
        };
        assert_eq!(plan.also_tagged_elsewhere, 1);
        assert_eq!(plan.affected.len(), 2);
        for n in [10, 11] {
            let row = save::load(&store, qso_key(id(n))).await?.unwrap();
            let deleted = matches!(choice, TagDeletionChoice::DeleteAll)
                || (matches!(choice, TagDeletionChoice::DeleteUnique) && n == 10);
            if deleted {
                assert_eq!(row.working_state(), Some(&RecordState::Deleted));
            } else {
                let Some(RecordState::Live(RecordValue::Qso(qso))) = row.working_state() else {
                    panic!("preserved contact expected")
                };
                assert!(!qso.tags.contains(&id(20)));
                assert_eq!(qso.tags.contains(&id(21)), n == 11);
            }
        }
        assert_eq!(save::load(&store, qso_key(id(12))).await?, Some(untagged));
        assert_eq!(
            save::load(&store, tag_key).await?.unwrap().working_state(),
            Some(&RecordState::Deleted)
        );
        store.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn changed_membership_cannot_broaden_a_confirmed_tag_deletion() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("workspace.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    let tag_key = RecordKey {
        account: id(1),
        id: RecordId::Tag(id(20)),
    };
    let tag = saved(
        &store,
        RecordState::Live(RecordValue::Tag(Box::new(LogbookTag::new(
            id(20),
            "Tag".into(),
        )))),
        tag_key,
    )
    .await?;
    let mut q = qso(id(10));
    q.tags = [id(20)].into();
    let first = saved(&store, contact(q), qso_key(id(10))).await?;
    let mut q = qso(id(11));
    q.tags = [id(20)].into();
    let second = saved(&store, contact(q), qso_key(id(11))).await?;
    assert!(matches!(
        deletion::delete_tag(
            &store,
            &TagDeletionPreview {
                tag: tag.clone(),
                members: vec![first.clone()],
                counts: deletion::TagDeletionCounts {
                    affected: 1,
                    also_tagged_elsewhere: 0,
                    unique: 1
                }
            },
            TagDeletionChoice::DeleteAll
        )
        .await?,
        TagDeleteOutcome::Stale { .. }
    ));
    assert_eq!(save::load(&store, qso_key(id(10))).await?, Some(first));
    assert_eq!(save::load(&store, qso_key(id(11))).await?, Some(second));
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn same_named_tags_remain_distinct_across_reopen_and_deletion() -> anyhow::Result<()> {
    use clacks_client_store::read::{
        self,
        review::{RecordKind, ReviewQuery},
    };
    use std::collections::BTreeMap;

    let directory = tempfile::tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, id(1), StoreSettings::default()).await?;
    for tag_id in [id(20), id(21)] {
        let tag = LogbookTag::new(tag_id, "POTA".into());
        let value = RecordValue::Tag(Box::new(tag));
        let key = support::key(&value);
        saved(&store, RecordState::Live(value), key).await?;
    }
    store.close().await?;

    let store = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    let mut snapshot = read::snapshot(&store).await?;
    let rows = snapshot
        .review(&ReviewQuery {
            kind: Some(RecordKind::Tag),
            ..Default::default()
        })
        .await?;
    snapshot.close().await?;
    let tags: BTreeMap<_, _> = rows
        .iter()
        .map(|row| {
            let Some(RecordState::Live(RecordValue::Tag(tag))) = row.record.working_state() else {
                panic!("live tag expected");
            };
            (tag.id, tag.name.as_str())
        })
        .collect();

    assert_eq!(tags, [(id(20), "POTA"), (id(21), "POTA")].into());

    let first = rows
        .iter()
        .find(|row| row.record.key.id == RecordId::Tag(id(20)))
        .unwrap();
    let second = rows
        .iter()
        .find(|row| row.record.key.id == RecordId::Tag(id(21)))
        .unwrap();
    assert!(matches!(
        deletion::delete_tag(
            &store,
            &deletion::preview(&store, id(20)).await?,
            TagDeletionChoice::LeaveAll
        )
        .await?,
        TagDeleteOutcome::Applied { .. }
    ));
    assert_eq!(
        save::load(&store, first.record.key)
            .await?
            .unwrap()
            .working_state(),
        Some(&RecordState::Deleted)
    );
    assert_eq!(
        save::load(&store, second.record.key).await?,
        Some(second.record.clone())
    );
    store.close().await?;
    Ok(())
}
